use yakui::{
    Alignment, Color, Constraints, CrossAxisAlignment, MainAxisSize, align, colored_box,
    colored_box_container, constrained, label, pad, slider, text, textbox,
    widgets::{Button, List, Pad},
};

use crate::{
    Action, SettingsStore, Sim, WorldManager,
    settings::{UserSettings, display_key},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuScreen {
    Closed,
    Pause,
    Options,
    Video,
    Controls,
    Bindings,
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
    screen: MenuScreen,
    rebinding: Option<Action>,
    new_world_name: String,
    status: Option<String>,
    control_page: usize,
    world_page: usize,
}

impl GuiState {
    pub fn new() -> Self {
        Self {
            show_hud: true,
            screen: MenuScreen::Closed,
            rebinding: None,
            new_world_name: "New World".to_owned(),
            status: None,
            control_page: 0,
            world_page: 0,
        }
    }

    pub fn menu_open(&self) -> bool {
        self.screen != MenuScreen::Closed
    }

    pub fn toggle_hud(&mut self) {
        self.show_hud = !self.show_hud;
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
        sim: Option<&Sim>,
        settings: &mut SettingsStore,
        worlds: &mut WorldManager,
        surface_size: [f32; 2],
    ) -> GuiAction {
        let mut action = GuiAction::default();
        if self.show_hud && !self.menu_open() {
            self.hud(sim);
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
        action
    }

    fn hud(&self, sim: Option<&Sim>) {
        align(Alignment::CENTER, || {
            colored_box(Color::WHITE.with_alpha(0.9), [3.0, 15.0]);
            colored_box(Color::WHITE.with_alpha(0.9), [15.0, 3.0]);
        });

        let Some(sim) = sim else {
            return;
        };
        align(Alignment::TOP_LEFT, || {
            pad(Pad::all(8.0), || {
                colored_box_container(Color::BLACK.with_alpha(0.7), || {
                    let material_count = if sim.cfg.gameplay_enabled {
                        sim.count_inventory_entities_matching_material(sim.selected_material())
                            .to_string()
                    } else {
                        "∞".to_owned()
                    };
                    label(format!(
                        "Selected material: {:?} (×{material_count})",
                        sim.selected_material()
                    ));
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

            label(format!("Interface Scale: {:.0}%", video.ui_scale * 100.0));
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

#[allow(dead_code)]
fn _settings_type_anchor(_: &UserSettings) {}
