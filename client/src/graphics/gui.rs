use yakui::{
    Alignment, Color, Constraints, CrossAxisAlignment, MainAxisSize, align, colored_box,
    colored_box_container, constrained, image, label, offset, pad, row, slider, stack, text,
    textbox,
    widgets::{Button, List, Pad},
};

use common::world::Material;

use crate::{
    Action, SettingsStore, Sim, WorldManager,
    inventory::{HOTBAR_SLOTS, ItemStack, STACK_LIMIT},
    settings::{UserSettings, display_key},
};

use super::material_icons::MaterialIcons;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuScreen {
    Closed,
    Pause,
    Options,
    Video,
    Controls,
    Bindings,
    Inventory,
    Worlds,
    CreateWorld,
}

#[derive(Default)]
pub struct GuiAction {
    pub quit: bool,
    pub restart: bool,
    pub video_changed: bool,
}

pub struct GuiState {
    show_hud: bool,
    show_debug: bool,
    screen: MenuScreen,
    rebinding: Option<Action>,
    new_world_name: String,
    status: Option<String>,
    control_page: usize,
    world_page: usize,
    icons: MaterialIcons,
    held_stack: Option<ItemStack>,
    cursor_position: [f32; 2],
    hovered_material: Option<Material>,
}

impl GuiState {
    pub fn new(icons: MaterialIcons) -> Self {
        Self {
            show_hud: true,
            show_debug: false,
            screen: MenuScreen::Closed,
            rebinding: None,
            new_world_name: "New World".to_owned(),
            status: None,
            control_page: 0,
            world_page: 0,
            icons,
            held_stack: None,
            cursor_position: [0.0, 0.0],
            hovered_material: None,
        }
    }

    pub fn menu_open(&self) -> bool {
        self.screen != MenuScreen::Closed
    }

    pub fn inventory_open(&self) -> bool {
        self.screen == MenuScreen::Inventory
    }

    pub fn set_cursor_position(&mut self, position: [f32; 2]) {
        self.cursor_position = position;
    }

    pub fn open_inventory(
        &mut self,
        sim: Option<&mut Sim>,
        settings: &mut SettingsStore,
        world_id: &str,
    ) {
        self.screen = MenuScreen::Inventory;
        self.reconcile_and_sync(sim, settings, world_id);
    }

    pub fn close_inventory(
        &mut self,
        sim: Option<&mut Sim>,
        settings: &mut SettingsStore,
        world_id: &str,
    ) {
        let unlimited = sim.as_deref().is_none_or(|sim| {
            !sim.cfg.gameplay_enabled || settings.value.inventory.layout(world_id).creative_tab
        });
        settings
            .value
            .inventory
            .layout_mut(world_id)
            .return_held(&mut self.held_stack, unlimited);
        self.screen = MenuScreen::Closed;
        self.reconcile_and_sync(sim, settings, world_id);
        settings.save();
    }

    pub fn select_hotbar_slot(
        &mut self,
        index: usize,
        sim: Option<&mut Sim>,
        settings: &mut SettingsStore,
        world_id: &str,
    ) {
        settings
            .value
            .inventory
            .layout_mut(world_id)
            .select_hotbar(index);
        self.reconcile_and_sync(sim, settings, world_id);
        settings.save();
    }

    pub fn cycle_hotbar(
        &mut self,
        delta: i32,
        sim: Option<&mut Sim>,
        settings: &mut SettingsStore,
        world_id: &str,
    ) {
        settings
            .value
            .inventory
            .layout_mut(world_id)
            .cycle_hotbar(delta);
        self.reconcile_and_sync(sim, settings, world_id);
        settings.save();
    }

    pub fn pick_material(
        &mut self,
        material: Material,
        sim: Option<&mut Sim>,
        settings: &mut SettingsStore,
        world_id: &str,
    ) {
        let unlimited = sim.as_deref().is_none_or(|sim| {
            !sim.cfg.gameplay_enabled || settings.value.inventory.layout(world_id).creative_tab
        });
        settings
            .value
            .inventory
            .layout_mut(world_id)
            .select_existing_or_replace(material, unlimited);
        self.reconcile_and_sync(sim, settings, world_id);
        settings.save();
    }

