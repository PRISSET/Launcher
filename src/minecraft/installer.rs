use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::Client;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::fs;

use super::version::*;
use super::types::*;

const VERSION_MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META_URL: &str = "https://meta.fabricmc.net";
const JAVA21_URL: &str = "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.5%2B11/OpenJDK21U-jre_x64_windows_hotspot_21.0.5_11.zip";
const MODS_REPO_BASE: &str = "https://api.github.com/repos/PRISSET/mods/contents";

pub struct MinecraftInstaller {
    client: Client,
    game_dir: PathBuf,
}

impl MinecraftInstaller {
    pub fn new(game_dir: PathBuf) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_else(|_| Client::new()),
            game_dir,
        }
    }

    pub async fn is_installed(&self) -> bool {
        let fabric_id = format!("fabric-loader-{}-{}", FABRIC_LOADER_VERSION, MINECRAFT_VERSION);
        let fabric_json = self.game_dir
            .join("versions")
            .join(&fabric_id)
            .join(format!("{}.json", fabric_id));
        
        let client_jar = self.game_dir
            .join("versions")
            .join(MINECRAFT_VERSION)
            .join(format!("{}.jar", MINECRAFT_VERSION));
        
        fabric_json.exists() && client_jar.exists()
    }

    pub async fn install_simple(&self) -> Result<()> {
        self.ensure_java().await?;
        let version_info = self.download_version_info().await?;
        self.download_client(&version_info).await?;
        self.download_libraries(&version_info).await?;
        self.download_assets(&version_info).await?;
        self.install_fabric().await?;
        self.download_mods().await?;
        Ok(())
    }

    pub async fn download_mods(&self) -> Result<()> {
        let mods_dir = self.game_dir.join("mods");
        fs::create_dir_all(&mods_dir)?;
        
        let mods_api_url = format!("{}/{}", MODS_REPO_BASE, MODS_FOLDER);
        
        let response = self.client
            .get(&mods_api_url)
            .header("User-Agent", "ByStep-Launcher")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Не удалось получить список модов: {}", response.status()));
        }
        
        let files: Vec<GitHubFile> = response.json().await?;
        let mod_files: Vec<&GitHubFile> = files.iter()
            .filter(|f| f.file_type == "file" && (f.name.ends_with(".jar") || f.name.ends_with(".zip")))
            .collect();
        
        let mod_names: Vec<String> = mod_files.iter().map(|f| f.name.clone()).collect();
        
        if let Ok(entries) = fs::read_dir(&mods_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if (file_name.ends_with(".jar") || file_name.ends_with(".zip")) && !mod_names.contains(&file_name) {
                    for _ in 0..3 {
                        if fs::remove_file(entry.path()).is_ok() {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        }
        
        for file in mod_files {
            let mod_path = mods_dir.join(&file.name);
            
            if mod_path.exists() {
                continue;
            }
            
            if let Some(download_url) = &file.download_url {
                let _ = self.download_file(download_url, &mod_path).await;
            }
        }
        
        Ok(())
    }
    
    pub async fn download_shaderpacks(&self) -> Result<()> {
        let shaderpacks_dir = self.game_dir.join("shaderpacks");
        fs::create_dir_all(&shaderpacks_dir)?;
        
        let api_url = format!("{}/{}/shaderpacks", MODS_REPO_BASE, MODS_FOLDER);
        
        let response = self.client
            .get(&api_url)
            .header("User-Agent", "ByStep-Launcher")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Ok(());
        }
        
        let files: Vec<GitHubFile> = response.json().await?;
        
        for file in files.iter().filter(|f| f.file_type == "file") {
            let shaderpack_path = shaderpacks_dir.join(&file.name);
            
            if shaderpack_path.exists() {
                continue;
            }
            
            if let Some(download_url) = &file.download_url {
                let _ = self.download_file(download_url, &shaderpack_path).await;
            }
        }
        
        Ok(())
    }
    
    pub async fn download_resourcepacks(&self) -> Result<()> {
        let resourcepacks_dir = self.game_dir.join("resourcepacks");
        fs::create_dir_all(&resourcepacks_dir)?;
        
        let api_url = format!("{}/{}/resourcepacks", MODS_REPO_BASE, MODS_FOLDER);
        
        let response = self.client
            .get(&api_url)
            .header("User-Agent", "ByStep-Launcher")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Ok(());
        }
        
        let files: Vec<GitHubFile> = response.json().await?;
        
        for file in files.iter().filter(|f| f.file_type == "file") {
            let pack_path = resourcepacks_dir.join(&file.name);
            
            if pack_path.exists() {
                continue;
            }
            
            if let Some(download_url) = &file.download_url {
                let _ = self.download_file(download_url, &pack_path).await;
            }
        }
        
        Ok(())
    }

    async fn ensure_java(&self) -> Result<()> {
        let java_dir = self.game_dir.join("runtime").join("java-21");
        let java_exe = java_dir.join("bin").join("java.exe");
        
        if java_exe.exists() {
            return Ok(());
        }
        
        let runtime_dir = self.game_dir.join("runtime");
        fs::create_dir_all(&runtime_dir)?;
        
        let zip_path = runtime_dir.join("java21.zip");
        self.download_file(JAVA21_URL, &zip_path).await?;
        self.extract_zip(&zip_path, &runtime_dir)?;
        let _ = fs::remove_file(&zip_path);
        
        if let Ok(entries) = fs::read_dir(&runtime_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.starts_with("jdk-21") || name.starts_with("openjdk") {
                    let extracted = entry.path();
                    if extracted != java_dir && extracted.is_dir() {
                        let _ = fs::rename(&extracted, &java_dir);
                    }
                }
            }
        }
        
        if !java_exe.exists() {
            return Err(anyhow!("Failed to install Java 21"));
        }
        
        Ok(())
    }
    
    fn extract_zip(&self, zip_path: &Path, dest: &Path) -> Result<()> {
        let file = fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => dest.join(path),
                None => continue,
            };
            
            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        
        Ok(())
    }

    async fn download_version_info(&self) -> Result<VersionInfo> {
        let manifest: VersionManifest = self.client
            .get(VERSION_MANIFEST_URL)
            .send()
            .await?
            .json()
            .await?;

        let version = manifest.versions
            .iter()
            .find(|v| v.id == MINECRAFT_VERSION)
            .ok_or_else(|| anyhow!("Версия {} не найдена", MINECRAFT_VERSION))?;

        let version_info: VersionInfo = self.client
            .get(&version.url)
            .send()
            .await?
            .json()
            .await?;

        let versions_dir = self.game_dir.join("versions").join(MINECRAFT_VERSION);
        fs::create_dir_all(&versions_dir)?;
        
        let json_path = versions_dir.join(format!("{}.json", MINECRAFT_VERSION));
        let json_content = serde_json::to_string_pretty(&version_info)?;
        fs::write(&json_path, json_content)?;

        Ok(version_info)
    }

    async fn download_client(&self, version_info: &VersionInfo) -> Result<()> {
        let versions_dir = self.game_dir.join("versions").join(MINECRAFT_VERSION);
        fs::create_dir_all(&versions_dir)?;
        
        let jar_path = versions_dir.join(format!("{}.jar", MINECRAFT_VERSION));
        
        if jar_path.exists() {
            return Ok(());
        }

        self.download_file(&version_info.downloads.client.url, &jar_path).await?;
        Ok(())
    }

    async fn download_libraries(&self, version_info: &VersionInfo) -> Result<()> {
        let libraries_dir = self.game_dir.join("libraries");
        
        for library in &version_info.libraries {
            if !self.should_use_library(library) {
                continue;
            }

            if let Some(downloads) = &library.downloads {
                if let Some(artifact) = &downloads.artifact {
                    let lib_path = libraries_dir.join(&artifact.path);
                    
                    if lib_path.exists() {
                        continue;
                    }

                    if let Some(parent) = lib_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    let _ = self.download_file(&artifact.url, &lib_path).await;
                }
            }
        }

        Ok(())
    }

    fn should_use_library(&self, library: &Library) -> bool {
        if let Some(rules) = &library.rules {
            for rule in rules {
                if let Some(os) = &rule.os {
                    let is_windows = os.name == "windows";
                    if rule.action == "allow" && !is_windows {
                        return false;
                    }
                    if rule.action == "disallow" && is_windows {
                        return false;
                    }
                }
            }
        }
        true
    }

    async fn download_assets(&self, version_info: &VersionInfo) -> Result<()> {
        let indexes_dir = self.game_dir.join("assets").join("indexes");
        let objects_dir = self.game_dir.join("assets").join("objects");
        fs::create_dir_all(&indexes_dir)?;
        fs::create_dir_all(&objects_dir)?;

        let index_path = indexes_dir.join(format!("{}.json", version_info.asset_index.id));
        
        if !index_path.exists() {
            self.download_file(&version_info.asset_index.url, &index_path).await?;
        }

        let index_content = fs::read_to_string(&index_path)?;
        let asset_index: AssetIndex = serde_json::from_str(&index_content)?;

        for (_name, object) in &asset_index.objects {
            let hash_prefix = &object.hash[..2];
            let object_dir = objects_dir.join(hash_prefix);
            fs::create_dir_all(&object_dir)?;
            
            let object_path = object_dir.join(&object.hash);
            
            if object_path.exists() {
                continue;
            }

            let url = format!(
                "https://resources.download.minecraft.net/{}/{}",
                hash_prefix, object.hash
            );

            let _ = self.download_file(&url, &object_path).await;
        }

        Ok(())
    }

    async fn install_fabric(&self) -> Result<()> {
        let fabric_profile_url = format!(
            "{}/v2/versions/loader/{}/{}/profile/json",
            FABRIC_META_URL, MINECRAFT_VERSION, FABRIC_LOADER_VERSION
        );

        let fabric_profile: serde_json::Value = self.client
            .get(&fabric_profile_url)
            .send()
            .await?
            .json()
            .await?;

        let fabric_version_id = format!("fabric-loader-{}-{}", FABRIC_LOADER_VERSION, MINECRAFT_VERSION);
        let fabric_dir = self.game_dir.join("versions").join(&fabric_version_id);
        fs::create_dir_all(&fabric_dir)?;

        let json_path = fabric_dir.join(format!("{}.json", fabric_version_id));
        fs::write(&json_path, serde_json::to_string_pretty(&fabric_profile)?)?;

        if let Some(libraries) = fabric_profile.get("libraries").and_then(|l| l.as_array()) {
            for lib in libraries {
                if let (Some(name), Some(url)) = (
                    lib.get("name").and_then(|n| n.as_str()),
                    lib.get("url").and_then(|u| u.as_str()),
                ) {
                    let path = self.maven_name_to_path(name);
                    let lib_path = self.game_dir.join("libraries").join(&path);
                    
                    if lib_path.exists() {
                        continue;
                    }

                    if let Some(parent) = lib_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    let full_url = format!("{}{}", url, path);
                    let _ = self.download_file(&full_url, &lib_path).await;
                }
            }
        }

        self.create_default_options()?;

        Ok(())
    }

    fn create_default_options(&self) -> Result<()> {
        let options_path = self.game_dir.join("options.txt");
        
        if options_path.exists() {
            let content = fs::read_to_string(&options_path).unwrap_or_default();
            if !content.contains("lang:") {
                let new_content = format!("lang:ru_ru\n{}", content);
                fs::write(&options_path, new_content)?;
            }
            return Ok(());
        }
        
        let options_content = r#"lang:ru_ru
soundCategory_master:1.0
soundCategory_music:1.0
soundCategory_record:1.0
soundCategory_weather:1.0
soundCategory_block:1.0
soundCategory_hostile:1.0
soundCategory_neutral:1.0
soundCategory_player:1.0
soundCategory_ambient:1.0
soundCategory_voice:1.0
modelPart_cape:true
modelPart_jacket:true
modelPart_left_sleeve:true
modelPart_right_sleeve:true
modelPart_left_pants_leg:true
modelPart_right_pants_leg:true
modelPart_hat:true
mainHand:"right"
resourcePacks:["vanilla","file/Actually-3D-Stuff-1.21.zip"]
"#;
        
        fs::write(&options_path, options_content)?;
        
        Ok(())
    }
    
    fn maven_name_to_path(&self, name: &str) -> String {
        let parts: Vec<&str> = name.split(':').collect();
        if parts.len() >= 3 {
            let group = parts[0].replace('.', "/");
            let artifact = parts[1];
            let version = parts[2];
            format!("{}/{}/{}/{}-{}.jar", group, artifact, version, artifact, version)
        } else {
            name.to_string()
        }
    }

    async fn download_file(&self, url: &str, path: &Path) -> Result<()> {
        let response = self.client
            .get(url)
            .header("User-Agent", "ByStep-Launcher")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to download: {}", url));
        }
        
        let mut file = fs::File::create(path)?;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk)?;
        }

        Ok(())
    }
    
    pub fn find_java(&self) -> Result<PathBuf> {
        let java_dir = self.game_dir.join("runtime").join("java-21");
        let java_exe = java_dir.join("bin").join("java.exe");
        
        if java_exe.exists() {
            return Ok(java_exe);
        }
        
        Err(anyhow!("Java 21 not found"))
    }
}
