use std::{collections::BTreeMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use common::Anonymize;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub video: VideoSettings,
    pub controls: ControlSettings,
}

impl UserSettings {
    fn sanitize(&mut self) {
        let defaults = Self::default();
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
    ToggleHud,
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
    Material10,
}

impl Action {
    pub const ALL: [Self; 24] = [
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
        Self::ToggleHud,
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
        Self::Material10,
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
            Self::ToggleHud => "toggle_hud",
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
            Self::Material10 => "material_10",
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
            Self::ToggleHud => "Toggle HUD",
            Self::PreviousMaterial => "Previous Material",
            Self::NextMaterial => "Next Material",
            Self::PickMaterial => "Pick Looked-at Material",
            Self::Material1 => "Material Slot 1",
            Self::Material2 => "Material Slot 2",
            Self::Material3 => "Material Slot 3",
            Self::Material4 => "Material Slot 4",
            Self::Material5 => "Material Slot 5",
            Self::Material6 => "Material Slot 6",
            Self::Material7 => "Material Slot 7",
            Self::Material8 => "Material Slot 8",
            Self::Material9 => "Material Slot 9",
            Self::Material10 => "Material Slot 10",
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
            Self::ToggleHud => "F1",
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
            Self::Material10 => "Digit0",
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
            Self::Material10 => Some(9),
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