    fn reconcile_and_sync(
        &mut self,
        sim: Option<&mut Sim>,
        settings: &mut SettingsStore,
        world_id: &str,
    ) {
        let Some(sim) = sim else {
            return;
        };
        let counts = sim.inventory_material_counts();
        let layout = settings.value.inventory.layout_mut(world_id);
        let unlimited = !sim.cfg.gameplay_enabled || layout.creative_tab;
        layout.reconcile(&counts, unlimited, self.held_stack);
        sim.set_creative_mode(layout.creative_tab);
        sim.set_selected_material(layout.selected_stack().material.unwrap_or(Material::Void));
    }

    pub fn toggle_hud(&mut self) {
        self.show_hud = !self.show_hud;
    }

    pub fn toggle_debug(&mut self) {
        self.show_debug = !self.show_debug;
    }

    pub fn handle_escape(&mut self) {
        self.rebinding = None;
        self.status = None;
        self.screen = match self.screen {
            MenuScreen::Closed => MenuScreen::Pause,
            MenuScreen::Pause => MenuScreen::Closed,
            MenuScreen::Options | MenuScreen::Worlds => MenuScreen::Pause,
            MenuScreen::Video | MenuScreen::Controls => MenuScreen::Options,
            MenuScreen::Bindings => MenuScreen::Controls,
            MenuScreen::Inventory => MenuScreen::Closed,
            MenuScreen::CreateWorld => MenuScreen::Worlds,
        };
    }

    pub fn capture_binding(&mut self, key: String, settings: &mut SettingsStore) -> bool {
        let Some(action) = self.rebinding.take() else {
            return false;
        };
        settings.value.controls.set_binding(action, key);
        settings.save();
        true
    }

    pub fn is_rebinding(&self) -> bool {
        self.rebinding.is_some()
    }

    /// Prepare the GUI for rendering. This should be called between Yakui::start and Yakui::finish.
    pub fn run(
        &mut self,
        mut sim: Option<&mut Sim>,
        settings: &mut SettingsStore,
        worlds: &mut WorldManager,
        surface_size: [f32; 2],
    ) -> GuiAction {
        let mut action = GuiAction::default();
        let world_id = worlds.selected_id().to_owned();
        self.reconcile_and_sync(sim.as_deref_mut(), settings, &world_id);
        let layout = settings.value.inventory.layout(&world_id);
        let unlimited = sim
            .as_deref()
            .is_none_or(|sim| !sim.cfg.gameplay_enabled || layout.creative_tab);
        if self.show_hud && !self.menu_open() {
            self.hud(&layout, unlimited);
        }
        if self.show_debug
            && !self.menu_open()
            && let Some(sim) = sim.as_deref()
        {
            self.debug_overlay(sim);
        }
        if !self.menu_open() {
            return action;
        }

        align(Alignment::CENTER, || {
            colored_box(Color::BLACK.with_alpha(0.72), surface_size);
        });

        let mut next_screen = None;
        match self.screen {
            MenuScreen::Closed => {}
            MenuScreen::Pause => self.pause_menu(worlds, &mut next_screen, &mut action),
            MenuScreen::Options => self.options_menu(&mut next_screen),
            MenuScreen::Video => {
                self.video_menu(settings, &mut next_screen, &mut action);
            }
            MenuScreen::Controls => {
                self.controls_menu(settings, &mut next_screen);
            }
            MenuScreen::Bindings => {
                self.bindings_menu(settings, &mut next_screen);
            }
            MenuScreen::Inventory => {
                self.inventory_menu(sim, settings, &world_id, &mut next_screen);
            }
            MenuScreen::Worlds => {
                self.worlds_menu(worlds, &mut next_screen, &mut action);
            }
            MenuScreen::CreateWorld => {
                self.create_world_menu(worlds, &mut next_screen, &mut action);
            }
        }
        if let Some(screen) = next_screen {
            self.screen = screen;
            self.rebinding = None;
            self.status = None;
        }
        if self.inventory_open() {
            self.draw_held_stack(settings.value.video.ui_scale);
        }
        action
    }

