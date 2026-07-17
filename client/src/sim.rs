use std::{collections::VecDeque, time::Duration};

use fxhash::FxHashMap;
use hecs::Entity;
use tracing::{debug, error, trace};

use crate::{
    local_character_controller::LocalCharacterController, metrics, prediction::PredictedMotion,
    worldgen_driver::WorldgenDriver,
};
use common::{
    EntityId, GraphEntities, SimConfig, Step, character_controller,
    collision_math::Ray,
    graph::{Graph, NodeId},
    graph_ray_casting,
    math::{MDirection, MIsometry, MPoint},
    node::VoxelData,
    proto::{
        self, BlockUpdate, Character, CharacterInput, CharacterState, ChunkVoxelEdits, Command,
        Component, Inventory, Position,
    },
    sanitize_motion_input,
    voxel_cursor::VoxelCursor,
    voxel_math::{CoordSign, Coords},
    world::Material,
};

const MATERIAL_PALETTE: [Material; 10] = [
    Material::WoodPlanks,
    Material::Grass,
    Material::Dirt,
    Material::Sand,
    Material::Snow,
    Material::WhiteBrick,
    Material::GreyBrick,
    Material::Basalt,
    Material::Water,
    Material::Lava,
];

#[derive(Debug, Clone, Copy)]
pub struct DebugCoordinates {
    pub node_hash: u128,
    pub node_depth: u32,
    pub loaded_node_count: u32,
    pub klein: [f32; 3],
    pub lorentz: [f32; 4],
    pub local_hyperbolic_radius: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct HomeGuidance {
    pub crossings_remaining: u32,
    /// The center of the next exit in camera-relative Klein coordinates.
    pub target_in_view: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy)]
pub struct AdminPickPreviewVoxel {
    pub chunk_to_view: na::Matrix4<f32>,
    pub coords: Coords,
}

/// Game state
pub struct Sim {
    // World state
    pub graph: Graph,
    /// Drives chunk generation
    worldgen_driver: WorldgenDriver,
    pub graph_entities: GraphEntities,
    entity_ids: FxHashMap<EntityId, Entity>,
    pub world: hecs::World,
    pub cfg: SimConfig,
    pub local_character_id: EntityId,
    pub local_character: Option<Entity>,
    step: Option<Step>,

    // Input state
    since_input_sent: Duration,
    /// Most recent input
    ///
    /// Units are relative to movement speed.
    movement_input: na::Vector3<f32>,
    /// Average input over the current time step. The portion of the timestep which has not yet
    /// elapsed is considered to have zero input.
    ///
    /// Units are relative to movement speed.
    average_movement_input: na::Vector3<f32>,
    no_clip: bool,
    creative_mode: bool,
    admin_pick_selected: bool,
    admin_pick_dimensions: [u16; 3],
    admin_dig_active: bool,
    admin_dig_remaining: Option<u64>,
    pending_chunk_edits: VecDeque<ChunkVoxelEdits>,
    pending_chunk_edit_count: usize,
    geometry_preview: bool,
    return_to_spawn_requested: bool,
    /// Whether no_clip will be toggled next step
    toggle_no_clip: bool,
    /// Whether the current step starts with a jump
    is_jumping: bool,
    /// Whether the jump button has been pressed since the last step
    jump_pressed: bool,
    /// Whether the jump button is currently held down
    jump_held: bool,
    /// Whether the place-block button has been pressed since the last step
    place_block_pressed: bool,
    /// Whether the break-block button has been pressed since the last step
    break_block_pressed: bool,

    selected_material: Material,

    prediction: PredictedMotion,
    local_character_controller: LocalCharacterController,
}

