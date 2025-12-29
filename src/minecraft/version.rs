use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GameVersion {
    Fabric1_20_1,
    #[default]
    Fabric1_21_1,
}

impl GameVersion {
    pub fn minecraft_version(&self) -> &'static str {
        match self {
            GameVersion::Fabric1_20_1 => "1.20.1",
            GameVersion::Fabric1_21_1 => "1.21.1",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GameVersion::Fabric1_20_1 => "1.20.1 Fabric",
            GameVersion::Fabric1_21_1 => "1.21.1 Fabric",
        }
    }

    pub fn mods_folder(&self) -> &'static str {
        match self {
            GameVersion::Fabric1_20_1 => "1.20.1-fabric",
            GameVersion::Fabric1_21_1 => "1.21.1-fabric",
        }
    }

    pub fn fabric_loader_version(&self) -> &'static str {
        match self {
            GameVersion::Fabric1_20_1 => "0.16.10",
            GameVersion::Fabric1_21_1 => "0.18.1",
        }
    }

    pub fn java_version(&self) -> u8 {
        match self {
            GameVersion::Fabric1_20_1 => 17,
            GameVersion::Fabric1_21_1 => 21,
        }
    }

    pub fn all() -> Vec<GameVersion> {
        vec![GameVersion::Fabric1_20_1, GameVersion::Fabric1_21_1]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShaderQuality {
    Off,
    #[default]
    On,
}

impl ShaderQuality {
    pub fn display_name(&self) -> &'static str {
        match self {
            ShaderQuality::Off => "Выкл",
            ShaderQuality::On => "Вкл",
        }
    }

    pub fn all() -> Vec<ShaderQuality> {
        vec![ShaderQuality::Off, ShaderQuality::On]
    }
}
