mod state;
mod styles;
mod utils;
mod update;
mod subscription;
mod view;
mod views;

pub use state::*;
pub use utils::{load_gif_frames, load_avatar_frames, load_icon, check_for_updates, fetch_server_status};

use iced::Task;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::path::PathBuf;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

impl MinecraftLauncher {
    pub fn new() -> (Self, Task<Message>) {
        let settings = Self::load_settings().unwrap_or_default();
        let play_stats = Self::load_play_stats().unwrap_or_default();
        let gif_frames = load_gif_frames();
        let avatar_frames = load_avatar_frames();
        
        let discord_client = Self::init_discord();
        
        (
            Self {
                nickname: settings.nickname,
                ram_gb: settings.ram_gb,
                selected_version: settings.selected_version,
                shader_quality: settings.shader_quality,
                launch_state: LaunchState::CheckingUpdate,
                active_tab: Tab::Dashboard,
                game_running: Arc::new(AtomicBool::new(false)),
                gif_frames,
                avatar_frames,
                current_frame: 0,
                update_checked: false,
                play_stats,
                current_session_seconds: 0,
                discord_client,
                game_start_time: None,
                server_status: ServerStatus::default(),
                crash_count: 0,
                show_crash_dialog: false,
                show_changelog: false,
                crash_log: None,
            },
            Task::batch([
                Task::perform(check_for_updates(), Message::UpdateStatus),
                Task::perform(fetch_server_status(), Message::ServerStatusUpdate),
            ]),
        )
    }
    
    fn init_discord() -> Arc<Mutex<Option<DiscordIpcClient>>> {
        let client = DiscordIpcClient::new(DISCORD_CLIENT_ID)
            .ok()
            .and_then(|mut c| {
                c.connect().ok()?;
                Some(c)
            });
        Arc::new(Mutex::new(client))
    }

    pub fn save_settings(&self) {
        if let Some(config_dir) = Self::get_config_dir() {
            let settings = LauncherSettings { 
                nickname: self.nickname.clone(), 
                ram_gb: self.ram_gb,
                selected_version: self.selected_version,
                shader_quality: self.shader_quality,
            };
            if let Ok(json) = serde_json::to_string_pretty(&settings) {
                let _ = std::fs::write(config_dir.join("settings.json"), json);
            }
        }
    }

    pub fn load_settings() -> Option<LauncherSettings> {
        let config_dir = Self::get_config_dir()?;
        let content = std::fs::read_to_string(config_dir.join("settings.json")).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn get_config_dir() -> Option<PathBuf> {
        let config_dir = directories::ProjectDirs::from("com", "bystep", "launcher")?.config_dir().to_path_buf();
        std::fs::create_dir_all(&config_dir).ok()?;
        Some(config_dir)
    }

    pub fn get_game_data_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "bystep", "minecraft")
            .map(|dirs| dirs.data_dir().to_path_buf())
    }

    pub fn save_play_stats(&self) {
        if let Some(config_dir) = Self::get_config_dir() {
            if let Ok(json) = serde_json::to_string_pretty(&self.play_stats) {
                let _ = std::fs::write(config_dir.join("playtime.json"), json);
            }
        }
    }

    pub fn load_play_stats() -> Option<PlayTimeStats> {
        let config_dir = Self::get_config_dir()?;
        let content = std::fs::read_to_string(config_dir.join("playtime.json")).ok()?;
        serde_json::from_str(&content).ok()
    }
}
