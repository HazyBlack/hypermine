use std::{
    fs,
    path::{Component as PathComponent, PathBuf},
    thread,
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::warn;

const WORLD_REGISTRY_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntry {
    pub id: String,
    pub name: String,
    pub save: PathBuf,
    pub created_unix_seconds: u64,
    #[serde(default)]
    pub format_version: u32,
    #[serde(default)]
    pub content_registry_version: u32,
    #[serde(default)]
    pub worldgen_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct WorldRegistry {
    #[serde(default)]
    version: u32,
    selected_id: String,
    worlds: Vec<WorldEntry>,
    #[serde(default)]
    pending_deletions: Vec<PendingWorldDeletion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingWorldDeletion {
    id: String,
    save: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteWorldOutcome {
    pub restart_required: bool,
    pub files_removed: bool,
}

pub struct WorldManager {
    registry: WorldRegistry,
    registry_path: PathBuf,
    data_dir: PathBuf,
}

impl WorldManager {
    pub fn load(dirs: &directories::ProjectDirs, initial_save: PathBuf) -> Self {
        let registry_path = dirs.config_dir().join("worlds.toml");
        let data_dir = dirs.data_local_dir().to_owned();
        let registry = fs::read_to_string(&registry_path)
            .ok()
            .and_then(|data| toml::from_str(&data).ok())
            .filter(|registry: &WorldRegistry| !registry.worlds.is_empty())
            .unwrap_or_else(|| {
                let name = initial_save
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .map(title_case)
                    .unwrap_or_else(|| "My World".to_owned());
                WorldRegistry {
                    version: WORLD_REGISTRY_VERSION,
                    selected_id: "first-world".to_owned(),
                    worlds: vec![WorldEntry {
                        id: "first-world".to_owned(),
                        name,
                        save: initial_save,
                        created_unix_seconds: now(),
                        format_version: save::CURRENT_FORMAT_VERSION,
                        content_registry_version: save::CURRENT_CONTENT_REGISTRY_VERSION,
                        worldgen_version: save::CURRENT_WORLDGEN_VERSION,
                    }],
                    pending_deletions: Vec::new(),
                }
            });
        let mut manager = Self {
            registry,
            registry_path,
            data_dir,
        };
        manager.registry.version = WORLD_REGISTRY_VERSION;
        for world in &mut manager.registry.worlds {
            if world.format_version == 0 {
                world.format_version = save::CURRENT_FORMAT_VERSION;
            }
            if world.content_registry_version == 0 {
                world.content_registry_version = save::CURRENT_CONTENT_REGISTRY_VERSION;
            }
            if world.worldgen_version == 0 {
                world.worldgen_version = save::CURRENT_WORLDGEN_VERSION;
            }
        }
        if !manager
            .registry
            .worlds
            .iter()
            .any(|world| world.id == manager.registry.selected_id)
        {
            manager.registry.selected_id = manager.registry.worlds[0].id.clone();
        }
        manager.apply_pending_deletions();
        if let Err(error) = manager.save_registry() {
            warn!("couldn't save world registry: {error:#}");
        }
        manager
    }

    pub fn worlds(&self) -> &[WorldEntry] {
        &self.registry.worlds
    }

    pub fn selected_id(&self) -> &str {
        &self.registry.selected_id
    }

    pub fn selected_name(&self) -> &str {
        self.registry
            .worlds
            .iter()
            .find(|world| world.id == self.registry.selected_id)
            .map(|world| world.name.as_str())
            .unwrap_or("Unknown World")
    }

    pub fn selected_save(&self) -> PathBuf {
        let save = &self
            .registry
            .worlds
            .iter()
            .find(|world| world.id == self.registry.selected_id)
            .unwrap_or(&self.registry.worlds[0])
            .save;
        if save.is_absolute() {
            save.clone()
        } else {
            self.data_dir.join(save)
        }
    }

    pub fn select(&mut self, id: &str) -> Result<()> {
        if !self.registry.worlds.iter().any(|world| world.id == id) {
            anyhow::bail!("world does not exist");
        }
        self.registry.selected_id = id.to_owned();
        self.save_registry()
    }

    pub fn create(&mut self, name: &str) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("world name cannot be empty");
        }
        let base = slug(name);
        let mut id = base.clone();
        let mut suffix = 2;
        while self.registry.worlds.iter().any(|world| world.id == id) {
            id = format!("{base}-{suffix}");
            suffix += 1;
        }
        let save = PathBuf::from("worlds").join(&id).join("world.save");
        let world_dir = self.data_dir.join("worlds").join(&id);
        fs::create_dir_all(&world_dir).context("creating world directory")?;
        fs::write(
            world_dir.join("world.toml"),
            toml::to_string_pretty(&WorldEntry {
                id: id.clone(),
                name: name.to_owned(),
                save: save.clone(),
                created_unix_seconds: now(),
                format_version: save::CURRENT_FORMAT_VERSION,
                content_registry_version: save::CURRENT_CONTENT_REGISTRY_VERSION,
                worldgen_version: save::CURRENT_WORLDGEN_VERSION,
            })?,
        )
        .context("writing world metadata")?;
        self.registry.worlds.push(WorldEntry {
            id: id.clone(),
            name: name.to_owned(),
            save,
            created_unix_seconds: now(),
            format_version: save::CURRENT_FORMAT_VERSION,
            content_registry_version: save::CURRENT_CONTENT_REGISTRY_VERSION,
            worldgen_version: save::CURRENT_WORLDGEN_VERSION,
        });
        self.registry.selected_id = id.clone();
        self.save_registry()?;
        Ok(id)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("world name cannot be empty");
        }
        let world = self
            .registry
            .worlds
            .iter_mut()
            .find(|world| world.id == id)
            .context("world does not exist")?;
        world.name = name.to_owned();
        let world = world.clone();
        self.write_world_metadata(&world)?;
        self.save_registry()
    }

    pub fn delete(&mut self, id: &str) -> Result<DeleteWorldOutcome> {
        if self.registry.worlds.len() <= 1 {
            anyhow::bail!("create another world before deleting the last one");
        }
        let index = self
            .registry
            .worlds
            .iter()
            .position(|world| world.id == id)
            .context("world does not exist")?;
        let world = self.registry.worlds[index].clone();
        let pending = PendingWorldDeletion {
            id: world.id.clone(),
            save: world.save.clone(),
        };
        self.validate_world_deletion(&pending)?;
        let world = self.registry.worlds.remove(index);
        let restart_required = self.registry.selected_id == world.id;
        if restart_required {
            self.registry.selected_id = self.registry.worlds[0].id.clone();
        }
        self.registry.pending_deletions.push(pending.clone());
        self.save_registry()?;
        let files_removed = self.remove_world_files(&pending).is_ok();
        if files_removed {
            self.registry
                .pending_deletions
                .retain(|item| item.id != pending.id);
            self.save_registry()?;
        }
        Ok(DeleteWorldOutcome {
            restart_required,
            files_removed,
        })
    }

    fn write_world_metadata(&self, world: &WorldEntry) -> Result<()> {
        let relative = PathBuf::from("worlds").join(&world.id);
        if !world.save.starts_with(&relative) {
            return Ok(());
        }
        let directory = self.data_dir.join(relative);
        fs::create_dir_all(&directory).context("creating world directory")?;
        fs::write(directory.join("world.toml"), toml::to_string_pretty(world)?)
            .context("writing world metadata")
    }

    fn apply_pending_deletions(&mut self) {
        for attempt in 0..40 {
            let pending = self.registry.pending_deletions.clone();
            self.registry.pending_deletions = pending
                .iter()
                .filter(|item| self.remove_world_files(item).is_err())
                .cloned()
                .collect();
            if self.registry.pending_deletions.is_empty()
                || self.registry.pending_deletions.len() == pending.len() && attempt == 39
            {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        for pending in &self.registry.pending_deletions {
            warn!(world = %pending.id, "world files remain pending deletion");
        }
    }

    fn remove_world_files(&self, pending: &PendingWorldDeletion) -> Result<()> {
        self.validate_world_deletion(pending)?;
        let managed_directory = self.data_dir.join("worlds").join(&pending.id);
        let save_path = self.data_dir.join(&pending.save);
        if save_path.starts_with(&managed_directory) {
            if managed_directory.exists() {
                fs::remove_dir_all(&managed_directory).context("deleting world directory")?;
            }
        } else if save_path.exists() {
            fs::remove_file(&save_path).context("deleting legacy world save")?;
        }
        Ok(())
    }

    fn validate_world_deletion(&self, pending: &PendingWorldDeletion) -> Result<()> {
        if pending.save.is_absolute()
            || pending.save.components().any(|component| {
                matches!(
                    component,
                    PathComponent::ParentDir | PathComponent::RootDir | PathComponent::Prefix(_)
                )
            })
        {
            anyhow::bail!("refusing to delete a save outside Hypermine's managed data directory");
        }
        Ok(())
    }

    fn save_registry(&self) -> Result<()> {
        let parent = self
            .registry_path
            .parent()
            .context("world registry has no parent")?;
        fs::create_dir_all(parent).context("creating config directory")?;
        fs::write(&self.registry_path, toml::to_string_pretty(&self.registry)?)
            .context("writing world registry")
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn slug(name: &str) -> String {
    let mut slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "world".to_owned()
    } else {
        slug.chars().take(48).collect()
    }
}

fn title_case(name: &str) -> String {
    name.replace(['-', '_'], " ")
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_names_become_safe_unique_directory_names() {
        assert_eq!(slug("My Hyperbolic World!"), "my-hyperbolic-world");
        assert_eq!(slug("***"), "world");
    }

    #[test]
    fn worlds_have_independent_save_paths_and_survive_reload() {
        let root =
            std::env::temp_dir().join(format!("hypermine-world-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let dirs = directories::ProjectDirs::from_path(root.clone()).unwrap();
        let mut manager = WorldManager::load(&dirs, PathBuf::from("existing.save"));
        manager.create("Fresh World").unwrap();

        assert_eq!(manager.worlds().len(), 2);
        assert_eq!(manager.selected_name(), "Fresh World");
        assert!(
            manager
                .selected_save()
                .ends_with(PathBuf::from("worlds/fresh-world/world.save"))
        );
        assert_ne!(manager.worlds()[0].save, manager.worlds()[1].save);
        assert_eq!(
            manager.worlds()[1].format_version,
            save::CURRENT_FORMAT_VERSION
        );
        assert_eq!(
            manager.worlds()[1].content_registry_version,
            save::CURRENT_CONTENT_REGISTRY_VERSION
        );

        let reloaded = WorldManager::load(&dirs, PathBuf::from("ignored.save"));
        assert_eq!(reloaded.worlds().len(), 2);
        assert_eq!(reloaded.selected_name(), "Fresh World");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_worlds_can_be_renamed_and_deleted_safely() {
        let root = std::env::temp_dir().join(format!(
            "hypermine-world-management-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let dirs = directories::ProjectDirs::from_path(root.clone()).unwrap();
        let mut manager = WorldManager::load(&dirs, PathBuf::from("existing.save"));
        assert!(manager.delete("first-world").is_err());
        let disposable = manager.create("Disposable").unwrap();
        manager.rename(&disposable, "Renamed World").unwrap();
        assert_eq!(manager.selected_name(), "Renamed World");
        let save_path = manager.selected_save();
        fs::write(&save_path, b"test save").unwrap();
        manager.select("first-world").unwrap();

        let outcome = manager.delete(&disposable).unwrap();
        assert!(!outcome.restart_required);
        assert!(outcome.files_removed);
        assert!(!save_path.exists());
        assert!(!manager.worlds().iter().any(|world| world.id == disposable));

        let external = manager.create("External").unwrap();
        manager
            .registry
            .worlds
            .iter_mut()
            .find(|world| world.id == external)
            .unwrap()
            .save = root.join("outside.save");
        assert!(manager.delete(&external).is_err());
        assert!(manager.worlds().iter().any(|world| world.id == external));

        let reloaded = WorldManager::load(&dirs, PathBuf::from("ignored.save"));
        assert!(!reloaded.worlds().iter().any(|world| world.id == disposable));
        fs::remove_dir_all(root).unwrap();
    }
}