    fn hud(&self, layout: &crate::inventory::InventoryLayout, unlimited: bool) {
        align(Alignment::CENTER, || {
            colored_box(Color::WHITE.with_alpha(0.9), [3.0, 15.0]);
            colored_box(Color::WHITE.with_alpha(0.9), [15.0, 3.0]);
        });

        align(Alignment::BOTTOM_CENTER, || {
            let mut hotbar_padding = Pad::ZERO;
            hotbar_padding.bottom = 14.0;
            pad(hotbar_padding, || {
                let mut list = List::column();
                list.item_spacing = 5.0;
                list.cross_axis_alignment = CrossAxisAlignment::Center;
                list.main_axis_size = MainAxisSize::Min;
                list.show(|| {
                    if let Some(material) = layout.selected_stack().material {
                        colored_box_container(Color::BLACK.with_alpha(0.72), || {
                            pad(Pad::balanced(10.0, 4.0), || {
                                label(material_name(material));
                            });
                        });
                    }
                    self.hotbar(layout, unlimited, false, 42.0);
                });
            });
        });
    }

    fn debug_overlay(&self, sim: &Sim) {
        let coordinates = sim.debug_coordinates();
        align(Alignment::TOP_LEFT, || {
            pad(Pad::all(12.0), || {
                colored_box_container(Color::BLACK.with_alpha(0.78), || {
                    pad(Pad::all(10.0), || {
                        let mut list = List::column();
                        list.item_spacing = 3.0;
                        list.main_axis_size = MainAxisSize::Min;
                        list.show(|| {
                            text(19.0, "Hypermine Developer (H^3)");
                            if let Some(coordinates) = coordinates {
                                let high = coordinates.node_hash >> 64;
                                let low = coordinates.node_hash as u64;
                                label(format!("Cell: {high:016x}:{low:016x}"));
                                label(format!(
                                    "Cell depth from spawn: {} dodecahedra",
                                    coordinates.node_depth
                                ));
                                label(format!(
                                    "Klein local:  x {:+.5}   y {:+.5}   z {:+.5}",
                                    coordinates.klein[0],
                                    coordinates.klein[1],
                                    coordinates.klein[2]
                                ));
                                label(format!(
                                    "Lorentz H^3: ({:+.5}, {:+.5}, {:+.5}, {:+.5})",
                                    coordinates.lorentz[0],
                                    coordinates.lorentz[1],
                                    coordinates.lorentz[2],
                                    coordinates.lorentz[3]
                                ));
                                label(format!(
                                    "Local hyperbolic radius: {:.5}",
                                    coordinates.local_hyperbolic_radius
                                ));
                            } else {
                                label("Coordinates are numerically unstable at this position.");
                                label("Use Creative Inventory > Return to Spawn to recover.");
                            }
                            label("Position = cell address + local H^3 coordinates");
                            label("F3 - hide developer overlay");
                        });
                    });
                });
            });
        });
    }

    fn hotbar(
        &self,
        layout: &crate::inventory::InventoryLayout,
        unlimited: bool,
        interactive: bool,
        size: f32,
    ) -> Option<usize> {
        let mut clicked = None;
        let mut hotbar = List::row();
        hotbar.item_spacing = 2.0;
        hotbar.main_axis_size = MainAxisSize::Min;
        hotbar.show(|| {
            for index in 0..HOTBAR_SLOTS {
                let response = item_slot(
                    &self.icons,
                    layout.slots[index],
                    layout.selected_hotbar == index,
                    size,
                    Some(index + 1),
                    unlimited,
                );
                if interactive && response.clicked {
                    clicked = Some(index);
                }
            }
        });
        clicked
    }

