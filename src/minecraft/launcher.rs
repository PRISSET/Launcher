use anyhow::{anyhow, Result};
use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Stdio;

use super::version::{GameVersion, ShaderQuality};

pub fn get_game_directory() -> PathBuf {
    directories::ProjectDirs::from("com", "bystep", "minecraft")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".bystep-minecraft")
        })
}

pub fn get_versioned_game_directory(version: GameVersion) -> PathBuf {
    let base_dir = get_game_directory();
    base_dir.join(version.mods_folder())
}

pub fn generate_offline_uuid(nickname: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("OfflinePlayer:{}", nickname));
    let result = hasher.finalize();
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        u32::from_be_bytes([result[0], result[1], result[2], result[3]]),
        u16::from_be_bytes([result[4], result[5]]),
        u16::from_be_bytes([result[6], result[7]]),
        u16::from_be_bytes([result[8], result[9]]),
        u64::from_be_bytes([0, 0, result[10], result[11], result[12], result[13], result[14], result[15]])
    )
}

pub fn find_java(game_dir: &Path, version: GameVersion) -> Result<PathBuf> {
    let java_version = version.java_version();
    let base_dir = get_game_directory();
    let java_dir = base_dir.join("runtime").join(format!("java-{}", java_version));
    let java_exe = java_dir.join("bin").join("java.exe");
    
    if java_exe.exists() {
        return Ok(java_exe);
    }
    
    Err(anyhow!("Java {} not found", java_version))
}

fn collect_jars(dir: &Path, jars: &mut Vec<String>) -> Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_jars(&path, jars)?;
            } else if path.extension().map_or(false, |ext| ext == "jar") {
                jars.push(path.display().to_string());
            }
        }
    }
    Ok(())
}

pub fn build_launch_command(
    game_dir: &Path,
    nickname: &str,
    ram_gb: u32,
    server_address: Option<&str>,
    version: GameVersion,
) -> Result<std::process::Command> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let mc_version = version.minecraft_version();
    let java_path = find_java(game_dir, version)?;
    
    let mut cmd = std::process::Command::new(java_path);
    
    cmd.creation_flags(CREATE_NO_WINDOW);
    
    cmd.arg(format!("-Xmx{}G", ram_gb));
    cmd.arg(format!("-Xms{}G", ram_gb.min(2)));
    cmd.arg("-XX:+UseG1GC");
    cmd.arg("-XX:+ParallelRefProcEnabled");
    cmd.arg("-XX:MaxGCPauseMillis=200");
    
    let natives_dir = game_dir.join("natives");
    fs::create_dir_all(&natives_dir)?;
    cmd.arg(format!("-Djava.library.path={}", natives_dir.display()));
    cmd.arg("-Dminecraft.launcher.brand=ByStep");
    cmd.arg("-Dminecraft.launcher.version=1.1.1");
    
    let mut classpath = Vec::new();
    let libraries_dir = game_dir.join("libraries");
    if libraries_dir.exists() {
        collect_jars(&libraries_dir, &mut classpath)?;
    }
    
    let client_jar = game_dir
        .join("versions")
        .join(mc_version)
        .join(format!("{}.jar", mc_version));
    classpath.push(client_jar.display().to_string());
    
    cmd.arg("-cp");
    cmd.arg(classpath.join(";"));
    
    let version_json_path = game_dir
        .join("versions")
        .join(mc_version)
        .join(format!("{}.json", mc_version));
    
    let asset_index_id = if version_json_path.exists() {
        let content = fs::read_to_string(&version_json_path).unwrap_or_default();
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
            info.get("assetIndex")
                .and_then(|ai| ai.get("id"))
                .and_then(|id| id.as_str())
                .unwrap_or(mc_version)
                .to_string()
        } else {
            mc_version.to_string()
        }
    } else {
        mc_version.to_string()
    };
    
    let fabric_version_id = format!("fabric-loader-{}-{}", version.fabric_loader_version(), mc_version);
    cmd.arg("net.fabricmc.loader.impl.launch.knot.KnotClient");
    
    cmd.arg("--username").arg(nickname);
    cmd.arg("--version").arg(&fabric_version_id);
    cmd.arg("--gameDir").arg(game_dir);
    cmd.arg("--assetsDir").arg(game_dir.join("assets"));
    cmd.arg("--assetIndex").arg(&asset_index_id);
    cmd.arg("--uuid").arg(generate_offline_uuid(nickname));
    cmd.arg("--accessToken").arg("0");
    cmd.arg("--userType").arg("legacy");
    
    if let Some(server) = server_address {
        if !server.is_empty() {
            let _ = create_servers_dat(game_dir, server);
            let parts: Vec<&str> = server.split(':').collect();
            cmd.arg("--server").arg(parts[0]);
            if parts.len() > 1 {
                cmd.arg("--port").arg(parts[1]);
            }
        }
    }
    
    Ok(cmd)
}

