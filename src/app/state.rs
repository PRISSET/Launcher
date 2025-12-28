use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use discord_rich_presence::DiscordIpcClient;
use iced::widget::image;

pub const SERVER_ADDRESS: &str = "144.31.169.7:25565";
pub const CURRENT_VERSION: &str = "1.1.0";
pub const GITHUB_RELEASES_API: &str = "https://api.github.com/repos/PRISSET/Launcher/releases/latest";
pub const INSTALLER_NAME: &str = "ByStep-Launcher-Setup.exe";
pub const DISCORD_CLIENT_ID: &str = "1454405559120822426";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherSettings {
    pub nickname: String,
    pub ram_gb: u32,
    #[serde(default)]
    pub shaders_enabled: bool,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            ram_gb: 4,
            shaders_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlayTimeStats {
    pub daily: HashMap<String, u64>,
    pub total_seconds: u64,
}

#[derive(Debug, Clone)]
pub enum LaunchState {
    CheckingUpdate,
    UpdateAvailable { version: String, download_url: String },
    Updating { progress: String },
    Idle,
    Installing { step: String, progress: f32 },
    Launching,
    Playing,
    Error(String),
}

impl PartialEq for LaunchState {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ServerStatus {
    pub online: bool,
    pub players_online: u32,
    pub players_max: u32,
    pub player_names: Vec<String>,
}


#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Dashboard,
    Statistics,
    Settings,
}

#[derive(Debug, Clone)]
pub enum Message {
    NicknameChanged(String),
    RamChanged(u32),
    ShadersToggled(bool),
    LaunchGame,
    SwitchTab(Tab),
    InstallProgress(String, f32),
    LaunchComplete(Result<(), String>),
    GameExited,
    GameCrashed,
    NextFrame,
    CheckUpdate,
    UpdateStatus(UpdateResult),
    PlayTimeTick,
    ServerStatusUpdate(ServerStatus),
    AcceptUpdate,
    DeclineUpdate,
    ReinstallGame,
    DismissCrashDialog,
}

#[derive(Debug, Clone)]
pub enum UpdateResult {
    NoUpdate,
    UpdateAvailable(String, String),
    Downloading(String),
    Downloaded(PathBuf),
    Error(String),
}

pub struct MinecraftLauncher {
    pub nickname: String,
    pub ram_gb: u32,
    pub shaders_enabled: bool,
    pub launch_state: LaunchState,
    pub active_tab: Tab,
    pub game_running: Arc<AtomicBool>,
    pub gif_frames: Vec<image::Handle>,
    pub avatar_frames: Vec<image::Handle>,
    pub current_frame: usize,
    pub update_checked: bool,
    pub play_stats: PlayTimeStats,
    pub current_session_seconds: u64,
    pub discord_client: Arc<Mutex<Option<DiscordIpcClient>>>,
    pub game_start_time: Option<i64>,
    pub server_status: ServerStatus,
    pub crash_count: u32,
    pub show_crash_dialog: bool,
}