impl Sim {
    pub fn new(
        cfg: SimConfig,
        chunk_load_parallelism: usize,
        local_character_id: EntityId,
    ) -> Self {
        let mut graph = Graph::new(cfg.chunk_size);
        graph.ensure_node_state(NodeId::ROOT);
        Self {
            graph,
            worldgen_driver: WorldgenDriver::new(chunk_load_parallelism),
            graph_entities: GraphEntities::new(),
            entity_ids: FxHashMap::default(),
            world: hecs::World::new(),
            cfg,
            local_character_id,
            local_character: None,
            step: None,

            since_input_sent: Duration::new(0, 0),
            movement_input: na::zero(),
            average_movement_input: na::zero(),
            no_clip: true,
            creative_mode: true,
            admin_pick_selected: false,
            admin_pick_dimensions: [1, 1, 1],
            admin_dig_active: false,
            admin_dig_remaining: None,
            pending_chunk_edits: VecDeque::new(),
            pending_chunk_edit_count: 0,
            geometry_preview: false,
            return_to_spawn_requested: false,
            toggle_no_clip: false,
            is_jumping: false,
            jump_pressed: false,
            jump_held: false,
            place_block_pressed: false,
            break_block_pressed: false,
            selected_material: Material::WoodPlanks,
            prediction: PredictedMotion::new(proto::Position {
                node: NodeId::ROOT,
                local: MIsometry::identity(),
            }),
            local_character_controller: LocalCharacterController::new(),
        }
    }

    /// Rotates the camera's view in a context-dependent manner based on the desired yaw and pitch angles.
    pub fn look(&mut self, delta_yaw: f32, delta_pitch: f32, delta_roll: f32) {
        if self.no_clip {
            self.local_character_controller
                .look_free(delta_yaw, delta_pitch, delta_roll);
        } else {
            self.local_character_controller
                .look_level(delta_yaw, delta_pitch);
        }
    }

    pub fn set_movement_input(&mut self, mut raw_movement_input: na::Vector3<f32>) {
        if !self.no_clip {
            // Vertical movement keys shouldn't do anything unless no-clip is on.
            raw_movement_input.y = 0.0;
        }
        if raw_movement_input.norm_squared() >= 1.0 {
            // Cap movement input at 1
            raw_movement_input.normalize_mut();
        }
        self.movement_input = raw_movement_input;
    }

    pub fn toggle_no_clip(&mut self) {
        // We prepare to toggle no_clip after the next step instead of immediately, as otherwise,
        // there would be a discontinuity when predicting the player's position within a given step,
        // causing an undesirable jolt.
        self.toggle_no_clip = true;
    }

    pub fn set_jump_held(&mut self, jump_held: bool) {
        self.jump_held = jump_held;
        self.jump_pressed = jump_held || self.jump_pressed;
    }

    pub fn set_jump_pressed_true(&mut self) {
        self.jump_pressed = true;
    }

    pub fn set_place_block_pressed_true(&mut self) {
        self.place_block_pressed = true;
    }

    /// Returns the block the player is looking at, if any. Also includes distance and face
    pub fn looking_at(&self) -> Option<graph_ray_casting::GraphCastHit> {
        let view_position = self.view();
        let ray_casting_result = graph_ray_casting::ray_cast(
            &self.graph,
            &view_position,
            &Ray::new(MPoint::w(), -MDirection::z()),
            self.cfg.character.block_reach,
        );
        if let Ok(ray_casting_result) = ray_casting_result {
            ray_casting_result
        } else {
            tracing::warn!("Tried to run a raycast beyond generated terrain.");
            None
        }
    }

    /// Selects the material from a preset palette.
    pub fn select_material(&mut self, idx: usize) {
        self.selected_material = *MATERIAL_PALETTE.get(idx).unwrap_or(&MATERIAL_PALETTE[0]);
    }

    /// Cycles the selected material through all materials.
    pub fn next_material(&mut self) {
        self.selected_material =
            Material::VALUES[(self.selected_material as usize + 1) % Material::COUNT];
    }

    pub fn prev_material(&mut self) {
        self.selected_material = Material::VALUES
            [(self.selected_material as usize + Material::COUNT - 1) % Material::COUNT];
    }

    /// selects the material of the block the player is looking at. Will never select void.
    pub fn pick_material(&mut self) {
        if let Some(material) = self.looked_at_material() {
            self.selected_material = material;
        }
    }

    pub fn set_creative_mode(&mut self, creative: bool) {
        self.creative_mode = creative;
    }

