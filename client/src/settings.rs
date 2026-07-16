use std::{collections::BTreeMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use common::Anonymize;

use crate::inventory::InventorySettings;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub video: VideoSettings,
    pub controls: ControlSettings,
    pub inventory: InventorySettings,
}

impl UserSettings {
    fn sanitize(&mut self) {
        let defaults = Self::default();
        // F3 used to open the developer overlay. Move that untouched legacy default to F4 so
        // existing players receive the new player-facing F3 screen without a key conflict.
        if !self
            .controls
            .bindings
            .contains_key(Action::PlayerNavigation.id())
            && self
                .controls
                .bindings
                .get(Action::DeveloperOverlay.id())
                .map(String::as_str)
                == Some("F3")
        {
            self.controls.set_binding(Action::DeveloperOverlay, "F4");
        }
        self.video.width = self.video.width.clamp(800, 3840);
        self.video.height = self.video.height.clamp(480, 2160);
        clamp_finite(
            &mut self.video.fov_degrees,
            50.0,
            110.0,
            defaults.video.fov_degrees,
        );
        clamp_finite(
            &mut self.video.view_distance_m,
            30.0,
            110.0,
            defaults.video.view_distance_m,
        );
        clamp_finite(
            &mut self.video.ui_scale,
            0.75,
            1.75,
            defaults.video.ui_scale,
        );
        clamp_finite(
            &mut self.controls.mouse_sensitivity,
            0.2,
            3.0,
            defaults.controls.mouse_sensitivity,
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    /// Horizontal/vertical window size when not fullscreen.
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub vsync: bool,
    pub fov_degrees: f32,
    /// Render distance expressed in the meter scale used by configuration files.
    pub view_distance_m: f32,
    pub ui_scale: f32,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fullscreen: false,
            vsync: true,
            fov_degrees: 72.0,
            view_distance_m: 75.0,
            ui_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlSettings {
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    pub bindings: BTreeMap<String, String>,
}

impl Default for ControlSettings {
    fn default() -> Self {
        let bindings = Action::ALL
            .into_iter()
            .map(|action| (action.id().to_owned(), action.default_key().to_owned()))
            .collect();
        Self {
            mouse_sensitivity: 1.0,
            invert_y: false,
            bindings,
        }
    }
}

impl ControlSettings {
    pub fn binding(&self, action: Action) -> &str {
        self.bindings
            .get(action.id())
            .map(String::as_str)
            .unwrap_or_else(|| action.default_key())
    }

    pub fn set_binding(&mut self, action: Action, key: impl Into<String>) {
        self.bindings.insert(action.id().to_owned(), key.into());
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    ToggleNoClip,
    MoveUp,
    MoveDown,
    RollLeft,
    RollRight,
    OpenInventory,
    ToggleHud,
    PlayerNavigation,
    DeveloperOverlay,
    GuideHome,
    PreviousMaterial,
    NextMaterial,
    PickMaterial,
    Material1,
    Material2,
    Material3,
    Material4,
    Material5,
    Material6,
    Material7,
    Material8,
    Material9,
}

impl Action {
    pub const ALL: [Self; 27] = [
        Self::Forward,
        Self::Backward,
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::ToggleNoClip,
        Self::MoveUp,
        Self::MoveDown,
        Self::RollLeft,
        Self::RollRight,
        Self::OpenInventory,
        Self::ToggleHud,
        Self::PlayerNavigation,
        Self::DeveloperOverlay,
        Self::GuideHome,
        Self::PreviousMaterial,
        Self::NextMaterial,
        Self::PickMaterial,
        Self::Material1,
        Self::Material2,
        Self::Material3,
        Self::Material4,
        Self::Material5,
        Self::Material6,
        Self::Material7,
        Self::Material8,
        Self::Material9,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
            Self::Left => "left",
            Self::Right => "right",
            Self::Jump => "jump",
            Self::ToggleNoClip => "toggle_no_clip",
            Self::MoveUp => "move_up",
            Self::MoveDown => "move_down",
            Self::RollLeft => "roll_left",
            Self::RollRight => "roll_right",
            Self::OpenInventory => "open_inventory",
            Self::ToggleHud => "toggle_hud",
            Self::PlayerNavigation => "player_navigation",
            Self::DeveloperOverlay => "developer_overlay",
            Self::GuideHome => "guide_home",
            Self::PreviousMaterial => "previous_material",
            Self::NextMaterial => "next_material",
            Self::PickMaterial => "pick_material",
            Self::Material1 => "material_1",
            Self::Material2 => "material_2",
            Self::Material3 => "material_3",
            Self::Material4 => "material_4",
            Self::Material5 => "material_5",
            Self::Material6 => "material_6",
            Self::Material7 => "material_7",
            Self::Material8 => "material_8",
            Self::Material9 => "material_9",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Forward => "Move Forward",
            Self::Backward => "Move Backward",
            Self::Left => "Strafe Left",
            Self::Right => "Strafe Right",
            Self::Jump => "Jump",
            Self::ToggleNoClip => "Toggle No-clip",
            Self::MoveUp => "Fly Up",
            Self::MoveDown => "Fly Down",
            Self::RollLeft => "Roll Left",
            Self::RollRight => "Roll Right",
            Self::OpenInventory => "Open Inventory",
            Self::ToggleHud => "Toggle HUD",
            Self::PlayerNavigation => "Location & Navigation",
            Self::DeveloperOverlay => "Developer Coordinates",
            Self::GuideHome => "Guide Home",
            Self::PreviousMaterial => "Previous Hotbar Slot",
            Self::NextMaterial => "Next Hotbar Slot",
            Self::PickMaterial => "Pick Looked-at Material",
            Self::Material1 => "Hotbar Slot 1",
            Self::Material2 => "Hotbar Slot 2",
            Self::Material3 => "Hotbar Slot 3",
            Self::Material4 => "Hotbar Slot 4",
            Self::Material5 => "Hotbar Slot 5",
            Self::Material6 => "Hotbar Slot 6",
            Self::Material7 => "Hotbar Slot 7",
            Self::Material8 => "Hotbar Slot 8",
            Self::Material9 => "Hotbar Slot 9",
        }
    }

    pub fn default_key(self) -> &'static str {
        match self {
            Self::Forward => "KeyW",
            Self::Backward => "KeyS",
            Self::Left => "KeyA",
            Self::Right => "KeyD",
            Self::Jump => "Space",
            Self::ToggleNoClip => "KeyV",
            Self::MoveUp => "KeyR",
            Self::MoveDown => "KeyF",
            Self::RollLeft => "KeyQ",
            Self::RollRight => "KeyE",
            Self::OpenInventory => "KeyI",
            Self::ToggleHud => "F1",
            Self::PlayerNavigation => "F3",
            Self::DeveloperOverlay => "F4",
            Self::GuideHome => "KeyH",
            Self::PreviousMaterial => "Minus",
            Self::NextMaterial => "Equal",
            Self::PickMaterial => "KeyG",
            Self::Material1 => "Digit1",
            Self::Material2 => "Digit2",
            Self::Material3 => "Digit3",
            Self::Material4 => "Digit4",
            Self::Material5 => "Digit5",
            Self::Material6 => "Digit6",
            Self::Material7 => "Digit7",
            Self::Material8 => "Digit8",
            Self::Material9 => "Digit9",
        }
    }

    pub fn material_index(self) -> Option<usize> {
        match self {
            Self::Material1 => Some(0),
            Self::Material2 => Some(1),
            Self::Material3 => Some(2),
            Self::Material4 => Some(3),
            Self::Material5 => Some(4),
            Self::Material6 => Some(5),
            Self::Material7 => Some(6),
            Self::Material8 => Some(7),
            Self::Material9 => Some(8),
            _ => None,
        }
    }
}

pub struct SettingsStore {
    pub value: UserSettings,
    path: PathBuf,
}

impl SettingsStore {
    pub fn load(dirs: &directories::ProjectDirs) -> Self {
        let path = dirs.config_dir().join("settings.toml");
        let mut value = match fs::read_to_string(&path) {
            Ok(data) => match toml::from_str(&data) {
                Ok(settings) => settings,
                Err(error) => {
                    warn!("couldn't parse settings: {error}");
                    UserSettings::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => UserSettings::default(),
            Err(error) => {
                warn!("couldn't read settings: {error}");
                UserSettings::default()
            }
        };
        value.sanitize();
        Self { value, path }
    }

    pub fn save(&self) {
        let Some(parent) = self.path.parent() else {
            return;
        };
        if let Err(error) = fs::create_dir_all(parent)
            .and_then(|()| toml::to_string_pretty(&self.value).map_err(std::io::Error::other))
            .and_then(|data| fs::write(&self.path, data))
        {
            warn!("couldn't save settings: {error}");
        } else {
            info!(path = %self.path.anonymize().display(), "saved settings");
        }
    }
}

fn clamp_finite(value: &mut f32, min: f32, max: f32, fallback: f32) {
    *value = if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    };
}

pub fn display_key(key: &str) -> String {
    key.strip_prefix("Key")
        .or_else(|| key.strip_prefix("Digit"))
        .unwrap_or(key)
        .replace("Arrow", "Arrow ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_f3_developer_binding() {
        let mut settings = UserSettings::default();
        settings
            .controls
            .bindings
            .remove(Action::PlayerNavigation.id());
        settings
            .controls
            .set_binding(Action::DeveloperOverlay, "F3");

        settings.sanitize();

        assert_eq!(settings.controls.binding(Action::PlayerNavigation), "F3");
        assert_eq!(settings.controls.binding(Action::DeveloperOverlay), "F4");
        assert_eq!(settings.controls.binding(Action::GuideHome), "KeyH");
    }
}