pub fn create_servers_dat(game_dir: &Path, server_address: &str) -> Result<()> {
    let servers_path = game_dir.join("servers.dat");
    
    let parts: Vec<&str> = server_address.split(':').collect();
    let ip = parts[0];
    let port = if parts.len() > 1 { parts[1] } else { "25565" };
    let full_address = format!("{}:{}", ip, port);
    
    let mut data = Vec::new();
    
    data.push(0x0A);
    data.push(0x00);
    data.push(0x00);
    
    data.push(0x09);
    let servers_name = b"servers";
    data.push(0x00);
    data.push(servers_name.len() as u8);
    data.extend_from_slice(servers_name);
    
    data.push(0x0A);
    data.extend_from_slice(&1i32.to_be_bytes());
    
    data.push(0x08);
    let name_key = b"name";
    data.push(0x00);
    data.push(name_key.len() as u8);
    data.extend_from_slice(name_key);
    let server_name = "ByStep Server";
    let name_bytes = server_name.as_bytes();
    data.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
    data.extend_from_slice(name_bytes);
    
    data.push(0x08);
    let ip_key = b"ip";
    data.push(0x00);
    data.push(ip_key.len() as u8);
    data.extend_from_slice(ip_key);
    let ip_bytes = full_address.as_bytes();
    data.extend_from_slice(&(ip_bytes.len() as u16).to_be_bytes());
    data.extend_from_slice(ip_bytes);
    
    data.push(0x01);
    let hidden_key = b"hidden";
    data.push(0x00);
    data.push(hidden_key.len() as u8);
    data.extend_from_slice(hidden_key);
    data.push(0x00);
    
    data.push(0x00);
    data.push(0x00);
    
    fs::write(&servers_path, &data)?;
    
    Ok(())
}

pub fn create_default_options(game_dir: &Path) -> Result<()> {
    let options_path = game_dir.join("options.txt");
    if options_path.exists() {
        return Ok(());
    }
    
    let options = r#"version:3465
autoJump:false
operatorItemsTab:false
autoSuggestions:true
chatColors:true
chatLinks:true
chatLinksPrompt:true
enableVsync:true
entityShadows:true
forceUnicodeFont:false
discrete_mouse_scroll:false
invertYMouse:false
realmsNotifications:true
reducedDebugInfo:false
showSubtitles:false
directionalAudio:false
touchscreen:false
fullscreen:false
bobView:true
toggleCrouch:false
toggleSprint:false
darkMojangStudiosBackground:false
hideLightningFlashes:false
mouseSensitivity:0.5
fov:0.0
screenEffectScale:1.0
fovEffectScale:1.0
darknessEffectScale:1.0
glintSpeed:0.5
glintStrength:0.75
damageTiltStrength:1.0
highContrast:false
gamma:0.5
renderDistance:12
simulationDistance:12
entityDistanceScaling:1.0
guiScale:0
particles:0
maxFps:120
graphicsMode:1
ao:true
prioritizeChunkUpdates:0
biomeBlendRadius:2
renderClouds:"true"
resourcePacks:[]
incompatibleResourcePacks:[]
lastServer:
lang:ru_ru
soundDevice:
chatVisibility:0
chatOpacity:1.0
chatLineSpacing:0.0
textBackgroundOpacity:0.5
backgroundForChatOnly:true
hideServerAddress:false
advancedItemTooltips:false
pauseOnLostFocus:true
overrideWidth:0
overrideHeight:0
chatHeightFocused:1.0
chatDelay:0.0
chatHeightUnfocused:0.4375
chatScale:1.0
chatWidth:1.0
notificationDisplayTime:1.0
mipmapLevels:4
useNativeTransport:true
mainHand:"right"
attackIndicator:1
narrator:0
tutorialStep:none
mouseWheelSensitivity:1.0
rawMouseInput:true
glDebugVerbosity:1
skipMultiplayerWarning:true
skipRealms32bitWarning:false
hideMatchedNames:true
joinedFirstServer:true
hideBundleTutorial:false
syncChunkWrites:true
showAutosaveIndicator:true
allowServerListing:true
onlyShowSecureChat:false
panoramaScrollSpeed:1.0
telemetryOptInExtra:false
onboardAccessibility:false
"#;
    
    fs::write(&options_path, options)?;
    Ok(())
}

pub fn configure_shaders(game_dir: &Path, quality: ShaderQuality, _version: GameVersion) -> Result<()> {
    let _ = create_default_options(game_dir);
    
    let iris_config_path = game_dir.join("config").join("iris.properties");
    
    if let Some(parent) = iris_config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let (shaderpack, enable_shaders) = match quality {
        ShaderQuality::Off => ("", false),
        ShaderQuality::On => ("ComplementaryUnbound_r5.6.1.zip", true),
    };
    
    let iris_config = format!(
        "shaderPack={}\nenableShaders={}\n",
        shaderpack,
        enable_shaders
    );
    
    fs::write(&iris_config_path, iris_config)?;
    
    Ok(())
}