    pub fn creative_mode(&self) -> bool {
        self.creative_mode
    }

    pub fn set_admin_pick_selected(&mut self, selected: bool) {
        self.admin_pick_selected = selected;
    }

    pub fn admin_pick_selected(&self) -> bool {
        self.admin_pick_selected && (!self.cfg.gameplay_enabled || self.creative_mode)
    }

    pub fn set_admin_pick_dimensions(&mut self, width: u16, height: u16, depth: u16) {
        self.admin_pick_dimensions = [width, height, depth];
    }

    pub fn admin_pick_dimensions(&self) -> [u16; 3] {
        self.admin_pick_dimensions
    }

    pub fn admin_pick_block_count(&self) -> u64 {
        self.admin_pick_dimensions
            .into_iter()
            .map(u64::from)
            .product()
    }

    pub fn admin_dig_active(&self) -> bool {
        self.admin_dig_active
    }

    pub fn admin_dig_remaining(&self) -> Option<u64> {
        self.admin_dig_remaining
    }

    /// Builds the visible, near-first portion of the Admin Pick selection. Large volumes are
    /// intentionally capped for frame-time safety; the HUD still reports the full operation size.
    pub fn admin_pick_preview(&self, limit: usize) -> Vec<AdminPickPreviewVoxel> {
        if !self.admin_pick_selected() || limit == 0 {
            return Vec::new();
        }
        let Some(hit) = self.looking_at() else {
            return Vec::new();
        };
        let dimensions = self.admin_pick_dimensions;
        let origin = PreviewCursor {
            voxel: VoxelCursor::from_hit(hit.chunk, hit.voxel_coords, hit.face_axis, hit.face_sign),
            chunk_to_view: hit.chunk_to_view,
        };
        let mut plane_origin = origin;
        let mut row_origin = origin;
        let mut cursor = origin;
        let mut indices = [0u16; 3];
        let total = self.admin_pick_block_count().min(limit as u64) as usize;
        let mut result = Vec::with_capacity(total);
        while result.len() < total {
            result.push(AdminPickPreviewVoxel {
                chunk_to_view: cursor.chunk_to_view,
                coords: cursor.voxel.coords,
            });
            indices[0] += 1;
            if indices[0] < dimensions[0] {
                let Some(next) = step_preview_offset(
                    &self.graph,
                    cursor,
                    0,
                    centered_offset(indices[0] - 1),
                    centered_offset(indices[0]),
                ) else {
                    break;
                };
                cursor = next;
                continue;
            }
            indices[0] = 0;
            indices[1] += 1;
            if indices[1] < dimensions[1] {
                let Some(next) = step_preview_offset(
                    &self.graph,
                    row_origin,
                    1,
                    centered_offset(indices[1] - 1),
                    centered_offset(indices[1]),
                ) else {
                    break;
                };
                row_origin = next;
                cursor = row_origin;
                continue;
            }
            indices[1] = 0;
            indices[2] += 1;
            if indices[2] >= dimensions[2] {
                break;
            }
            let Some(next) = step_preview_offset(&self.graph, plane_origin, 2, 0, 1) else {
                break;
            };
            plane_origin = next;
            row_origin = plane_origin;
            cursor = plane_origin;
        }
        result
    }

    pub fn toggle_geometry_preview(&mut self) {
        if !self.cfg.gameplay_enabled || self.creative_mode {
            self.geometry_preview = !self.geometry_preview;
        }
    }

    pub fn geometry_preview_enabled(&self) -> bool {
        self.geometry_preview && (!self.cfg.gameplay_enabled || self.creative_mode)
    }

    pub fn request_return_to_spawn(&mut self) {
        self.return_to_spawn_requested = true;
        self.local_character_controller.reset_orientation();
    }

    pub fn looked_at_material(&self) -> Option<Material> {
        let hit = self.looking_at()?;
        self.graph
            .get_material(hit.chunk, hit.voxel_coords)
            .filter(|material| *material != Material::Void)
    }

    pub fn selected_material(&self) -> Material {
        self.selected_material
    }