    fn inventory_menu(
        &mut self,
        sim: Option<&mut Sim>,
        settings: &mut SettingsStore,
        world_id: &str,
        next: &mut Option<MenuScreen>,
    ) {
        let mut layout = settings.value.inventory.layout(world_id);
        let counts = sim
            .as_deref()
            .map(Sim::inventory_material_counts)
            .unwrap_or([0; Material::COUNT]);
        let mut unlimited = sim
            .as_deref()
            .is_none_or(|sim| !sim.cfg.gameplay_enabled || layout.creative_tab);
        layout.reconcile(&counts, unlimited, self.held_stack);

        let icons = self.icons.clone();
        let mut held = self.held_stack;
        let mut hovered = None;
        let mut changed = false;
        let mut close = false;
        let mut return_to_spawn = false;
        let slot_size = (46.0 / settings.value.video.ui_scale.sqrt()).clamp(34.0, 52.0);
        inventory_panel(
            if layout.creative_tab {
                "Creative Inventory"
            } else {
                "Inventory"
            },
            || {
                row(|| {
                    if menu_button(if layout.creative_tab {
                        "Survival Inventory"
                    } else {
                        "Survival Inventory [active]"
                    }) {
                        layout.creative_tab = false;
                        unlimited = sim.as_deref().is_none_or(|sim| !sim.cfg.gameplay_enabled);
                        changed = true;
                    }
                    if menu_button(if layout.creative_tab {
                        "Creative Inventory [active]"
                    } else {
                        "Creative Inventory"
                    }) {
                        layout.creative_tab = true;
                        unlimited = true;
                        changed = true;
                    }
                });

                if layout.creative_tab {
                    label("All Blocks");
                    slot_grid(5, 9, slot_size, |index| {
                        let material = Material::VALUES.get(index + 1).copied();
                        let stack = material
                            .map(|material| ItemStack::new(material, STACK_LIMIT))
                            .unwrap_or(ItemStack::EMPTY);
                        let response = item_slot(&icons, stack, false, slot_size, None, true);
                        if response.hovering {
                            hovered = material;
                        }
                        if response.clicked
                            && let Some(material) = material
                        {
                            held = Some(ItemStack::new(material, STACK_LIMIT));
                            changed = true;
                        }
                    });
                    if menu_button("Return to Spawn") {
                        return_to_spawn = true;
                        close = true;
                    }
                } else {
                    label("Storage");
                    slot_grid(3, 9, slot_size, |grid_index| {
                        let index = HOTBAR_SLOTS + grid_index;
                        let stack = layout.slots[index];
                        let response = item_slot(&icons, stack, false, slot_size, None, false);
                        if response.hovering {
                            hovered = stack.material;
                        }
                        if response.clicked {
                            layout.left_click_slot(index, &mut held, false);
                            changed = true;
                        }
                    });
                }

                label("Hotbar — select with 1–9 or the mouse wheel");
                slot_grid(1, HOTBAR_SLOTS, slot_size, |index| {
                    let stack = layout.slots[index];
                    let response = item_slot(
                        &icons,
                        stack,
                        layout.selected_hotbar == index,
                        slot_size,
                        Some(index + 1),
                        unlimited,
                    );
                    if response.hovering {
                        hovered = stack.material;
                    }
                    if response.clicked {
                        if held.is_none() {
                            layout.select_hotbar(index);
                        } else {
                            layout.left_click_slot(index, &mut held, unlimited);
                        }
                        changed = true;
                    }
                });

                label(
                    hovered
                        .map(material_name)
                        .unwrap_or_else(|| "Move stacks with the left mouse button".to_owned()),
                );
                if menu_button("Done") {
                    close = true;
                }
            },
        );

        self.held_stack = held;
        self.hovered_material = hovered;
        if close {
            layout.return_held(&mut self.held_stack, unlimited);
            *next = Some(MenuScreen::Closed);
            changed = true;
        }
        if let Some(sim) = sim {
            sim.set_creative_mode(layout.creative_tab);
            sim.set_selected_material(layout.selected_stack().material.unwrap_or(Material::Void));
            if return_to_spawn {
                sim.request_return_to_spawn();
            }
        }
        settings
            .value
            .inventory
            .worlds
            .insert(world_id.to_owned(), layout);
        if changed {
            settings.save();
        }
    }

    fn draw_held_stack(&self, ui_scale: f32) {
        let Some(held_stack) = self.held_stack.filter(|stack| !stack.is_empty()) else {
            return;
        };
        let Some(material) = held_stack.material else {
            return;
        };
        let position = [
            self.cursor_position[0] / ui_scale - 20.0,
            self.cursor_position[1] / ui_scale - 20.0,
        ];
        align(Alignment::TOP_LEFT, || {
            offset(position.into(), || {
                stack(|| {
                    colored_box(Color::BLACK.with_alpha(0.72), [40.0, 40.0]);
                    if let Some(icon) = self.icons.get(material) {
                        align(Alignment::CENTER, || {
                            image(icon, [32.0, 32.0]);
                        });
                    }
                    align(Alignment::BOTTOM_RIGHT, || {
                        pad(Pad::all(2.0), || {
                            label(held_stack.count.to_string());
                        });
                    });
                });
            });
        });
    }

