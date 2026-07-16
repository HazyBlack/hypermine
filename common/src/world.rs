use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MaterialClass {
    Empty,
    Soil,
    Rock,
    Liquid,
    Organic,
    Constructed,
    Ice,
}

#[derive(Debug, Copy, Clone)]
pub struct MaterialDefinition {
    pub material: Material,
    pub key: &'static str,
    pub display_name: &'static str,
    pub texture_file: Option<&'static str>,
    pub class: MaterialClass,
}

#[derive(
    Debug, Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
#[repr(u16)]
pub enum Material {
    #[default]
    Void = 0,
    Dirt = 1,
    Sand = 2,
    Silt = 3,
    Clay = 4,
    Mud = 5,
    SandyLoam = 6,
    SiltyLoam = 7,
    ClayLoam = 8,
    RedSand = 9,
    Limestone = 10,
    Shale = 11,
    Dolomite = 12,
    Sandstone = 13,
    RedSandstone = 14,
    Marble = 15,
    Slate = 16,
    Granite = 17,
    Diorite = 18,
    Andesite = 19,
    Gabbro = 20,
    Basalt = 21,
    Olivine = 22,
    Water = 23,
    Lava = 24,
    Wood = 25,
    Leaves = 26,
    WoodPlanks = 27,
    GreyBrick = 28,
    WhiteBrick = 29,
    Ice = 30,
    IceSlush = 31,
    Gravel = 32,
    Snow = 33,
    CoarseGrass = 34,
    TanGrass = 35,
    LushGrass = 36,
    MudGrass = 37,
    Grass = 38,
    CaveGrass = 39,
}

impl Material {
    pub const COUNT: usize = 40;

    pub const VALUES: [Self; Self::COUNT] = [
        Material::Void,
        Material::Dirt,
        Material::Sand,
        Material::Silt,
        Material::Clay,
        Material::Mud,
        Material::SandyLoam,
        Material::SiltyLoam,
        Material::ClayLoam,
        Material::RedSand,
        Material::Limestone,
        Material::Shale,
        Material::Dolomite,
        Material::Sandstone,
        Material::RedSandstone,
        Material::Marble,
        Material::Slate,
        Material::Granite,
        Material::Diorite,
        Material::Andesite,
        Material::Gabbro,
        Material::Basalt,
        Material::Olivine,
        Material::Water,
        Material::Lava,
        Material::Wood,
        Material::Leaves,
        Material::WoodPlanks,
        Material::GreyBrick,
        Material::WhiteBrick,
        Material::Ice,
        Material::IceSlush,
        Material::Gravel,
        Material::Snow,
        Material::CoarseGrass,
        Material::TanGrass,
        Material::LushGrass,
        Material::MudGrass,
        Material::Grass,
        Material::CaveGrass,
    ];

