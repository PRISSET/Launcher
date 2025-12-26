use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use iced::futures::channel::mpsc;
use iced::futures::SinkExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Stdio;

const MINECRAFT_VERSION: &str = "1.21.1";
const FABRIC_LOADER_VERSION: &str = "0.17.2";
const VERSION_MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META_URL: &str = "https://meta.fabricmc.net";

pub const MODS_SERVER_URL: &str = "https://your-server.com/mods";
const JAVA21_URL: &str = "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.5%2B11/OpenJDK21U-jre_x64_windows_hotspot_21.0.5_11.zip";

// GitHub API для получения списка модов
const MODS_REPO_API: &str = "https://api.github.com/repos/PRISSET/mods/contents";
const MODS_RAW_URL: &str = "https://raw.githubusercontent.com/PRISSET/mods/main";
const SHADERPACKS_REPO_API: &str = "https://api.github.com/repos/PRISSET/mods/contents/shaderpacks";
const RESOURCEPACKS_REPO_API: &str = "https://api.github.com/repos/PRISSET/mods/contents/resourcepacks";

#[derive(Debug, Clone)]
pub struct InstallProgress {
    pub step: String,
    pub progress: f32,
}

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}

#[derive(Debug, Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionInfo {
    id: String,
    #[serde(rename = "assetIndex")]
    asset_index: AssetIndexInfo,
    downloads: Downloads,
    libraries: Vec<Library>,
    #[serde(rename = "mainClass")]
    main_class: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct AssetIndexInfo {
    id: String,
    url: String,
    sha1: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Downloads {
    client: DownloadInfo,
}

#[derive(Debug, Deserialize, Serialize)]
struct DownloadInfo {
    url: String,
    sha1: String,
    size: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Library {
    downloads: Option<LibraryDownloads>,
    name: String,
    rules: Option<Vec<Rule>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LibraryDownloads {
    artifact: Option<Artifact>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Artifact {
    path: String,
    url: String,
    sha1: String,
    size: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Rule {
    action: String,
    os: Option<OsRule>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OsRule {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AssetIndex {
    objects: std::collections::HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

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
        self.ensure_java21_simple().await?;
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
        
        // Получаем список файлов из GitHub API
        let response = self.client
            .get(MODS_REPO_API)
            .header("User-Agent", "ByStep-Launcher")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Не удалось получить список модов: {}", response.status()));
        }
        
        let files: Vec<serde_json::Value> = response.json().await?;
        
        for file in files {
            let name = file.get("name").and_then(|n| n.as_str()).unwrap_or("");
            
            // Скачиваем только .jar файлы
            if !name.ends_with(".jar") {
                continue;
            }
            
            let mod_path = mods_dir.join(name);
            
            // Пропускаем если мод уже скачан
            if mod_path.exists() {
                continue;
            }
            
            // URL-encode имя файла для скачивания
            let encoded_name = name.replace(" ", "%20");
            let download_url = format!("{}/{}", MODS_RAW_URL, encoded_name);
            let _ = self.download_file(&download_url, &mod_path).await;
        }
        
        Ok(())
    }
    
    pub async fn download_shaderpacks(&self) -> Result<()> {
        let shaderpacks_dir = self.game_dir.join("shaderpacks");
        fs::create_dir_all(&shaderpacks_dir)?;
        
        let response = self.client
            .get(SHADERPACKS_REPO_API)
            .header("User-Agent", "ByStep-Launcher")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Не удалось получить список шейдерпаков"));
        }
        
        let files: Vec<serde_json::Value> = response.json().await?;
        
        for file in files {
            let name = file.get("name").and_then(|n| n.as_str()).unwrap_or("");
            
            if !name.ends_with(".zip") {
                continue;
            }
            
            let shaderpack_path = shaderpacks_dir.join(name);
            
            if shaderpack_path.exists() {
                continue;
            }
            
            let encoded_name = name.replace(" ", "%20");
            let download_url = format!("{}/shaderpacks/{}", MODS_RAW_URL, encoded_name);
            let _ = self.download_file(&download_url, &shaderpack_path).await;
        }
        
        Ok(())
    }
    
    pub async fn download_resourcepacks(&self) -> Result<()> {
        let resourcepacks_dir = self.game_dir.join("resourcepacks");
        fs::create_dir_all(&resourcepacks_dir)?;
        
        let response = self.client
            .get(RESOURCEPACKS_REPO_API)
            .header("User-Agent", "ByStep-Launcher")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Не удалось получить список текстурпаков"));
        }
        
        let files: Vec<serde_json::Value> = response.json().await?;
        
        for file in files {
            let name = file.get("name").and_then(|n| n.as_str()).unwrap_or("");
            
            if !name.ends_with(".zip") {
                continue;
            }
            
            let pack_path = resourcepacks_dir.join(name);
            
            if pack_path.exists() {
                continue;
            }
            
            let encoded_name = name.replace(" ", "%20");
            let download_url = format!("{}/resourcepacks/{}", MODS_RAW_URL, encoded_name);
            let _ = self.download_file(&download_url, &pack_path).await;
        }
        
        Ok(())
    }
    
    async fn ensure_java21_simple(&self) -> Result<()> {
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

    pub async fn install_with_channel(&self, mut tx: mpsc::Sender<InstallProgress>) -> Result<()> {
        let _ = tx.send(InstallProgress { 
            step: "Проверка Java 21...".into(), 
            progress: 0.0 
        }).await;
        
        self.ensure_java21_channel(&mut tx).await?;
        
        let _ = tx.send(InstallProgress { 
            step: "Получение версии Minecraft...".into(), 
            progress: 0.1 
        }).await;
        
        let version_info = self.download_version_info().await?;
        
        let _ = tx.send(InstallProgress { 
            step: "Скачивание клиента Minecraft...".into(), 
            progress: 0.15 
        }).await;
        self.download_client(&version_info).await?;

        let _ = tx.send(InstallProgress { 
            step: "Скачивание библиотек...".into(), 
            progress: 0.25 
        }).await;
        self.download_libraries(&version_info).await?;

        let _ = tx.send(InstallProgress { 
            step: "Скачивание ресурсов...".into(), 
            progress: 0.5 
        }).await;
        self.download_assets(&version_info).await?;

        let _ = tx.send(InstallProgress { 
            step: "Установка Fabric...".into(), 
            progress: 0.8 
        }).await;
        self.install_fabric().await?;

        let _ = tx.send(InstallProgress { 
            step: "Готово!".into(), 
            progress: 1.0 
        }).await;
        
        Ok(())
    }
    
    async fn ensure_java21_channel(&self, tx: &mut mpsc::Sender<InstallProgress>) -> Result<()> {
        let java_dir = self.game_dir.join("runtime").join("java-21");
        let java_exe = java_dir.join("bin").join("java.exe");
        
        if java_exe.exists() {
            return Ok(());
        }
        
        let _ = tx.send(InstallProgress { 
            step: "Скачивание Java 21...".into(), 
            progress: 0.02 
        }).await;
        
        let runtime_dir = self.game_dir.join("runtime");
        fs::create_dir_all(&runtime_dir)?;
        
        let zip_path = runtime_dir.join("java21.zip");
        
        self.download_file(JAVA21_URL, &zip_path).await?;
        
        let _ = tx.send(InstallProgress { 
            step: "Распаковка Java 21...".into(), 
            progress: 0.05 
        }).await;
        
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

    pub async fn install(&self, progress_callback: impl Fn(InstallProgress)) -> Result<()> {
        progress_callback(InstallProgress { 
            step: "Проверка Java 21...".into(), 
            progress: 0.0 
        });
        
        self.ensure_java21(&progress_callback).await?;
        
        progress_callback(InstallProgress { 
            step: "Получение версии Minecraft...".into(), 
            progress: 0.1 
        });
        
        let version_info = self.download_version_info().await?;
        
        progress_callback(InstallProgress { 
            step: "Скачивание клиента Minecraft...".into(), 
            progress: 0.15 
        });
        self.download_client(&version_info).await?;

        progress_callback(InstallProgress { 
            step: "Скачивание библиотек...".into(), 
            progress: 0.25 
        });
        self.download_libraries(&version_info).await?;

        progress_callback(InstallProgress { 
            step: "Скачивание ресурсов...".into(), 
            progress: 0.5 
        });
        self.download_assets(&version_info).await?;

        progress_callback(InstallProgress { 
            step: "Установка Fabric...".into(), 
            progress: 0.8 
        });
        self.install_fabric().await?;

        progress_callback(InstallProgress { 
            step: "Готово!".into(), 
            progress: 1.0 
        });
        
        Ok(())
    }
    
    async fn ensure_java21(&self, progress_callback: &impl Fn(InstallProgress)) -> Result<()> {
        let java_dir = self.game_dir.join("runtime").join("java-21");
        let java_exe = java_dir.join("bin").join("java.exe");
        
        if java_exe.exists() {
            println!("Java 21 already installed at {:?}", java_exe);
            return Ok(());
        }
        
        println!("Java 21 not found, downloading...");
        progress_callback(InstallProgress { 
            step: "Скачивание Java 21...".into(), 
            progress: 0.02 
        });
        
        let runtime_dir = self.game_dir.join("runtime");
        fs::create_dir_all(&runtime_dir)?;
        
        let zip_path = runtime_dir.join("java21.zip");
        
        self.download_file(JAVA21_URL, &zip_path).await?;
        
        progress_callback(InstallProgress { 
            step: "Распаковка Java 21...".into(), 
            progress: 0.05 
        });
        
        println!("Extracting Java 21...");
        self.extract_zip(&zip_path, &runtime_dir)?;
        
        let _ = fs::remove_file(&zip_path);
        
        if let Ok(entries) = fs::read_dir(&runtime_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.starts_with("jdk-21") || name.starts_with("openjdk") {
                    let extracted = entry.path();
                    if extracted != java_dir && extracted.is_dir() {
                        println!("Renaming {:?} to {:?}", extracted, java_dir);
                        let _ = fs::rename(&extracted, &java_dir);
                    }
                }
            }
        }
        
        if java_exe.exists() {
            println!("Java 21 installed successfully!");
        } else {
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

        // Создаём options.txt с русским языком
        self.create_default_options()?;

        Ok(())
    }

    fn create_default_options(&self) -> Result<()> {
        let options_path = self.game_dir.join("options.txt");
        
        // Если файл уже существует, не перезаписываем (пользователь мог изменить настройки)
        if options_path.exists() {
            // Проверяем, есть ли уже настройка языка
            let content = fs::read_to_string(&options_path).unwrap_or_default();
            if !content.contains("lang:") {
                // Добавляем язык в существующий файл
                let new_content = format!("lang:ru_ru\n{}", content);
                fs::write(&options_path, new_content)?;
            }
            return Ok(());
        }
        
        // Создаём новый options.txt с русским языком и базовыми настройками
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
        println!("Created options.txt with Russian language");
        
        Ok(())
    }
}

/// Настраивает options.txt для включения/выключения шейдеров
pub fn configure_shaders(game_dir: &Path, enable_shaders: bool) -> Result<()> {
    let iris_config_dir = game_dir.join("config").join("iris.properties");
    
    // Создаём папку config если нет
    if let Some(parent) = iris_config_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let shaderpack = if enable_shaders {
        "ComplementaryUnbound_r5.6.1.zip"
    } else {
        ""
    };
    
    let iris_config = format!(
        "shaderPack={}\nenableShaders={}\n",
        shaderpack,
        enable_shaders
    );
    
    fs::write(&iris_config_dir, iris_config)?;
    
    Ok(())
}

impl MinecraftInstaller {
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
        let response = self.client.get(url).send().await?;
        
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
}

pub fn generate_offline_uuid(nickname: &str) -> String {
    use sha2::{Sha256, Digest};
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

pub fn get_game_directory() -> PathBuf {
    directories::ProjectDirs::from("com", "bystep", "minecraft")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".bystep-minecraft")
        })
}

pub fn build_launch_command(
    game_dir: &Path,
    nickname: &str,
    ram_gb: u32,
    server_address: Option<&str>,
) -> Result<std::process::Command> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let fabric_version_id = format!("fabric-loader-{}-{}", FABRIC_LOADER_VERSION, MINECRAFT_VERSION);
    
    // Читаем version info чтобы получить правильный asset index id
    let version_json_path = game_dir
        .join("versions")
        .join(MINECRAFT_VERSION)
        .join(format!("{}.json", MINECRAFT_VERSION));
    
    let asset_index_id = if version_json_path.exists() {
        let content = fs::read_to_string(&version_json_path).unwrap_or_default();
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
            info.get("assetIndex")
                .and_then(|ai| ai.get("id"))
                .and_then(|id| id.as_str())
                .unwrap_or(MINECRAFT_VERSION)
                .to_string()
        } else {
            MINECRAFT_VERSION.to_string()
        }
    } else {
        MINECRAFT_VERSION.to_string()
    };
    
    let java_path = find_java()?;
    let mut cmd = std::process::Command::new(java_path);
    
    // Скрываем консольное окно
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    
    cmd.arg(format!("-Xmx{}G", ram_gb));
    cmd.arg(format!("-Xms{}G", ram_gb.min(2)));
    cmd.arg("-XX:+UseG1GC");
    cmd.arg("-XX:+ParallelRefProcEnabled");
    cmd.arg("-XX:MaxGCPauseMillis=200");
    
    let natives_dir = game_dir.join("natives");
    fs::create_dir_all(&natives_dir)?;
    cmd.arg(format!("-Djava.library.path={}", natives_dir.display()));
    cmd.arg("-Dminecraft.launcher.brand=ByStep");
    cmd.arg("-Dminecraft.launcher.version=1.0.0");
    
    let mut classpath = Vec::new();
    
    let libraries_dir = game_dir.join("libraries");
    if libraries_dir.exists() {
        collect_jars(&libraries_dir, &mut classpath)?;
    }
    
    let client_jar = game_dir
        .join("versions")
        .join(MINECRAFT_VERSION)
        .join(format!("{}.jar", MINECRAFT_VERSION));
    classpath.push(client_jar.display().to_string());
    
    let separator = ";";
    
    cmd.arg("-cp");
    cmd.arg(classpath.join(separator));
    
    cmd.arg("net.fabricmc.loader.impl.launch.knot.KnotClient");
    
    cmd.arg("--username");
    cmd.arg(nickname);
    
    cmd.arg("--version");
    cmd.arg(&fabric_version_id);
    
    cmd.arg("--gameDir");
    cmd.arg(game_dir);
    
    cmd.arg("--assetsDir");
    cmd.arg(game_dir.join("assets"));
    
    cmd.arg("--assetIndex");
    cmd.arg(&asset_index_id);
    
    cmd.arg("--uuid");
    cmd.arg(generate_offline_uuid(nickname));
    
    cmd.arg("--accessToken");
    cmd.arg("0");
    
    cmd.arg("--userType");
    cmd.arg("legacy");
    
    // Создаём servers.dat с нашим сервером
    if let Some(server) = server_address {
        if !server.is_empty() {
            let _ = create_servers_dat(game_dir, server);
            
            // Также пробуем через аргументы (для совместимости)
            let parts: Vec<&str> = server.split(':').collect();
            cmd.arg("--server");
            cmd.arg(parts[0]);
            if parts.len() > 1 {
                cmd.arg("--port");
                cmd.arg(parts[1]);
            }
        }
    }
    
    cmd.current_dir(game_dir);
    
    Ok(cmd)
}

/// Создаёт servers.dat файл с сервером ByStep
fn create_servers_dat(game_dir: &Path, server_address: &str) -> Result<()> {
    use std::io::{Cursor, Write};
    
    let servers_path = game_dir.join("servers.dat");
    
    let parts: Vec<&str> = server_address.split(':').collect();
    let ip = parts[0];
    let port = parts.get(1).unwrap_or(&"25565");
    let full_address = format!("{}:{}", ip, port);
    
    // NBT формат для servers.dat
    // Compound "" {
    //   List "servers" [
    //     Compound {
    //       String "ip": "address:port"
    //       String "name": "ByStep Server"
    //       Byte "acceptTextures": 1
    //     }
    //   ]
    // }
    
    let mut data = Vec::new();
    
    // Root compound tag (type 10, empty name)
    data.push(10); // TAG_Compound
    data.extend_from_slice(&0u16.to_be_bytes()); // empty name length
    
    // "servers" list
    data.push(9); // TAG_List
    let servers_name = b"servers";
    data.extend_from_slice(&(servers_name.len() as u16).to_be_bytes());
    data.extend_from_slice(servers_name);
    data.push(10); // list contains TAG_Compound
    data.extend_from_slice(&1i32.to_be_bytes()); // 1 element in list
    
    // Server entry compound (no type/name for list elements)
    
    // "ip" string
    data.push(8); // TAG_String
    let ip_name = b"ip";
    data.extend_from_slice(&(ip_name.len() as u16).to_be_bytes());
    data.extend_from_slice(ip_name);
    let ip_bytes = full_address.as_bytes();
    data.extend_from_slice(&(ip_bytes.len() as u16).to_be_bytes());
    data.extend_from_slice(ip_bytes);
    
    // "name" string
    data.push(8); // TAG_String
    let name_key = b"name";
    data.extend_from_slice(&(name_key.len() as u16).to_be_bytes());
    data.extend_from_slice(name_key);
    let name_val = b"ByStep Server";
    data.extend_from_slice(&(name_val.len() as u16).to_be_bytes());
    data.extend_from_slice(name_val);
    
    // "acceptTextures" byte (optional, for resource packs)
    data.push(1); // TAG_Byte
    let accept_name = b"acceptTextures";
    data.extend_from_slice(&(accept_name.len() as u16).to_be_bytes());
    data.extend_from_slice(accept_name);
    data.push(1); // true
    
    // End of server compound
    data.push(0); // TAG_End
    
    // End of root compound
    data.push(0); // TAG_End
    
    fs::write(&servers_path, &data)?;
    
    Ok(())
}

fn find_java() -> Result<PathBuf> {
    let game_dir = get_game_directory();
    let local_java = game_dir.join("runtime").join("java-21").join("bin").join("java.exe");
    if local_java.exists() {
        println!("Using local Java 21: {:?}", local_java);
        return Ok(local_java);
    }
    
    let java21_paths = [
        r"C:\Program Files\Java\jdk-21\bin\java.exe",
        r"C:\Program Files\Eclipse Adoptium\jdk-21.0.5.11-hotspot\bin\java.exe",
        r"C:\Program Files\Eclipse Adoptium\jdk-21.0.4.7-hotspot\bin\java.exe",
        r"C:\Program Files\Eclipse Adoptium\jdk-21.0.3.9-hotspot\bin\java.exe",
        r"C:\Program Files\Eclipse Adoptium\jdk-21.0.2.13-hotspot\bin\java.exe",
        r"C:\Program Files\Eclipse Adoptium\jdk-21.0.1.12-hotspot\bin\java.exe",
        r"C:\Program Files\Microsoft\jdk-21.0.5.11-hotspot\bin\java.exe",
        r"C:\Program Files\Microsoft\jdk-21.0.4.7-hotspot\bin\java.exe",
        r"C:\Program Files\Microsoft\jdk-21\bin\java.exe",
        r"C:\Program Files\Amazon Corretto\jdk21.0.5_11\bin\java.exe",
        r"C:\Program Files\Zulu\zulu-21\bin\java.exe",
        r"C:\Program Files\BellSoft\LibericaJDK-21\bin\java.exe",
    ];
    
    for path in &java21_paths {
        let p = PathBuf::from(path);
        if p.exists() {
            println!("Found Java 21: {:?}", p);
            return Ok(p);
        }
    }
    
    let program_files = PathBuf::from(r"C:\Program Files");
    if program_files.exists() {
        for dir_name in ["Eclipse Adoptium", "Java", "Microsoft", "Amazon Corretto", "Zulu", "BellSoft"] {
            let dir = program_files.join(dir_name);
            if dir.exists() {
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_lowercase();
                        if name.contains("21") || name.contains("jdk-21") {
                            let java = entry.path().join("bin").join("java.exe");
                            if java.exists() {
                                println!("Found Java 21: {:?}", java);
                                return Ok(java);
                            }
                        }
                    }
                }
            }
        }
    }
    
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let java_exe = PathBuf::from(&java_home).join("bin").join("java.exe");
        if java_exe.exists() {
            println!("Using JAVA_HOME: {:?}", java_exe);
            return Ok(java_exe);
        }
    }
    
    Err(anyhow!("Java 21 не найдена! Установите Java 21 с https://adoptium.net/"))
}

fn collect_jars(dir: &Path, jars: &mut Vec<String>) -> Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_jars(&path, jars)?;
            } else if path.extension().map(|e| e == "jar").unwrap_or(false) {
                jars.push(path.display().to_string());
            }
        }
    }
    Ok(())
}