    fn pause_menu(
        &mut self,
        worlds: &WorldManager,
        next: &mut Option<MenuScreen>,
        action: &mut GuiAction,
    ) {
        menu_panel("Game Menu", || {
            label(format!("World: {}", worlds.selected_name()));
            if menu_button("Resume") {
                *next = Some(MenuScreen::Closed);
            }
            if menu_button("Options...") {
                *next = Some(MenuScreen::Options);
            }
            if menu_button("Worlds...") {
                *next = Some(MenuScreen::Worlds);
            }
            if menu_button("Quit Game") {
                action.quit = true;
            }
        });
    }

    fn options_menu(&mut self, next: &mut Option<MenuScreen>) {
        menu_panel("Options", || {
            if menu_button("Video Settings...") {
                *next = Some(MenuScreen::Video);
            }
            if menu_button("Controls...") {
                *next = Some(MenuScreen::Controls);
            }
            if menu_button("Done") {
                *next = Some(MenuScreen::Pause);
            }
        });
    }

    fn video_menu(
        &mut self,
        settings: &mut SettingsStore,
        next: &mut Option<MenuScreen>,
        action: &mut GuiAction,
    ) {
        let video = &mut settings.value.video;
        let mut changed = false;
        menu_panel("Video Settings", || {
            label(format!("Render Distance: {:.0} m", video.view_distance_m));
            if let Some(value) = slider(video.view_distance_m as f64, 30.0, 110.0).value {
                video.view_distance_m = (value as f32 / 5.0).round() * 5.0;
                changed = true;
            }

            label(format!("Field of View: {:.0}°", video.fov_degrees));
            if let Some(value) = slider(video.fov_degrees as f64, 50.0, 110.0).value {
                video.fov_degrees = value.round() as f32;
                changed = true;
            }

            label(format!("GUI Scale: {:.0}%", video.ui_scale * 100.0));
            if let Some(value) = slider(video.ui_scale as f64, 0.75, 1.75).value {
                video.ui_scale = (value as f32 * 20.0).round() / 20.0;
                changed = true;
            }

            toggle_row("Fullscreen", &mut video.fullscreen, &mut changed);
            toggle_row("VSync", &mut video.vsync, &mut changed);

            label(format!(
                "Windowed Resolution: {} × {}",
                video.width, video.height
            ));
            let resolutions = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];
            if menu_button("Next Resolution") {
                let current = resolutions
                    .iter()
                    .position(|&(width, height)| width == video.width && height == video.height)
                    .unwrap_or(0);
                (video.width, video.height) = resolutions[(current + 1) % resolutions.len()];
                changed = true;
            }

            if menu_button("Done") {
                *next = Some(MenuScreen::Options);
            }
        });
        if changed {
            settings.save();
            action.video_changed = true;
        }
    }

    fn controls_menu(&mut self, settings: &mut SettingsStore, next: &mut Option<MenuScreen>) {
        let mut changed = false;
        menu_panel("Controls", || {
            label(format!(
                "Mouse Sensitivity: {:.0}%",
                settings.value.controls.mouse_sensitivity * 100.0
            ));
            if let Some(value) =
                slider(settings.value.controls.mouse_sensitivity as f64, 0.2, 3.0).value
            {
                settings.value.controls.mouse_sensitivity = (value as f32 * 20.0).round() / 20.0;
                changed = true;
            }
            toggle_row(
                "Invert Mouse Y",
                &mut settings.value.controls.invert_y,
                &mut changed,
            );
            label("Mouse: Left — Break Block  |  Right — Place Block");
            label("Escape — Pause / Back (reserved)");
            if menu_button("Key Bindings...") {
                *next = Some(MenuScreen::Bindings);
            }
            if menu_button("Done") {
                *next = Some(MenuScreen::Options);
            }
        });
        if changed {
            settings.save();
        }
    }

    fn bindings_menu(&mut self, settings: &mut SettingsStore, next: &mut Option<MenuScreen>) {
        const PER_PAGE: usize = 6;
        let page_count = Action::ALL.len().div_ceil(PER_PAGE);
        self.control_page = self.control_page.min(page_count - 1);
        let start = self.control_page * PER_PAGE;
        let end = (start + PER_PAGE).min(Action::ALL.len());
        let mut changed = false;
        menu_panel("Key Bindings", || {
            label("Click a binding, then press its new key.");
            label(format!("Page {} of {page_count}", self.control_page + 1));
            for binding_action in Action::ALL[start..end].iter().copied() {
                let key = if self.rebinding == Some(binding_action) {
                    "Press a key...".to_owned()
                } else {
                    display_key(settings.value.controls.binding(binding_action))
                };
                if menu_button(format!("{}: {key}", binding_action.label())) {
                    self.rebinding = Some(binding_action);
                }
            }
            if menu_button("Previous Bindings") && self.control_page > 0 {
                self.control_page -= 1;
            }
            if menu_button("Next Bindings") && self.control_page + 1 < page_count {
                self.control_page += 1;
            }
            if menu_button("Reset All Controls") {
                settings.value.controls.reset();
                self.rebinding = None;
                changed = true;
            }
            if menu_button("Done") {
                *next = Some(MenuScreen::Controls);
            }
        });
        if changed {
            settings.save();
        }
    }

    fn worlds_menu(
        &mut self,
        worlds: &mut WorldManager,
        next: &mut Option<MenuScreen>,
        action: &mut GuiAction,
    ) {
        let entries = worlds.worlds().to_vec();
        let selected = worlds.selected_id().to_owned();
        const PER_PAGE: usize = 4;
        let page_count = entries.len().div_ceil(PER_PAGE).max(1);
        self.world_page = self.world_page.min(page_count - 1);
        let start = self.world_page * PER_PAGE;
        menu_panel("Worlds", || {
            label("Each world has its own save and generated terrain.");
            label(format!(
                "World List — Page {} of {page_count}",
                self.world_page + 1
            ));
            for index in 0..PER_PAGE {
                if let Some(world) = entries.get(start + index) {
                    let marker = if world.id == selected {
                        "  [Current]"
                    } else {
                        ""
                    };
                    if menu_button(format!("{}{}", world.name, marker)) && world.id != selected {
                        match worlds.select(&world.id) {
                            Ok(()) => action.restart = true,
                            Err(error) => self.status = Some(error.to_string()),
                        }
                    }
                } else {
                    let _ = menu_button("— empty slot —");
                }
            }
            let previous_label = if self.world_page == 0 {
                "Previous Worlds (first page)"
            } else {
                "Previous Worlds"
            };
            if menu_button(previous_label) && self.world_page > 0 {
                self.world_page -= 1;
            }
            let next_label = if self.world_page + 1 == page_count {
                "Next Worlds (last page)"
            } else {
                "Next Worlds"
            };
            if menu_button(next_label) && self.world_page + 1 < page_count {
                self.world_page += 1;
            }
            if menu_button("Create New World...") {
                *next = Some(MenuScreen::CreateWorld);
            }
            if menu_button("Back") {
                *next = Some(MenuScreen::Pause);
            }
            if let Some(status) = &self.status {
                label(status.clone());
            }
        });
    }

    fn create_world_menu(
        &mut self,
        worlds: &mut WorldManager,
        next: &mut Option<MenuScreen>,
        action: &mut GuiAction,
    ) {
        menu_panel("Create New World", || {
            label("World Name");
            let response = textbox(self.new_world_name.clone());
            if let Some(name) = response.text.as_ref() {
                self.new_world_name = name.clone();
            }
            if (response.activated || menu_button("Create World"))
                && !self.new_world_name.trim().is_empty()
            {
                match worlds.create(&self.new_world_name) {
                    Ok(_) => action.restart = true,
                    Err(error) => self.status = Some(error.to_string()),
                }
            }
            if menu_button("Cancel") {
                *next = Some(MenuScreen::Worlds);
            }
            if let Some(status) = &self.status {
                label(status.clone());
            }
        });
    }
}