    pub fn set_selected_material(&mut self, material: Material) {
        self.selected_material = material;
    }

    pub fn inventory_material_counts(&self) -> [usize; Material::COUNT] {
        let mut counts = [0; Material::COUNT];
        let Some(local_character) = self.local_character else {
            return counts;
        };
        let Ok(inventory) = self.world.get::<&Inventory>(local_character) else {
            return counts;
        };
        for id in &inventory.contents {
            let Some(&entity) = self.entity_ids.get(id) else {
                continue;
            };
            if let Ok(material) = self.world.get::<&Material>(entity) {
                counts[*material as usize] += 1;
            }
        }
        counts
    }

    /// Returns an EntityId in the inventory with the given material
    pub fn get_any_inventory_entity_matching_material(
        &self,
        material: Material,
    ) -> Option<EntityId> {
        self.world
            .get::<&Inventory>(self.local_character?)
            .ok()?
            .contents
            .iter()
            .copied()
            .find(|e| {
                self.entity_ids.get(e).is_some_and(|&entity| {
                    self.world
                        .get::<&Material>(entity)
                        .is_ok_and(|m| *m == material)
                })
            })
    }

    /// Returns the number of entities in the inventory with the given material
    pub fn count_inventory_entities_matching_material(&self, material: Material) -> usize {
        let Some(local_character) = self.local_character else {
            return 0;
        };
        let Ok(inventory) = self.world.get::<&Inventory>(local_character) else {
            return 0;
        };
        inventory
            .contents
            .iter()
            .copied()
            .filter(|e| {
                self.entity_ids.get(e).is_some_and(|&entity| {
                    self.world
                        .get::<&Material>(entity)
                        .is_ok_and(|m| *m == material)
                })
            })
            .count()
    }

    pub fn set_break_block_pressed_true(&mut self) {
        self.break_block_pressed = true;
    }

    pub fn cfg(&self) -> &SimConfig {
        &self.cfg
    }

    pub fn debug_coordinates(&self) -> Option<DebugCoordinates> {
        let position = *self.prediction.predicted_position();
        let point: na::Vector4<f32> = (position.local * MPoint::origin()).into();
        if !point.iter().all(|component| component.is_finite()) || point.w.abs() < f32::EPSILON {
            return None;
        }
        Some(DebugCoordinates {
            node_hash: self.graph.hash_of(position.node),
            node_depth: self.graph.depth(position.node),
            loaded_node_count: self.graph.len(),
            klein: [point.x / point.w, point.y / point.w, point.z / point.w],
            lorentz: [point.x, point.y, point.z, point.w],
            local_hyperbolic_radius: point.w.max(1.0).acosh(),
        })
    }

    pub fn home_guidance(&self) -> HomeGuidance {
        let view = self.view();
        let crossings_remaining = self.graph.depth(view.node);
        let target_in_view = self.graph.primary_parent_side(view.node).and_then(|side| {
            let origin = MPoint::origin();
            let opposite = *side.reflection() * origin;
            let face_center = origin.midpoint(&opposite);
            let point: na::Vector4<f32> = (view.local.inverse() * face_center).into();
            (point.iter().all(|component| component.is_finite()) && point.w.abs() >= f32::EPSILON)
                .then_some([point.x / point.w, point.y / point.w, point.z / point.w])
        });
        HomeGuidance {
            crossings_remaining,
            target_in_view,
        }
    }

