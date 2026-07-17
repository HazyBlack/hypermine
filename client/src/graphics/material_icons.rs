use std::{fs::File, io::BufReader, path::Path};

use common::world::Material;
use tracing::warn;
use yakui::{
    ManagedTextureId,
    paint::{Texture, TextureFilter, TextureFormat},
};

use crate::{
    Config,
    inventory::{ItemStack, Tool},
};

#[derive(Clone)]
pub struct MaterialIcons {
    textures: [Option<ManagedTextureId>; Material::COUNT],
    admin_pick: Option<ManagedTextureId>,
}

impl MaterialIcons {
    pub fn load(yak: &mut yakui::Yakui, config: &Config) -> Self {
        let mut textures = [None; Material::COUNT];
        let Some(directory) = config.find_asset(Path::new("materials")) else {
            warn!("material icon directory was not found");
            return Self {
                textures,
                admin_pick: None,
            };
        };
        for definition in Material::DEFINITIONS.iter().skip(1) {
            let Some(texture_file) = definition.texture_file else {
                continue;
            };
            let path = directory.join(texture_file);
            match load_rgba(&path) {
                Ok((width, height, pixels)) => {
                    let mut texture =
                        Texture::new(TextureFormat::Rgba8Srgb, (width, height).into(), pixels);
                    texture.min_filter = TextureFilter::Nearest;
                    texture.mag_filter = TextureFilter::Nearest;
                    textures[definition.material as usize] = Some(yak.add_texture(texture));
                }
                Err(error) => warn!(?path, %error, "couldn't load material icon"),
            }
        }
        let admin_pick = config
            .find_asset(Path::new("items/00040_admin_pick.png"))
            .and_then(|path| match load_rgba(&path) {
                Ok((width, height, pixels)) => {
                    let mut texture =
                        Texture::new(TextureFormat::Rgba8Srgb, (width, height).into(), pixels);
                    texture.min_filter = TextureFilter::Linear;
                    texture.mag_filter = TextureFilter::Linear;
                    Some(yak.add_texture(texture))
                }
                Err(error) => {
                    warn!(?path, %error, "couldn't load Admin Pick icon");
                    None
                }
            });
        Self {
            textures,
            admin_pick,
        }
    }

    pub fn get(&self, material: Material) -> Option<ManagedTextureId> {
        self.textures[material as usize]
    }

    pub fn get_stack(&self, stack: ItemStack) -> Option<ManagedTextureId> {
        if let Some(material) = stack.material {
            self.get(material)
        } else {
            match stack.tool {
                Some(Tool::AdminPick) => self.admin_pick,
                None => None,
            }
        }
    }
}

fn load_rgba(path: &Path) -> anyhow::Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    let mut reader = decoder.read_info()?;
    let output_size = reader
        .output_buffer_size()
        .unwrap_or_else(|| reader.info().width as usize * reader.info().height as usize * 4);
    let mut source = vec![0; output_size];
    let info = reader.next_frame(&mut source)?;
    let source = &source[..info.buffer_size()];
    let pixels = match info.color_type {
        png::ColorType::Rgba => source.to_vec(),
        png::ColorType::Rgb => source
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect(),
        png::ColorType::Grayscale => source
            .iter()
            .flat_map(|&value| [value, value, value, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => source
            .chunks_exact(2)
            .flat_map(|values| [values[0], values[0], values[0], values[1]])
            .collect(),
        png::ColorType::Indexed => anyhow::bail!("indexed PNG was not expanded"),
    };
    Ok((info.width, info.height, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_material_texture_exists() {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("assets/materials");
        for definition in Material::DEFINITIONS.iter().skip(1) {
            let texture = definition
                .texture_file
                .expect("non-empty material has a texture");
            assert!(
                assets.join(texture).is_file(),
                "missing texture for {}: {texture}",
                definition.key
            );
        }
    }

    #[test]
    fn admin_pick_icon_exists() {
        let asset = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("assets/items/00040_admin_pick.png");
        assert!(asset.is_file(), "missing Admin Pick icon");
    }
}
