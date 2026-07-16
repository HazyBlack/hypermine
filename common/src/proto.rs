use serde::{Deserialize, Serialize};

use crate::{
    EntityId, SimConfig, Step, dodeca, graph::NodeId, math::MIsometry, node::ChunkId,
    voxel_math::Coords, world::Material,
};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientHello {
    pub name: String,
    #[serde(default)]
    pub protocol_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerHello {
    pub character: EntityId,
    pub sim_config: SimConfig,
    pub protocol_version: u32,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct Position {
    pub node: NodeId,
    pub local: MIsometry<f32>,
}

impl Position {
    pub fn origin() -> Self {
        Self {
            node: NodeId::ROOT,
            local: MIsometry::identity(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDelta {
    pub step: Step,
    /// Highest input generation received prior to `step`
    pub latest_input: u16,
    pub positions: Vec<(EntityId, Position)>,
    pub character_states: Vec<(EntityId, CharacterState)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterState {
    pub velocity: na::Vector3<f32>,
    pub on_ground: bool,
    pub orientation: na::UnitQuaternion<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Spawns {
    pub step: Step,
    pub spawns: Vec<(EntityId, Vec<Component>)>,
    pub despawns: Vec<EntityId>,
    pub nodes: Vec<FreshNode>,
    pub block_updates: Vec<BlockUpdate>,
    pub voxel_data: Vec<(ChunkId, SerializedVoxelData)>,
    pub inventory_additions: Vec<(EntityId, EntityId)>,
    pub inventory_removals: Vec<(EntityId, EntityId)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Command {
    pub generation: u16,
    pub character_input: CharacterInput,
    pub orientation: na::UnitQuaternion<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterInput {
    /// Relative to the character's current position, excluding orientation
    pub movement: na::Vector3<f32>,
    pub jump: bool,
    pub no_clip: bool,
    #[serde(default)]
    pub creative: bool,
    #[serde(default)]
    pub return_to_spawn: bool,
    pub block_update: Option<BlockUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockUpdate {
    pub chunk_id: ChunkId,
    pub coords: Coords,
    pub new_material: Material,
    pub consumed_entity: Option<EntityId>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerializedVoxelData {
    /// Dense 3D array of 16-bit material tags for all voxels in this chunk
    pub inner: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Component {
    Character(Character),
    Position(Position),
    Material(Material),
    Inventory(Inventory),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FreshNode {
    /// The side joining the new node to `parent`
    pub side: dodeca::Side,
    pub parent: NodeId,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Character {
    pub name: String,
    pub state: CharacterState,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Inventory {
    pub contents: Vec<EntityId>,
}

pub mod connection_error_codes {
    use quinn::VarInt;

    pub const CONNECTION_LOST: VarInt = VarInt::from_u32(0);
    pub const STREAM_ERROR: VarInt = VarInt::from_u32(1);
    pub const BAD_CLIENT_COMMAND: VarInt = VarInt::from_u32(2);
    pub const NAME_CONFLICT: VarInt = VarInt::from_u32(3);
    pub const CLIENT_CLOSED_CONNECTION: VarInt = VarInt::from_u32(4);
    pub const INCOMPATIBLE_PROTOCOL: VarInt = VarInt::from_u32(5);
}

#[cfg(test)]
mod tests {
    use crate::{
        dodeca::Vertex, graph::NodeId, node::ChunkId, voxel_math::Coords, world::Material,
    };

    use super::{BlockEditBatch, VoxelEdit};

    #[test]
    fn block_edit_batches_have_a_hard_safety_budget() {
        let mut batch = BlockEditBatch::default();
        assert!(batch.is_within_budget());
        batch.edits = vec![
            VoxelEdit {
                chunk_id: ChunkId::new(NodeId::ROOT, Vertex::A),
                coords: Coords([0, 0, 0]),
                new_material: Material::Dirt,
            };
            BlockEditBatch::MAX_EDITS + 1
        ];
        assert!(!batch.is_within_budget());
    }
}

/// A permanent voxel change without inventory-side effects. Geometry tools can collect these into
/// one validated operation instead of issuing thousands of independent gameplay requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxelEdit {
    pub chunk_id: ChunkId,
    pub coords: Coords,
    pub new_material: Material,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockEditBatch {
    pub edits: Vec<VoxelEdit>,
}

impl BlockEditBatch {
    pub const MAX_EDITS: usize = 65_536;

    pub fn is_within_budget(&self) -> bool {
        self.edits.len() <= Self::MAX_EDITS
    }
}