    pub fn step(&mut self, dt: Duration, net: &mut server::Handle) {
        self.local_character_controller.renormalize_orientation();

        let step_interval = self.cfg.step_interval;
        self.since_input_sent += dt;
        if let Some(overflow) = self.since_input_sent.checked_sub(step_interval) {
            // At least one step interval has passed since we last sent input, so it's time to
            // send again.

            // Update average movement input for the time between the last input sample and the end of
            // the previous step. dt > overflow because we check whether a step has elapsed
            // after each increment.
            self.average_movement_input +=
                self.movement_input * (dt - overflow).as_secs_f32() / step_interval.as_secs_f32();

            // Send fresh input
            self.send_input(net);
            self.place_block_pressed = false;
            self.break_block_pressed = false;

            // Toggle no clip at the start of a new step
            if self.toggle_no_clip {
                self.no_clip = !self.no_clip;
                self.toggle_no_clip = false;
            }

            self.is_jumping = self.jump_held || self.jump_pressed;
            self.jump_pressed = false;

            // Reset state for the next step
            if overflow > step_interval {
                // If it's been more than two timesteps since we last sent input, skip ahead
                // rather than spamming the server.
                self.average_movement_input = na::zero();
                self.since_input_sent = Duration::new(0, 0);
            } else {
                self.average_movement_input =
                    self.movement_input * overflow.as_secs_f32() / step_interval.as_secs_f32();
                // Send the next input a little sooner if necessary to stay in sync
                self.since_input_sent = overflow;
            }
        } else {
            // Update average movement input for the time within the current step
            self.average_movement_input +=
                self.movement_input * dt.as_secs_f32() / step_interval.as_secs_f32();
        }
        self.update_view_position();
        if !self.no_clip {
            self.local_character_controller.align_to_gravity();
        }
        self.worldgen_driver.drive(
            self.view(),
            self.cfg.chunk_generation_distance,
            self.cfg.meters_to_absolute,
            &mut self.graph,
        );
    }

    pub fn handle_net(&mut self, msg: server::Message) {
        use server::Message::*;
        match msg {
            ConnectionLost(_) | Hello(_) => {
                unreachable!("Case already handled by caller");
            }
            Spawns(msg) => self.handle_spawns(msg),
            StateDelta(msg) => {
                // Discard out-of-order messages, taking care to account for step counter wrapping.
                if self.step.is_some_and(|x| x.wrapping_sub(msg.step) >= 0) {
                    return;
                }
                self.step = Some(msg.step);
                for &(id, ref new_pos) in &msg.positions {
                    self.update_position(id, new_pos);
                }
                for &(id, ref new_state) in &msg.character_states {
                    self.update_character_state(id, new_state);
                }
                self.admin_dig_remaining =
                    msg.admin_dig_remaining.iter().find_map(|&(id, remaining)| {
                        (id == self.local_character_id).then_some(remaining)
                    });
                self.admin_dig_active = self.admin_dig_remaining.is_some();
                self.reconcile_prediction(msg.latest_input);
            }
        }
    }

    /// Integrates a bounded amount of authoritative bulk terrain work. Keeping this outside the
    /// network receive loop guarantees that a large Admin Pick operation cannot monopolize a frame.
    pub fn process_pending_chunk_edits(&mut self, mut budget: usize) {
        while budget > 0 {
            let Some(mut batch) = self.pending_chunk_edits.pop_front() else {
                break;
            };
            if batch.edits.len() > budget {
                let remainder = batch.edits.split_off(budget);
                self.pending_chunk_edits.push_front(ChunkVoxelEdits {
                    chunk_id: batch.chunk_id,
                    edits: remainder,
                });
            }
            let count = batch.edits.len();
            self.worldgen_driver
                .apply_chunk_edits(&mut self.graph, batch);
            self.pending_chunk_edit_count = self.pending_chunk_edit_count.saturating_sub(count);
            budget -= count;
        }
    }

    pub fn pending_chunk_edit_count(&self) -> usize {
        self.pending_chunk_edit_count
            .saturating_add(self.worldgen_driver.preloaded_block_update_count())
    }

    fn update_position(&mut self, id: EntityId, new_pos: &Position) {
        match self.entity_ids.get(&id) {
            None => debug!(%id, "position update for unknown entity"),
            Some(&entity) => match self.world.get::<&mut Position>(entity) {
                Ok(mut pos) => {
                    if pos.node != new_pos.node {
                        self.graph_entities.remove(pos.node, entity);
                        self.graph_entities.insert(new_pos.node, entity);
                    }
                    *pos = *new_pos;
                }
                Err(e) => error!(%id, "position update error: {}", e),
            },
        }
    }

