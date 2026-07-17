#![allow(clippy::new_without_default)]
#![allow(clippy::needless_borrowed_reference)]

macro_rules! cstr {
    ($x:literal) => {{
        #[allow(unused_unsafe)]
        unsafe {
            std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($x, "\0").as_bytes())
        }
    }};
}

extern crate nalgebra as na;
mod config;
pub mod graphics;
pub mod inventory;
mod lahar_deprecated;
mod loader;
mod local_character_controller;
pub mod metrics;
pub mod net;
mod prediction;
pub mod settings;
pub mod sim;
mod worldgen_driver;
pub mod worlds;

pub use config::Config;
pub use settings::{Action, SettingsStore};
pub use sim::Sim;
pub use worlds::WorldManager;

use loader::{Asset, Loader};
