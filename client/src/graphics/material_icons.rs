use std::{fs, fs::File, io::BufReader, path::Path};

use common::world::Material;
use tracing::warn;
use yakui::{
    ManagedTextureId,
    paint::{Texture, TextureFilter, TextureFormat},
};

use crate::Config;

#[derive(Clone)]
pub struct MaterialIcons {
    textures: [Option<ManagedTextureId>; Material::COUNT],
}

impl MaterialIcons {
    pub fn load(yak: &mut yakui::Yakui, config: &Config) -> Self {
        let mut textures = [None; Material::COUNT];
        let Some(directory) = config.find_asset(Path::new("materials")) else {
            warn!("material icon directory was not found");
            return Self { textures };
        };
        let Ok(entries) = fs::read_dir(directory) else {
            warn!("material icon directory could not be read");
            return Self { textures };
        };
        let mut paths = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
            .collect::<Vec<_>>();
        paths.sort();

        for (index, path) in paths.into_iter().take(Material::COUNT - 1).enumerate() {
            match load_rgba(&path) {
                Ok((width, height, pixels)) => {
                    let mut texture =
                        Texture::new(TextureFormat::Rgba8Srgb, (width, height).into(), pixels);
                    texture.min_filter = TextureFilter::Nearest;
                    texture.mag_filter = TextureFilter::Nearest;
                    textures[index + 1] = Some(yak.add_texture(texture));
                }
                Err(error) => warn!(?path, %error, "couldn't load material icon"),
            }
        }
        Self { textures }
    }

    pub fn get(&self, material: Material) -> Option<ManagedTextureId> {
        self.textures[material as usize]
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