    fn update_character_state(&mut self, id: EntityId, new_character_state: &CharacterState) {
        match self.entity_ids.get(&id) {
            None => debug!(%id, "character state update for unknown entity"),
            Some(&entity) => match self.world.get::<&mut Character>(entity) {
                Ok(mut ch) => {
                    ch.state = new_character_state.clone();
                }
                Err(e) => {
                    error!(%id, "character state update error: {}", e)
                }
            },
        }
    }

    fn reconcile_prediction(&mut self, latest_input: u16) {
        let id = self.local_character_id;
        let Some(&entity) = self.entity_ids.get(&id) else {
            debug!(%id, "reconciliation attempted for unknown entity");
            return;
        };
        let pos = match self.world.get::<&Position>(entity) {
            Ok(pos) => pos,
            Err(e) => {
                error!(%id, "reconciliation error: {}", e);
                return;
            }
        };
        let ch = match self.world.get::<&Character>(entity) {
            Ok(ch) => ch,
            Err(e) => {
                error!(%id, "reconciliation error: {}", e);
                return;
            }
        };
        self.prediction.reconcile(
            &self.cfg,
            &self.graph,
            latest_input,
            *pos,
            ch.state.velocity,
            ch.state.on_ground,
        );
    }

    fn handle_spawns(&mut self, msg: proto::Spawns) {
        self.step = self.step.max(Some(msg.step));
        let mut builder = hecs::EntityBuilder::new();
        for (id, components) in msg.spawns {
            self.spawn(&mut builder, id, components);
        }
        for &id in &msg.despawns {
            match self.entity_ids.get(&id) {
                Some(&entity) => self.destroy(entity),
                None => error!(%id, "despawned unknown entity"),
            }
        }
        if !msg.nodes.is_empty() {
            trace!(count = msg.nodes.len(), "adding nodes");
            // The first "Spawns" message from the server populates the graph and allows CPU/GPU metrics
            // to be accurate instead of measuring thousands of no-op frames
            metrics::declare_ready_for_profiling();
        }
        for node in &msg.nodes {
            // We need to get a list of nodes from the server, especially on first log-in,
            // since otherwise, we won't be able to know where the local character is with
            // just the NodeId alone.
            let node_id = self.graph.ensure_neighbor(node.parent, node.side);
            self.graph.ensure_node_state(node_id);
        }
        for block_update in msg.block_updates.into_iter() {
            self.worldgen_driver
                .apply_block_update(&mut self.graph, block_update);
        }
        for edits in msg.chunk_edits {
            self.pending_chunk_edit_count = self
                .pending_chunk_edit_count
                .saturating_add(edits.edits.len());
            self.pending_chunk_edits.push_back(edits);
        }
        for (chunk_id, voxel_data) in msg.voxel_data {
            let Some(voxel_data) = VoxelData::deserialize(&voxel_data, self.cfg.chunk_size) else {
                tracing::error!("Voxel data received from server is of incorrect dimension");
                continue;
            };
            self.worldgen_driver
                .apply_voxel_data(&mut self.graph, chunk_id, voxel_data);
        }
        for (subject, new_entity) in msg.inventory_additions {
            self.world
                .get::<&mut Inventory>(*self.entity_ids.get(&subject).unwrap())
                .unwrap()
                .contents
                .push(new_entity);
        }
        for (subject, removed_entity) in msg.inventory_removals {
            self.world
                .get::<&mut Inventory>(*self.entity_ids.get(&subject).unwrap())
                .unwrap()
                .contents
                .retain(|&id| id != removed_entity);
        }
    }

    fn spawn(
        &mut self,
        builder: &mut hecs::EntityBuilder,
        id: EntityId,
        components: Vec<Component>,
    ) {
        trace!(%id, "spawning entity");
        builder.add(id);
        let mut node = None;
        for component in components {
            use common::proto::Component::*;
            match component {
                Character(x) => {
                    builder.add(x);
                }
                Position(x) => {
                    node = Some(x.node);
                    builder.add(x);
                }
                Inventory(x) => {
                    builder.add(x);
                }
                Material(x) => {
                    builder.add(x);
                }
            };
        }
        let entity = self.world.spawn(builder.build());
        if let Some(node) = node {
            self.graph_entities.insert(node, entity);
        }
        if id == self.local_character_id {
            self.local_character = Some(entity);
        }
        if let Some(x) = self.entity_ids.insert(id, entity) {
            self.destroy_idless(x);
            error!(%id, "id collision");
        }
    }