fn menu_panel(title: &str, children: impl FnOnce()) {
    align(Alignment::CENTER, || {
        constrained(
            Constraints {
                min: [440.0, 0.0].into(),
                max: [620.0, 650.0].into(),
            },
            || {
                colored_box_container(Color::rgba(26, 26, 31, 245), || {
                    pad(Pad::all(18.0), || {
                        let mut list = List::column();
                        list.item_spacing = 8.0;
                        list.main_axis_size = MainAxisSize::Min;
                        list.cross_axis_alignment = CrossAxisAlignment::Stretch;
                        list.show(|| {
                            text(30.0, title.to_owned());
                            children();
                        });
                    });
                });
            },
        );
    });
}

fn menu_button(text: impl Into<std::borrow::Cow<'static, str>>) -> bool {
    let mut button = Button::styled(text);
    button.padding = Pad::balanced(18.0, 8.0);
    button.show().clicked
}

fn toggle_row(label_text: &str, value: &mut bool, changed: &mut bool) {
    if menu_button(format!(
        "{label_text}: {}",
        if *value { "On" } else { "Off" }
    )) {
        *value = !*value;
        *changed = true;
    }
}

#[derive(Default)]
struct SlotResponse {
    clicked: bool,
    hovering: bool,
}

fn item_slot(
    icons: &MaterialIcons,
    item_stack: ItemStack,
    selected: bool,
    size: f32,
    number: Option<usize>,
    unlimited: bool,
) -> SlotResponse {
    let mut result = SlotResponse::default();
    constrained(
        Constraints {
            min: [size, size].into(),
            max: [size, size].into(),
        },
        || {
            stack(|| {
                let mut button = Button::styled("");
                button.padding = Pad::ZERO;
                button.border_radius = 2.0;
                button.style.fill = if selected {
                    Color::rgba(205, 205, 218, 250)
                } else {
                    Color::rgba(54, 54, 61, 248)
                };
                button.hover_style.fill = Color::rgba(104, 104, 118, 250);
                button.down_style.fill = Color::rgba(36, 36, 42, 250);
                let response = button.show();
                result.clicked = response.clicked;
                result.hovering = response.hovering;

                if let Some(material) = item_stack.material
                    && let Some(icon) = icons.get(material)
                {
                    align(Alignment::CENTER, || {
                        image(icon, [size * 0.68, size * 0.68]);
                    });
                }
                if let Some(number) = number {
                    align(Alignment::TOP_LEFT, || {
                        pad(Pad::all(2.0), || {
                            text(11.0, number.to_string());
                        });
                    });
                }
                if !item_stack.is_empty() {
                    align(Alignment::BOTTOM_RIGHT, || {
                        pad(Pad::all(2.0), || {
                            text(
                                12.0,
                                if unlimited {
                                    "∞".to_owned()
                                } else {
                                    item_stack.count.to_string()
                                },
                            );
                        });
                    });
                }
            });
        },
    );
    result
}