    pub const DEFINITIONS: [MaterialDefinition; Self::COUNT] = [
        material_def(Self::Void, "void", "Empty", None, MaterialClass::Empty),
        material_def(
            Self::Dirt,
            "dirt",
            "Dirt",
            Some("00001_dirt.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::Sand,
            "sand",
            "Sand",
            Some("00002_sand.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::Silt,
            "silt",
            "Silt",
            Some("00003_silt.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::Clay,
            "clay",
            "Clay",
            Some("00004_clay.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::Mud,
            "mud",
            "Mud",
            Some("00005_mud.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::SandyLoam,
            "sandy_loam",
            "Sandy Loam",
            Some("00006_sandyloam.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::SiltyLoam,
            "silty_loam",
            "Silty Loam",
            Some("00007_siltyloam.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::ClayLoam,
            "clay_loam",
            "Clay Loam",
            Some("00008_clayloam.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::RedSand,
            "red_sand",
            "Red Sand",
            Some("00009_redsand.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::Limestone,
            "limestone",
            "Limestone",
            Some("00010_limestone.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Shale,
            "shale",
            "Shale",
            Some("00011_shale.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Dolomite,
            "dolomite",
            "Dolomite",
            Some("00012_dolomite.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Sandstone,
            "sandstone",
            "Sandstone",
            Some("00013_sandstone.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::RedSandstone,
            "red_sandstone",
            "Red Sandstone",
            Some("00014_redsandstone.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Marble,
            "marble",
            "Marble",
            Some("00015_marble.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Slate,
            "slate",
            "Slate",
            Some("00016_slate.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Granite,
            "granite",
            "Granite",
            Some("00017_granite.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Diorite,
            "diorite",
            "Diorite",
            Some("00018_diorite.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Andesite,
            "andesite",
            "Andesite",
            Some("00019_andesite.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Gabbro,
            "gabbro",
            "Gabbro",
            Some("00020_gabbro.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Basalt,
            "basalt",
            "Basalt",
            Some("00021_basalt.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Olivine,
            "olivine",
            "Olivine",
            Some("00022_olivine.png"),
            MaterialClass::Rock,
        ),
        material_def(
            Self::Water,
            "water",
            "Water",
            Some("00023_water.png"),
            MaterialClass::Liquid,
        ),
        material_def(
            Self::Lava,
            "lava",
            "Lava",
            Some("00024_lava.png"),
            MaterialClass::Liquid,
        ),
        material_def(
            Self::Wood,
            "wood",
            "Wood",
            Some("00025_wood.png"),
            MaterialClass::Organic,
        ),
        material_def(
            Self::Leaves,
            "leaves",
            "Leaves",
            Some("00026_leaves.png"),
            MaterialClass::Organic,
        ),
        material_def(
            Self::WoodPlanks,
            "wood_planks",
            "Wood Planks",
            Some("00027_wood_planks.png"),
            MaterialClass::Constructed,
        ),
        material_def(
            Self::GreyBrick,
            "grey_brick",
            "Grey Brick",
            Some("00028_grey_brick.png"),
            MaterialClass::Constructed,
        ),
        material_def(
            Self::WhiteBrick,
            "white_brick",
            "White Brick",
            Some("00029_white_brick.png"),
            MaterialClass::Constructed,
        ),
        material_def(
            Self::Ice,
            "ice",
            "Ice",
            Some("00030_ice.png"),
            MaterialClass::Ice,
        ),
        material_def(
            Self::IceSlush,
            "ice_slush",
            "Ice Slush",
            Some("00031_iceslush.png"),
            MaterialClass::Ice,
        ),
        material_def(
            Self::Gravel,
            "gravel",
            "Gravel",
            Some("00032_gravel.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::Snow,
            "snow",
            "Snow",
            Some("00033_snow.png"),
            MaterialClass::Soil,
        ),
        material_def(
            Self::CoarseGrass,
            "coarse_grass",
            "Coarse Grass",
            Some("00034_coarsegrass.png"),
            MaterialClass::Organic,
        ),
        material_def(
            Self::TanGrass,
            "tan_grass",
            "Tan Grass",
            Some("00035_tangrass.png"),
            MaterialClass::Organic,
        ),
        material_def(
            Self::LushGrass,
            "lush_grass",
            "Lush Grass",
            Some("00036_lushgrass.png"),
            MaterialClass::Organic,
        ),
        material_def(
            Self::MudGrass,
            "mud_grass",
            "Mud Grass",
            Some("00037_mudgrass.png"),
            MaterialClass::Organic,
        ),
        material_def(
            Self::Grass,
            "grass",
            "Grass",
            Some("00038_grass.png"),
            MaterialClass::Organic,
        ),
        material_def(
            Self::CaveGrass,
            "cave_grass",
            "Cave Grass",
            Some("00039_cavegrass.png"),
            MaterialClass::Organic,
        ),
    ];

    pub const fn definition(self) -> &'static MaterialDefinition {
        &Self::DEFINITIONS[self as usize]
    }
}

const fn material_def(
    material: Material,
    key: &'static str,
    display_name: &'static str,
    texture_file: Option<&'static str>,
    class: MaterialClass,
) -> MaterialDefinition {
    MaterialDefinition {
        material,
        key,
        display_name,
        texture_file,
        class,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MaterialOutOfBounds;

impl std::fmt::Display for MaterialOutOfBounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Integer input does not represent a valid material")
    }
}

impl std::error::Error for MaterialOutOfBounds {}

impl TryFrom<u16> for Material {
    type Error = MaterialOutOfBounds;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Material::VALUES
            .get(value as usize)
            .ok_or(MaterialOutOfBounds)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::Material;

    #[test]
    fn u16_to_material_consistency_check() {
        for i in 0..Material::COUNT {
            let index = u16::try_from(i).unwrap();
            let material =
                Material::try_from(index).expect("no missing entries in try_from match statement");
            assert_eq!(index, material as u16);
        }
    }

    #[test]
    fn material_registry_has_stable_unique_entries() {
        let mut keys = HashSet::new();
        let mut textures = HashSet::new();
        for (id, definition) in Material::DEFINITIONS.iter().enumerate() {
            assert_eq!(definition.material as usize, id);
            assert!(keys.insert(definition.key));
            if let Some(texture) = definition.texture_file {
                assert!(textures.insert(texture));
            }
        }
    }
}