    fn send_input(&mut self, net: &mut server::Handle) {
        let orientation = if self.no_clip {
            self.local_character_controller.orientation()
        } else {
            self.local_character_controller.horizontal_orientation()
        };
        let admin_dig = self.get_admin_dig_request();
        if admin_dig.is_some() {
            self.admin_dig_active = true;
            self.admin_dig_remaining = admin_dig.map(proto::AdminDigRequest::block_count);
        }
        let cancel_admin_dig = self.admin_pick_selected() && self.place_block_pressed;
        if cancel_admin_dig {
            self.admin_dig_active = false;
            self.admin_dig_remaining = None;
        }
        let character_input = CharacterInput {
            movement: sanitize_motion_input(orientation * self.average_movement_input),
            jump: self.is_jumping,
            no_clip: self.no_clip,
            creative: self.creative_mode,
            return_to_spawn: self.return_to_spawn_requested,
            block_update: self.get_local_character_block_update(),
            admin_dig,
            cancel_admin_dig,
            admin_edit_backlog: self.pending_chunk_edit_count().min(u32::MAX as usize) as u32,
        };
        let generation = self
            .prediction
            .push(&self.cfg, &self.graph, &character_input);

        // Any failure here will be better handled in handle_net's ConnectionLost case
        let _ = net.outgoing.send(Command {
            generation,
            character_input,
            orientation: self.local_character_controller.orientation(),
        });
        self.return_to_spawn_requested = false;
    }

    fn update_view_position(&mut self) {
        let mut view_position = *self.prediction.predicted_position();
        let mut view_velocity = *self.prediction.predicted_velocity();
        let mut view_on_ground = *self.prediction.predicted_on_ground();
        let orientation = if self.no_clip {
            self.local_character_controller.orientation()
        } else {
            self.local_character_controller.horizontal_orientation()
        };
        // Apply input that hasn't been sent yet
        let predicted_input = CharacterInput {
            // We divide by how far we are through the timestep because self.average_movement_input
            // is always over the entire timestep, filling in zeroes for the future, and we
            // want to use the average over what we have so far. Dividing by zero is handled
            // by the character_controller sanitizing this input.
            movement: orientation * self.average_movement_input
                / (self.since_input_sent.as_secs_f32() / self.cfg.step_interval.as_secs_f32()),
            jump: self.is_jumping,
            no_clip: self.no_clip,
            creative: self.creative_mode,
            return_to_spawn: false,
            block_update: None,
            admin_dig: None,
            cancel_admin_dig: false,
            admin_edit_backlog: self.pending_chunk_edit_count().min(u32::MAX as usize) as u32,
        };
        character_controller::run_character_step(
            &self.cfg,
            &self.graph,
            &mut view_position,
            &mut view_velocity,
            &mut view_on_ground,
            &predicted_input,
            self.since_input_sent.as_secs_f32(),
        );

        self.local_character_controller.update_position(
            view_position,
            self.graph.get_relative_up(&view_position).unwrap(),
            !self.no_clip,
        )
    }

    pub fn view(&self) -> Position {
        let mut pos = self.local_character_controller.oriented_position();
        let up = self.graph.get_relative_up(&pos).unwrap();
        pos.local *= MIsometry::translation_along(
            &(up.as_ref() * (self.cfg.character.character_radius - 1e-3)),
        );
        pos
    }

    pub fn nearby_nodes(&self) -> common::traversal::NearbySnapshot {
        self.worldgen_driver.nearby_nodes()
    }

    /// Destroy all aspects of an entity
    fn destroy(&mut self, entity: Entity) {
        let id = *self
            .world
            .get::<&EntityId>(entity)
            .expect("destroyed nonexistent entity");
        self.entity_ids.remove(&id);
        self.destroy_idless(entity);
    }