fn slot_grid(rows: usize, columns: usize, size: f32, mut show_slot: impl FnMut(usize)) {
    for row_index in 0..rows {
        let dimensions = [
            columns as f32 * size + columns.saturating_sub(1) as f32 * 2.0,
            size,
        ];
        constrained(
            Constraints {
                min: dimensions.into(),
                max: dimensions.into(),
            },
            || {
                let mut row = List::row();
                row.item_spacing = 2.0;
                row.main_axis_size = MainAxisSize::Min;
                row.show(|| {
                    for column_index in 0..columns {
                        show_slot(row_index * columns + column_index);
                    }
                });
            },
        );
    }
}

fn inventory_panel(title: &str, children: impl FnOnce()) {
    align(Alignment::CENTER, || {
        constrained(
            Constraints {
                min: [520.0, 0.0].into(),
                max: [620.0, 650.0].into(),
            },
            || {
                colored_box_container(Color::rgba(26, 26, 31, 248), || {
                    pad(Pad::all(14.0), || {
                        let mut list = List::column();
                        list.item_spacing = 6.0;
                        list.main_axis_size = MainAxisSize::Min;
                        list.cross_axis_alignment = CrossAxisAlignment::Stretch;
                        list.show(|| {
                            text(28.0, title.to_owned());
                            children();
                        });
                    });
                });
            },
        );
    });
}

fn material_name(material: Material) -> String {
    let raw = format!("{material:?}");
    let mut result = String::with_capacity(raw.len() + 4);
    let mut previous_was_lowercase = false;
    for character in raw.chars() {
        if character.is_uppercase() && previous_was_lowercase {
            result.push(' ');
        }
        previous_was_lowercase = character.is_lowercase();
        result.push(character);
    }
    result
}

#[allow(dead_code)]
fn _settings_type_anchor(_: &UserSettings) {}