    /// Destroy an entity without an EntityId mapped
    fn destroy_idless(&mut self, entity: Entity) {
        if let Ok(position) = self.world.get::<&Position>(entity) {
            self.graph_entities.remove(position.node, entity);
        }
        self.world
            .despawn(entity)
            .expect("destroyed nonexistent entity");
    }

    /// Provides the logic for the player to be able to place and break blocks at will
    fn get_local_character_block_update(&self) -> Option<BlockUpdate> {
        if self.admin_pick_selected() {
            return None;
        }
        let placing = if self.place_block_pressed {
            true
        } else if self.break_block_pressed {
            false
        } else {
            return None;
        };

        let hit = self.looking_at()?;

        let block_pos = if placing {
            self.graph.get_block_neighbor(
                hit.chunk,
                hit.voxel_coords,
                hit.face_axis,
                hit.face_sign,
            )?
        } else {
            (hit.chunk, hit.voxel_coords)
        };

        let material = if placing {
            if self.selected_material == Material::Void {
                return None;
            }
            self.selected_material
        } else {
            Material::Void
        };

        let consumed_entity = if placing && self.cfg.gameplay_enabled && !self.creative_mode {
            Some(self.get_any_inventory_entity_matching_material(material)?)
        } else {
            None
        };

        Some(BlockUpdate {
            chunk_id: block_pos.0,
            coords: block_pos.1,
            new_material: material,
            consumed_entity,
        })
    }

    fn get_admin_dig_request(&self) -> Option<proto::AdminDigRequest> {
        if !self.admin_pick_selected() || !self.break_block_pressed {
            return None;
        }
        let hit = self.looking_at()?;
        let [width, height, depth] = self.admin_pick_dimensions;
        let request = proto::AdminDigRequest {
            chunk_id: hit.chunk,
            coords: hit.voxel_coords,
            face_axis: hit.face_axis,
            face_sign: hit.face_sign,
            width,
            height,
            depth,
        };
        request.is_valid().then_some(request)
    }
}

#[derive(Clone, Copy)]
struct PreviewCursor {
    voxel: VoxelCursor,
    chunk_to_view: na::Matrix4<f32>,
}

fn centered_offset(index: u16) -> i32 {
    if index == 0 {
        0
    } else if index % 2 == 1 {
        i32::from(index.div_ceil(2))
    } else {
        -i32::from(index / 2)
    }
}

fn step_preview_offset(
    graph: &Graph,
    mut cursor: PreviewCursor,
    axis: usize,
    from: i32,
    to: i32,
) -> Option<PreviewCursor> {
    let delta = to - from;
    let sign = if delta >= 0 {
        CoordSign::Plus
    } else {
        CoordSign::Minus
    };
    for _ in 0..delta.unsigned_abs() {
        cursor = step_preview(graph, cursor, axis, sign)?;
    }
    Some(cursor)
}

fn step_preview(
    graph: &Graph,
    cursor: PreviewCursor,
    axis: usize,
    sign: CoordSign,
) -> Option<PreviewCursor> {
    let direction = cursor.voxel.local_direction(axis, sign);
    let next = cursor.voxel.step(graph, axis, sign)?;
    let chunk_to_view = if next.chunk == cursor.voxel.chunk {
        cursor.chunk_to_view
    } else {
        let old_chunk_to_node = cursor.voxel.chunk.vertex.chunk_to_node();
        let node_to_view = cursor.chunk_to_view * old_chunk_to_node.try_inverse().unwrap();
        let next_node_to_old_node = if next.chunk.node == cursor.voxel.chunk.node {
            na::Matrix4::identity()
        } else {
            let side = cursor.voxel.chunk.vertex.canonical_sides()[direction.axis as usize];
            na::Matrix4::from(*side.reflection())
        };
        node_to_view * next_node_to_old_node * next.chunk.vertex.chunk_to_node()
    };
    Some(PreviewCursor {
        voxel: next,
        chunk_to_view,
    })
}
