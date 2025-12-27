#![windows_subsystem = "windows"]

mod minecraft;

use iced::{
    Alignment, Border, Color, Element, Length, Shadow, Subscription, Task, Theme, Vector,
    widget::{button, column, container, row, slider, text, text_input, image, stack, Space, toggler},
    window, time,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use minecraft::{MinecraftInstaller, get_game_directory, build_launch_command};
use chrono::{Local, Datelike, NaiveDate};
use std::collections::HashMap;
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::Mutex;

const DISCORD_CLIENT_ID: &str = "1454405559120822426";

const ACCENT: Color = Color { r: 0.85, g: 0.15, b: 0.15, a: 1.0 }; 
const BG_SIDEBAR: Color = Color { r: 0.05, g: 0.05, b: 0.07, a: 0.98 };
const BG_CARD: Color = Color { r: 0.08, g: 0.08, b: 0.1, a: 0.85 };
const TEXT_PRIMARY: Color = Color { r: 0.98, g: 0.98, b: 1.0, a: 1.0 };
const TEXT_SECONDARY: Color = Color { r: 0.7, g: 0.73, b: 0.78, a: 1.0 };
const SERVER_ADDRESS: &str = "144.31.169.7:25565";

const CURRENT_VERSION: &str = "1.0.7";
const GITHUB_RELEASES_API: &str = "https://api.github.com/repos/PRISSET/Launcher/releases/latest";
const INSTALLER_NAME: &str = "ByStep-Launcher-Setup.exe";

fn load_gif_frames() -> Vec<image::Handle> {
    use ::image::codecs::gif::GifDecoder;
    use ::image::AnimationDecoder;
    
    let gif_data = include_bytes!("giphy.gif");
    let cursor = std::io::Cursor::new(gif_data.as_slice());
    
    if let Ok(decoder) = GifDecoder::new(cursor) {
        decoder.into_frames()
            .filter_map(|f| f.ok())
            .map(|frame| {
                let rgba = frame.into_buffer();
                let (w, h) = rgba.dimensions();
                image::Handle::from_rgba(w, h, rgba.into_raw())
            })
            .collect()
    } else {
        vec![image::Handle::from_bytes(include_bytes!("../background.png").to_vec())]
    }
}

fn load_avatar_frames() -> Vec<image::Handle> {
    use ::image::codecs::gif::GifDecoder;
    use ::image::AnimationDecoder;
    
    let gif_data = include_bytes!("avatar.gif");
    let cursor = std::io::Cursor::new(gif_data.as_slice());
    
    if let Ok(decoder) = GifDecoder::new(cursor) {
        decoder.into_frames()
            .filter_map(|f| f.ok())
            .map(|frame| {
                let rgba = frame.into_buffer();
                let (w, h) = rgba.dimensions();
                image::Handle::from_rgba(w, h, rgba.into_raw())
            })
            .collect()
    } else {
        vec![image::Handle::from_bytes(include_bytes!("icon.png").to_vec())]
    }
}

pub fn main() -> iced::Result {
    let icon = load_icon();
    
    iced::application("ByStep Launcher", MinecraftLauncher::update, MinecraftLauncher::view)
        .subscription(MinecraftLauncher::subscription)
        .theme(MinecraftLauncher::theme)
        .window(window::Settings {
            icon: icon,
            ..Default::default()
        })
        .run_with(MinecraftLauncher::new)
}

fn load_icon() -> Option<window::Icon> {
    let icon_data = include_bytes!("icon.png");
    let img = ::image::load_from_memory(icon_data).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    window::icon::from_rgba(img.into_raw(), width, height).ok()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LauncherSettings {
    nickname: String,
    ram_gb: u32,
    #[serde(default)]
    shaders_enabled: bool,
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
struct PlayTimeStats {
    daily: HashMap<String, u64>,
    total_seconds: u64,
}

#[derive(Debug, Clone)]
enum LaunchState {
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

struct MinecraftLauncher {
    nickname: String,
    ram_gb: u32,
    shaders_enabled: bool,
    launch_state: LaunchState,
    active_tab: Tab,
    game_running: Arc<AtomicBool>,
    gif_frames: Vec<image::Handle>,
    avatar_frames: Vec<image::Handle>,
    current_frame: usize,
    update_checked: bool,
    play_stats: PlayTimeStats,
    current_session_seconds: u64,
    discord_client: Arc<Mutex<Option<DiscordIpcClient>>>,
    game_start_time: Option<i64>,
    server_status: ServerStatus,
    crash_count: u32,
    show_crash_dialog: bool,
}

#[derive(Debug, Clone, Default)]
struct ServerStatus {
    online: bool,
    players_online: u32,
    players_max: u32,
    player_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Tab {
    Dashboard,
    Statistics,
    Settings,
}

#[derive(Debug, Clone)]
enum Message {
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
enum UpdateResult {
    NoUpdate,
    UpdateAvailable(String, String),
    Downloading(String),
    Downloaded(PathBuf),
    Error(String),
}

impl MinecraftLauncher {
    fn new() -> (Self, Task<Message>) {
        let settings = Self::load_settings().unwrap_or_default();
        let play_stats = Self::load_play_stats().unwrap_or_default();
        let gif_frames = load_gif_frames();
        let avatar_frames = load_avatar_frames();
        
        let discord_client = Self::init_discord();
        
        (
            Self {
                nickname: settings.nickname,
                ram_gb: settings.ram_gb,
                shaders_enabled: settings.shaders_enabled,
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
    
    fn update_discord_presence(&self, state: &str, details: &str) {
        if let Ok(mut guard) = self.discord_client.lock() {
            if let Some(client) = guard.as_mut() {
                let mut act = activity::Activity::new()
                    .state(state)
                    .details(details)
                    .assets(
                        activity::Assets::new()
                            .large_image("icon")
                            .large_text("ByStep Launcher")
                    );
                
                if let Some(start) = self.game_start_time {
                    act = act.timestamps(activity::Timestamps::new().start(start));
                }
                
                let _ = client.set_activity(act);
            }
        }
    }
    
    fn clear_discord_presence(&self) {
        if let Ok(mut guard) = self.discord_client.lock() {
            if let Some(client) = guard.as_mut() {
                let _ = client.clear_activity();
            }
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NicknameChanged(nickname) => {
                self.nickname = nickname;
                self.save_settings();
            }
            Message::RamChanged(ram) => {
                self.ram_gb = ram;
                self.save_settings();
            }
            Message::ShadersToggled(enabled) => {
                self.shaders_enabled = enabled;
                self.save_settings();
            }
            Message::LaunchGame => {
                if !self.nickname.is_empty() && matches!(self.launch_state, LaunchState::Idle | LaunchState::Error(_)) {
                    self.launch_state = LaunchState::Installing { 
                        step: "Подготовка...".into(), 
                        progress: 0.0 
                    };
                    self.game_running.store(true, Ordering::SeqCst);
                }
            }
            Message::SwitchTab(tab) => {
                self.active_tab = tab;
            }
            Message::InstallProgress(step, progress) => {
                self.launch_state = LaunchState::Installing { step, progress };
            }
            Message::LaunchComplete(result) => {
                match result {
                    Ok(_) => {
                        self.launch_state = LaunchState::Playing;
                        self.game_start_time = Some(std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64);
                        self.update_discord_presence("Играет на сервере", &format!("Игрок: {}", self.nickname));
                    }
                    Err(e) => self.launch_state = LaunchState::Error(e),
                }
            }
            Message::GameExited => {
                self.launch_state = LaunchState::Idle;
                self.game_running.store(false, Ordering::SeqCst);
                self.save_play_stats();
                self.current_session_seconds = 0;
                self.game_start_time = None;
                self.crash_count = 0;
                self.update_discord_presence("В лаунчере", "Выбирает настройки");
            }
            Message::GameCrashed => {
                self.launch_state = LaunchState::Idle;
                self.game_running.store(false, Ordering::SeqCst);
                self.current_session_seconds = 0;
                self.game_start_time = None;
                self.crash_count += 1;
                if self.crash_count >= 2 {
                    self.show_crash_dialog = true;
                }
                self.update_discord_presence("В лаунчере", "Выбирает настройки");
            }
            Message::ReinstallGame => {
                self.show_crash_dialog = false;
                self.crash_count = 0;
                if let Some(game_dir) = Self::get_game_data_dir() {
                    let _ = std::fs::remove_dir_all(&game_dir);
                }
                self.launch_state = LaunchState::Idle;
            }
            Message::DismissCrashDialog => {
                self.show_crash_dialog = false;
            }
            Message::NextFrame => {
                if !self.gif_frames.is_empty() {
                    self.current_frame = (self.current_frame + 1) % self.gif_frames.len();
                }
            }
            Message::CheckUpdate => {
                self.launch_state = LaunchState::CheckingUpdate;
                return Task::perform(check_for_updates(), Message::UpdateStatus);
            }
            Message::UpdateStatus(result) => {
                self.update_checked = true;
                match result {
                    UpdateResult::NoUpdate => {
                        self.launch_state = LaunchState::Idle;
                        self.update_discord_presence("В лаунчере", "Выбирает настройки");
                    }
                    UpdateResult::UpdateAvailable(version, url) => {
                        self.launch_state = LaunchState::UpdateAvailable { 
                            version: version.clone(),
                            download_url: url,
                        };
                    }
                    UpdateResult::Downloading(msg) => {
                        self.launch_state = LaunchState::Updating { progress: msg };
                    }
                    UpdateResult::Downloaded(path) => {
                        let _ = std::process::Command::new(path).spawn();
                        std::process::exit(0);
                    }
                    UpdateResult::Error(e) => {
                        self.launch_state = LaunchState::Idle;
                        eprintln!("Update error: {}", e);
                    }
                }
            }
            Message::AcceptUpdate => {
                if let LaunchState::UpdateAvailable { version, download_url } = self.launch_state.clone() {
                    self.launch_state = LaunchState::Updating { 
                        progress: format!("Скачивание v{}...", version) 
                    };
                    return Task::perform(download_and_run_update(download_url), Message::UpdateStatus);
                }
            }
            Message::DeclineUpdate => {
                self.launch_state = LaunchState::Idle;
                self.update_discord_presence("В лаунчере", "Выбирает настройки");
            }
            Message::PlayTimeTick => {
                if matches!(self.launch_state, LaunchState::Playing) {
                    self.current_session_seconds += 1;
                    let today = Local::now().format("%Y-%m-%d").to_string();
                    *self.play_stats.daily.entry(today).or_insert(0) += 1;
                    self.play_stats.total_seconds += 1;
                    if self.current_session_seconds % 60 == 0 {
                        self.save_play_stats();
                    }
                }
            }
            Message::ServerStatusUpdate(status) => {
                self.server_status = status;
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let gif_timer = time::every(Duration::from_millis(50)).map(|_| Message::NextFrame);
        let play_timer = time::every(Duration::from_secs(1)).map(|_| Message::PlayTimeTick);
        let server_status_timer = Subscription::run_with_id(
            "server-status",
            iced::stream::channel(10, |mut output| async move {
                use iced::futures::SinkExt;
                loop {
                    let status = fetch_server_status().await;
                    let _ = output.send(Message::ServerStatusUpdate(status)).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                }
            })
        );
        
        if self.game_running.load(Ordering::SeqCst) {
            let nickname = self.nickname.clone();
            let ram_gb = self.ram_gb;
            let shaders_enabled = self.shaders_enabled;
            
            let game_sub = Subscription::run_with_id(
                "game-launcher",
                iced::stream::channel(100, move |mut output| async move {
                    use iced::futures::SinkExt;
                    
                    let _ = output.send(Message::InstallProgress("Подготовка...".into(), 0.05)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    let game_dir = get_game_directory();
                    if let Err(e) = std::fs::create_dir_all(&game_dir) {
                        let _ = output.send(Message::LaunchComplete(Err(e.to_string()))).await;
                        return;
                    }
                    
                    let installer = MinecraftInstaller::new(game_dir.clone());
                    
                    let _ = output.send(Message::InstallProgress("Проверка установки...".into(), 0.1)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    let is_installed = installer.is_installed().await;
                    
                    if !is_installed {
                        let _ = output.send(Message::InstallProgress("Проверка Java 21...".into(), 0.15)).await;
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        
                        match installer.install_simple().await {
                            Ok(()) => {
                                let _ = output.send(Message::InstallProgress("Установка завершена!".into(), 0.85)).await;
                            }
                            Err(e) => {
                                let _ = output.send(Message::LaunchComplete(Err(e.to_string()))).await;
                                return;
                            }
                        }
                    } else {
                        let _ = output.send(Message::InstallProgress("Игра установлена".into(), 0.8)).await;
                    }
                    
                    let _ = output.send(Message::InstallProgress("Проверка модов...".into(), 0.82)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    
                    if let Err(e) = installer.download_mods().await {
                        let _ = output.send(Message::InstallProgress(format!("Моды: {}", e), 0.85)).await;
                    } else {
                        let _ = output.send(Message::InstallProgress("Моды обновлены!".into(), 0.85)).await;
                    }
                    
                    let _ = output.send(Message::InstallProgress("Проверка шейдеров...".into(), 0.86)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    
                    if let Err(e) = installer.download_shaderpacks().await {
                        let _ = output.send(Message::InstallProgress(format!("Шейдеры: {}", e), 0.88)).await;
                    } else {
                        let _ = output.send(Message::InstallProgress("Шейдеры обновлены!".into(), 0.88)).await;
                    }
                    
                    let _ = output.send(Message::InstallProgress("Проверка текстурпаков...".into(), 0.90)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    
                    if let Err(e) = installer.download_resourcepacks().await {
                        let _ = output.send(Message::InstallProgress(format!("Текстуры: {}", e), 0.92)).await;
                    } else {
                        let _ = output.send(Message::InstallProgress("Текстуры обновлены!".into(), 0.92)).await;
                    }
                    
                    let _ = output.send(Message::InstallProgress("Настройка шейдеров...".into(), 0.94)).await;
                    let _ = minecraft::configure_shaders(&game_dir, shaders_enabled);
                    
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let _ = output.send(Message::InstallProgress("Запуск игры...".into(), 0.96)).await;
                    
                    let options_path = game_dir.join("options.txt");
                    let resource_packs_line = r#"resourcePacks:["vanilla","file/Actually-3D-Stuff-1.21.zip","file/cWearable-Christmas-Hats0_8v20.zip"]"#;
                    let incompatible_line = r#"incompatibleResourcePacks:[]"#;
                    
                    if options_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&options_path) {
                            let mut lines: Vec<String> = content.lines()
                                .filter(|line| !line.starts_with("resourcePacks:") && !line.starts_with("incompatibleResourcePacks:"))
                                .map(|s| s.to_string())
                                .collect();
                            lines.push(resource_packs_line.to_string());
                            lines.push(incompatible_line.to_string());
                            let _ = std::fs::write(&options_path, lines.join("\n"));
                        }
                    } else {
                        let options_content = format!("lang:ru_ru\n{}\n{}\n", resource_packs_line, incompatible_line);
                        let _ = std::fs::write(&options_path, options_content);
                    }
                    
                    let cmd_result = build_launch_command(&game_dir, &nickname, ram_gb, Some(SERVER_ADDRESS));
                    
                    match cmd_result {
                        Ok(mut cmd) => {
                            match cmd.spawn() {
                                Ok(mut child) => {
                                    let _ = output.send(Message::InstallProgress("Игра запущена!".into(), 1.0)).await;
                                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                    let _ = output.send(Message::LaunchComplete(Ok(()))).await;
                                    
                                    let exit_status = tokio::task::spawn_blocking(move || {
                                        child.wait()
                                    }).await;
                                    
                                    let crashed = match &exit_status {
                                        Ok(Ok(status)) => !status.success(),
                                        _ => true,
                                    };
                                    
                                    if crashed {
                                        let _ = output.send(Message::GameCrashed).await;
                                    } else {
                                        let _ = output.send(Message::GameExited).await;
                                    }
                                }
                                Err(e) => {
                                    let _ = output.send(Message::LaunchComplete(Err(format!("Не удалось запустить игру: {}", e)))).await;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = output.send(Message::LaunchComplete(Err(e.to_string()))).await;
                        }
                    }
                })
            );
            Subscription::batch([gif_timer, game_sub, play_timer, server_status_timer])
        } else {
            Subscription::batch([gif_timer, server_status_timer])
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let bg_handle = if !self.gif_frames.is_empty() {
            self.gif_frames[self.current_frame].clone()
        } else {
            image::Handle::from_bytes(include_bytes!("../background.png").to_vec())
        };
        
        let avatar_handle = if !self.avatar_frames.is_empty() {
            self.avatar_frames[self.current_frame % self.avatar_frames.len()].clone()
        } else {
            image::Handle::from_bytes(include_bytes!("icon.png").to_vec())
        };

        let sidebar = container(
            column![
                container(
                    column![
                        container(
                            image(avatar_handle)
                                .width(80)
                                .height(80)
                                .content_fit(iced::ContentFit::Cover)
                        )
                        .width(80)
                        .height(80)
                        .style(move |_| container::Style {
                            border: Border { 
                                radius: 8.0.into(), 
                                width: 2.0, 
                                color: Color { r: 0.3, g: 0.3, b: 0.3, a: 1.0 }
                            },
                            ..Default::default()
                        }),
                        Space::with_height(15),
                        text(if self.nickname.is_empty() { 
                            "Гость".to_string() 
                        } else { 
                            let chars: Vec<char> = self.nickname.chars().collect();
                            if chars.len() > 12 { 
                                chars[..12].iter().collect::<String>() + ".."
                            } else { 
                                self.nickname.clone() 
                            }
                        })
                        .size(18)
                        .style(move |_| text::Style { color: Some(TEXT_PRIMARY) }),
                        Space::with_height(6),
                        container(
                            text("PREMIUM").size(9)
                        )
                        .padding([4, 14])
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(ACCENT)),
                            border: Border { radius: 12.0.into(), ..Default::default() },
                            shadow: Shadow {
                                color: Color { r: 1.0, g: 0.2, b: 0.2, a: 0.7 },
                                offset: Vector::new(0.0, 0.0),
                                blur_radius: 12.0,
                            },
                            ..Default::default()
                        }),
                    ].spacing(0).align_x(Alignment::Center).width(Length::Fill)
                )
                .width(Length::Fill)
                .padding(iced::Padding { top: 25.0, right: 15.0, bottom: 20.0, left: 15.0 }),
                
                Space::with_height(15),

                sidebar_button("ГЛАВНАЯ", Tab::Dashboard, &self.active_tab),
                sidebar_button("СТАТИСТИКА", Tab::Statistics, &self.active_tab),
                sidebar_button("НАСТРОЙКИ", Tab::Settings, &self.active_tab),
                
                Space::with_height(Length::Fill),
                
                text("ByStep v1.0.7").size(10).color(Color { r: 0.4, g: 0.4, b: 0.4, a: 1.0 }),
            ]
            .padding(18)
            .spacing(6)
        )
        .width(200)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(Color { r: 0.05, g: 0.05, b: 0.08, a: 0.75 })),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 },
            },
            ..Default::default()
        });

        let content_area = container(
            match self.active_tab {
                Tab::Dashboard => self.dashboard_view(),
                Tab::Statistics => self.statistics_view(),
                Tab::Settings => self.settings_view(),
            }
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(40);

        let main_content = stack![
            image(bg_handle)
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Cover),
            
            container(Space::new(Length::Fill, Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(Color { r: 0.0, g: 0.0, b: 0.02, a: 0.5 })),
                    ..Default::default()
                }),

            row![
                sidebar,
                content_area
            ]
        ];

        let crash_dialog: Element<'_, Message> = if self.show_crash_dialog {
            container(
                container(
                    column![
                        text("Не удалось войти в игру?").size(18).color(TEXT_PRIMARY),
                        Space::with_height(10),
                        text("Игра завершилась с ошибкой несколько раз.\nРекомендуем переустановить файлы игры.").size(13).color(TEXT_SECONDARY),
                        Space::with_height(20),
                        row![
                            button(
                                container(text("Переустановить").size(14)).padding([10, 20])
                            )
                            .on_press(Message::ReinstallGame)
                            .style(move |_, status| {
                                let hovered = status == button::Status::Hovered;
                                button::Style {
                                    background: Some(iced::Background::Color(
                                        if hovered { Color { r: 0.95, g: 0.25, b: 0.25, a: 1.0 } }
                                        else { ACCENT }
                                    )),
                                    text_color: Color::WHITE,
                                    border: Border { radius: 8.0.into(), ..Default::default() },
                                    shadow: Shadow {
                                        color: Color { r: 1.0, g: 0.2, b: 0.2, a: 0.6 },
                                        offset: Vector::new(0.0, 0.0),
                                        blur_radius: 12.0,
                                    },
                                    ..Default::default()
                                }
                            }),
                            Space::with_width(10),
                            button(
                                container(text("Закрыть").size(14)).padding([10, 20])
                            )
                            .on_press(Message::DismissCrashDialog)
                            .style(move |_, status| {
                                let hovered = status == button::Status::Hovered;
                                button::Style {
                                    background: Some(iced::Background::Color(
                                        if hovered { Color { r: 0.25, g: 0.25, b: 0.28, a: 1.0 } }
                                        else { Color { r: 0.15, g: 0.15, b: 0.18, a: 1.0 } }
                                    )),
                                    text_color: TEXT_SECONDARY,
                                    border: Border { radius: 8.0.into(), width: 1.0, color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 } },
                                    ..Default::default()
                                }
                            }),
                        ]
                    ].align_x(Alignment::Center)
                )
                .padding(30)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(Color { r: 0.08, g: 0.08, b: 0.1, a: 0.98 })),
                    border: Border { radius: 15.0.into(), width: 1.0, color: ACCENT },
                    ..Default::default()
                })
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.7 })),
                ..Default::default()
            })
            .into()
        } else {
            Space::new(0, 0).into()
        };

        stack![
            container(main_content)
                .width(Length::Fill)
                .height(Length::Fill),
            crash_dialog
        ].into()
    }

    fn dashboard_view(&self) -> Element<'_, Message> {
        let (button_text, button_enabled) = match &self.launch_state {
            LaunchState::CheckingUpdate => ("ПРОВЕРКА...", false),
            LaunchState::UpdateAvailable { .. } => ("ИГРАТЬ", false),
            LaunchState::Updating { .. } => ("ОБНОВЛЕНИЕ...", false),
            LaunchState::Idle => ("ИГРАТЬ", !self.nickname.is_empty()),
            LaunchState::Installing { .. } => ("УСТАНОВКА...", false),
            LaunchState::Launching => ("ЗАПУСК...", false),
            LaunchState::Playing => ("В ИГРЕ", false),
            LaunchState::Error(_) => ("ПОВТОРИТЬ", true),
        };

        let status_widget: Element<'_, Message> = match &self.launch_state {
            LaunchState::CheckingUpdate => {
                container(
                    text("Проверка обновлений...").size(14).color(TEXT_SECONDARY)
                )
                .padding(15)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(BG_CARD)),
                    border: Border { radius: 8.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .width(Length::Fill)
                .into()
            }
            LaunchState::UpdateAvailable { version, .. } => {
                container(
                    column![
                        text(format!("Доступно обновление v{}", version)).size(16).color(ACCENT),
                        Space::with_height(10),
                        text("Хотите обновить сейчас?").size(13).color(TEXT_SECONDARY),
                        Space::with_height(15),
                        row![
                            button(
                                container(text("Обновить").size(14)).padding([8, 20])
                            )
                            .on_press(Message::AcceptUpdate)
                            .style(move |_, status| {
                                let hovered = status == button::Status::Hovered;
                                button::Style {
                                    background: Some(iced::Background::Color(
                                        if hovered { Color { r: 0.95, g: 0.25, b: 0.25, a: 1.0 } } 
                                        else { ACCENT }
                                    )),
                                    text_color: Color::WHITE,
                                    border: Border { radius: 8.0.into(), ..Default::default() },
                                    shadow: Shadow {
                                        color: Color { r: 1.0, g: 0.2, b: 0.2, a: 0.7 },
                                        offset: Vector::new(0.0, 0.0),
                                        blur_radius: 15.0,
                                    },
                                    ..Default::default()
                                }
                            }),
                            Space::with_width(10),
                            button(
                                container(text("Позже").size(14)).padding([8, 20])
                            )
                            .on_press(Message::DeclineUpdate)
                            .style(move |_, status| {
                                let hovered = status == button::Status::Hovered;
                                button::Style {
                                    background: Some(iced::Background::Color(
                                        if hovered { Color { r: 0.25, g: 0.25, b: 0.28, a: 1.0 } } 
                                        else { Color { r: 0.15, g: 0.15, b: 0.18, a: 1.0 } }
                                    )),
                                    text_color: TEXT_SECONDARY,
                                    border: Border { radius: 8.0.into(), width: 1.0, color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 } },
                                    ..Default::default()
                                }
                            }),
                        ]
                    ].align_x(Alignment::Center)
                )
                .padding(20)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(BG_CARD)),
                    border: Border { radius: 10.0.into(), width: 1.0, color: ACCENT },
                    ..Default::default()
                })
                .width(Length::Fill)
                .into()
            }
            LaunchState::Updating { progress } => {
                container(
                    column![
                        text(progress).size(14).color(ACCENT),
                        Space::with_height(5),
                        text("Пожалуйста, подождите...").size(12).color(TEXT_SECONDARY),
                    ].align_x(Alignment::Center)
                )
                .padding(20)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(BG_CARD)),
                    border: Border { radius: 10.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .width(Length::Fill)
                .into()
            }
            LaunchState::Installing { step, progress } => {
                container(
                    column![
                        text(step).size(14).color(TEXT_PRIMARY),
                        Space::with_height(10),
                        container(
                            container(Space::new(Length::Fill, Length::Fill))
                                .width(Length::FillPortion((*progress * 100.0) as u16))
                                .style(move |_| container::Style {
                                    background: Some(iced::Background::Color(ACCENT)),
                                    border: Border { radius: 3.0.into(), ..Default::default() },
                                    ..Default::default()
                                })
                        )
                        .width(Length::Fill)
                        .height(6)
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(Color { r: 0.2, g: 0.2, b: 0.2, a: 1.0 })),
                            border: Border { radius: 3.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                        Space::with_height(5),
                        text(format!("{}%", (*progress * 100.0) as u32)).size(12).color(ACCENT),
                    ].align_x(Alignment::Center)
                )
                .padding(20)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(BG_CARD)),
                    border: Border { radius: 10.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .width(Length::Fill)
                .into()
            }
            LaunchState::Error(e) => {
                container(
                    text(format!("Ошибка: {}", e)).size(14).color(Color { r: 1.0, g: 0.4, b: 0.4, a: 1.0 })
                )
                .padding(15)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(Color { r: 0.3, g: 0.1, b: 0.1, a: 0.8 })),
                    border: Border { radius: 8.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .width(Length::Fill)
                .into()
            }
            _ => Space::with_height(0).into()
        };

        let update_icon = image::Handle::from_bytes(include_bytes!("icons8-обновление-96.png").to_vec());
        let update_button = button(
            container(
                image(update_icon)
                    .width(24)
                    .height(24)
            ).padding([6, 8])
        )
        .on_press(Message::CheckUpdate)
        .style(move |_, status| {
            let hovered = status == button::Status::Hovered;
            button::Style {
                background: Some(iced::Background::Color(
                    if hovered { Color { r: 0.2, g: 0.2, b: 0.22, a: 0.9 } } 
                    else { Color { r: 0.12, g: 0.12, b: 0.14, a: 0.8 } }
                )),
                text_color: TEXT_SECONDARY,
                border: Border { radius: 8.0.into(), width: 1.0, color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 } },
                shadow: Shadow::default(),
                ..Default::default()
            }
        });

        let server_status_widget = container(
            column![
                row![
                    container(
                        Space::new(8, 8)
                    ).style(move |_| container::Style {
                        background: Some(iced::Background::Color(
                            if self.server_status.online { Color { r: 0.2, g: 0.8, b: 0.2, a: 1.0 } }
                            else { Color { r: 0.8, g: 0.2, b: 0.2, a: 1.0 } }
                        )),
                        border: Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                    Space::with_width(10),
                    text(if self.server_status.online { "СЕРВЕР ОНЛАЙН" } else { "СЕРВЕР ОФЛАЙН" })
                        .size(12)
                        .color(TEXT_SECONDARY),
                    Space::with_width(Length::Fill),
                    text(format!("{}/{}", self.server_status.players_online, self.server_status.players_max))
                        .size(14)
                        .color(if self.server_status.online { ACCENT } else { TEXT_SECONDARY }),
                ].align_y(Alignment::Center),
                if !self.server_status.player_names.is_empty() {
                    Element::from(
                        column![
                            Space::with_height(8),
                            text(self.server_status.player_names.join(", "))
                                .size(12)
                                .color(TEXT_SECONDARY)
                        ]
                    )
                } else {
                    Element::from(Space::with_height(0))
                }
            ]
        )
        .padding(15)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(BG_CARD)),
            border: Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        })
        .width(Length::Fill);

        column![
            row![
                column![
                    text("ГЛАВНАЯ").size(36).font(iced::Font::MONOSPACE).style(move |_| text::Style { color: Some(TEXT_PRIMARY) }),
                    text("Добро пожаловать в ByStep").size(14).color(TEXT_SECONDARY),
                ],
                Space::with_width(Length::Fill),
                update_button,
            ].align_y(Alignment::Start),

            Space::with_height(20),
            server_status_widget,
            Space::with_height(10),
            status_widget,
            Space::with_height(Length::Fill),

            container(
                column![
                    row![
                        column![
                            text("ВЕРСИЯ").size(11).color(TEXT_SECONDARY),
                            text("1.21.1 Fabric").size(14).color(TEXT_PRIMARY),
                        ].spacing(3),
                        Space::with_width(40),
                        column![
                            text("ОЗУ").size(11).color(TEXT_SECONDARY),
                            text(format!("{} ГБ", self.ram_gb)).size(14).color(ACCENT),
                        ].spacing(3),
                        Space::with_width(40),
                        column![
                            text("ШЕЙДЕРЫ").size(11).color(TEXT_SECONDARY),
                            toggler(self.shaders_enabled)
                                .on_toggle(Message::ShadersToggled)
                                .size(20)
                        ].spacing(3),
                        Space::with_width(Length::Fill),

                        button(
                            container(text(button_text).size(18))
                                .padding([12, 50])
                        )
                        .on_press_maybe(if button_enabled { Some(Message::LaunchGame) } else { None })
                        .style(move |_, status| {
                            let active = status == button::Status::Hovered && button_enabled;
                            button::Style {
                                background: Some(iced::Background::Color(
                                    if !button_enabled { Color { r: 0.3, g: 0.3, b: 0.3, a: 1.0 } }
                                    else if active { Color { r: 0.95, g: 0.25, b: 0.25, a: 1.0 } } 
                                    else { ACCENT }
                                )),
                                text_color: Color::WHITE,
                                border: Border { radius: 10.0.into(), width: 0.0, color: Color::TRANSPARENT },
                                shadow: if button_enabled {
                                    Shadow {
                                        color: Color { r: 1.0, g: 0.2, b: 0.2, a: 0.8 },
                                        offset: Vector::new(0.0, 0.0),
                                        blur_radius: 25.0,
                                    }
                                } else {
                                    Shadow::default()
                                },
                                ..Default::default()
                            }
                        }),
                    ].align_y(Alignment::Center)
                ]
                .padding(25)
            )
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(BG_CARD)),
                border: Border { radius: 15.0.into(), color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 }, width: 1.0 },
                ..Default::default()
            })
            .width(Length::Fill)
        ].into()
    }

    fn settings_view(&self) -> Element<'_, Message> {
        column![
            text("НАСТРОЙКИ").size(36).font(iced::Font::MONOSPACE).style(move |_| text::Style { color: Some(TEXT_PRIMARY) }),
            Space::with_height(30),
            
            container(
                column![
                    column![
                        text("НИКНЕЙМ").size(12).color(TEXT_SECONDARY),
                        text_input("Введите ник...", &self.nickname)
                            .on_input(Message::NicknameChanged)
                            .padding(14)
                            .style(input_style)
                    ].spacing(8),

                    Space::with_height(20),

                    column![
                        row![
                            text("ПАМЯТЬ (ГБ)").size(12).color(TEXT_SECONDARY),
                            Space::with_width(Length::Fill),
                            text(format!("{}", self.ram_gb)).size(14).color(ACCENT),
                        ],
                        slider(2..=16, self.ram_gb, Message::RamChanged)
                            .step(1u32)
                            .style(slider_style)
                    ].spacing(12),

                    Space::with_height(30),

                    column![
                        text("ПЕРЕУСТАНОВКА").size(12).color(TEXT_SECONDARY),
                        Space::with_height(8),
                        button(
                            container(text("Удалить файлы игры").size(14)).padding([10, 20])
                        )
                        .on_press(Message::ReinstallGame)
                        .style(move |_, status| {
                            let hovered = status == button::Status::Hovered;
                            button::Style {
                                background: Some(iced::Background::Color(
                                    if hovered { Color { r: 0.4, g: 0.1, b: 0.1, a: 1.0 } }
                                    else { Color { r: 0.3, g: 0.08, b: 0.08, a: 1.0 } }
                                )),
                                text_color: Color { r: 1.0, g: 0.4, b: 0.4, a: 1.0 },
                                border: Border { radius: 8.0.into(), width: 1.0, color: Color { r: 0.5, g: 0.15, b: 0.15, a: 1.0 } },
                                ..Default::default()
                            }
                        }),
                        Space::with_height(5),
                        text("Удалит все файлы игры для переустановки").size(11).color(TEXT_SECONDARY),
                    ].spacing(0),
                ]
                .padding(30)
            )
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(BG_CARD)),
                border: Border { radius: 15.0.into(), color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }, width: 1.0 },
                ..Default::default()
            })
            .width(Length::Fill)
            .max_width(500)
        ].into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn save_settings(&self) {
        if let Some(config_dir) = Self::get_config_dir() {
            let settings = LauncherSettings { 
                nickname: self.nickname.clone(), 
                ram_gb: self.ram_gb,
                shaders_enabled: self.shaders_enabled,
            };
            if let Ok(json) = serde_json::to_string_pretty(&settings) {
                let _ = std::fs::write(config_dir.join("settings.json"), json);
            }
        }
    }

    fn load_settings() -> Option<LauncherSettings> {
        let config_dir = Self::get_config_dir()?;
        let content = std::fs::read_to_string(config_dir.join("settings.json")).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn get_config_dir() -> Option<PathBuf> {
        let config_dir = directories::ProjectDirs::from("com", "bystep", "launcher")?.config_dir().to_path_buf();
        std::fs::create_dir_all(&config_dir).ok()?;
        Some(config_dir)
    }

    fn get_game_data_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "bystep", "minecraft")
            .map(|dirs| dirs.data_dir().to_path_buf())
    }

    fn save_play_stats(&self) {
        if let Some(config_dir) = Self::get_config_dir() {
            if let Ok(json) = serde_json::to_string_pretty(&self.play_stats) {
                let _ = std::fs::write(config_dir.join("playtime.json"), json);
            }
        }
    }

    fn load_play_stats() -> Option<PlayTimeStats> {
        let config_dir = Self::get_config_dir()?;
        let content = std::fs::read_to_string(config_dir.join("playtime.json")).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn statistics_view(&self) -> Element<'_, Message> {
        let today = Local::now();
        let today_str = today.format("%Y-%m-%d").to_string();
        let today_seconds = self.play_stats.daily.get(&today_str).copied().unwrap_or(0);
        
        let week_seconds: u64 = (0..7)
            .filter_map(|days_ago| {
                let date = today.date_naive() - chrono::Duration::days(days_ago);
                let date_str = date.format("%Y-%m-%d").to_string();
                self.play_stats.daily.get(&date_str).copied()
            })
            .sum();
        
        let month_seconds: u64 = self.play_stats.daily.iter()
            .filter(|(date_str, _)| {
                if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    date.year() == today.year() && date.month() == today.month()
                } else {
                    false
                }
            })
            .map(|(_, &secs)| secs)
            .sum();

        let format_time = |seconds: u64| -> String {
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            if hours > 0 {
                format!("{}ч {}м", hours, minutes)
            } else {
                format!("{}м", minutes)
            }
        };

        let session_display = if self.current_session_seconds > 0 {
            format_time(self.current_session_seconds)
        } else {
            "—".to_string()
        };

        column![
            text("СТАТИСТИКА").size(36).font(iced::Font::MONOSPACE).style(move |_| text::Style { color: Some(TEXT_PRIMARY) }),
            Space::with_height(30),
            
            container(
                column![
                    row![
                        container(
                            column![
                                text("ТЕКУЩАЯ СЕССИЯ").size(11).color(TEXT_SECONDARY),
                                Space::with_height(5),
                                text(session_display.clone()).size(24).color(ACCENT),
                            ].align_x(Alignment::Center)
                        ).width(Length::Fill).padding(15),
                        
                        container(
                            column![
                                text("СЕГОДНЯ").size(11).color(TEXT_SECONDARY),
                                Space::with_height(5),
                                text(format_time(today_seconds)).size(24).color(TEXT_PRIMARY),
                            ].align_x(Alignment::Center)
                        ).width(Length::Fill).padding(15),
                    ],
                    
                    Space::with_height(10),
                    
                    row![
                        container(
                            column![
                                text("ЗА НЕДЕЛЮ").size(11).color(TEXT_SECONDARY),
                                Space::with_height(5),
                                text(format_time(week_seconds)).size(24).color(TEXT_PRIMARY),
                            ].align_x(Alignment::Center)
                        ).width(Length::Fill).padding(15),
                        
                        container(
                            column![
                                text("ЗА МЕСЯЦ").size(11).color(TEXT_SECONDARY),
                                Space::with_height(5),
                                text(format_time(month_seconds)).size(24).color(TEXT_PRIMARY),
                            ].align_x(Alignment::Center)
                        ).width(Length::Fill).padding(15),
                    ],
                    
                    Space::with_height(10),
                    
                    container(
                        column![
                            text("ВСЕГО").size(11).color(TEXT_SECONDARY),
                            Space::with_height(5),
                            text(format_time(self.play_stats.total_seconds)).size(28).color(ACCENT),
                        ].align_x(Alignment::Center)
                    ).width(Length::Fill).padding(15),
                ]
            )
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(BG_CARD)),
                border: Border { radius: 15.0.into(), color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }, width: 1.0 },
                ..Default::default()
            })
            .width(Length::Fill)
            .max_width(500)
        ].into()
    }
}

fn input_style(_: &Theme, status: iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    let focused = status == iced::widget::text_input::Status::Focused;
    iced::widget::text_input::Style {
        background: iced::Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.3 }),
        border: Border {
            radius: 8.0.into(),
            color: if focused { ACCENT } else { Color::TRANSPARENT },
            width: 1.0,
        },
        value: TEXT_PRIMARY,
        placeholder: TEXT_SECONDARY,
        icon: Color::TRANSPARENT,
        selection: Color { r: 0.85, g: 0.15, b: 0.15, a: 0.3 },
    }
}

fn slider_style(_: &Theme, _: iced::widget::slider::Status) -> iced::widget::slider::Style {
    iced::widget::slider::Style {
        rail: iced::widget::slider::Rail {
            backgrounds: (
                iced::Background::Color(ACCENT),
                iced::Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 })
            ),
            width: 4.0,
            border: Border { radius: 2.0.into(), width: 0.0, color: Color::TRANSPARENT },
        },
        handle: iced::widget::slider::Handle {
            shape: iced::widget::slider::HandleShape::Circle { radius: 8.0 },
            background: iced::Background::Color(TEXT_PRIMARY),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

fn sidebar_button<'a>(label: &'a str, tab: Tab, active_tab: &Tab) -> Element<'a, Message> {
    let is_active = tab == *active_tab;
    button(
        container(text(label).size(12).font(iced::Font::MONOSPACE).style(move |_| text::Style { color: Some(if is_active { Color::WHITE } else { TEXT_SECONDARY }) }))
            .width(Length::Fill)
            .padding([12, 20])
    )
    .on_press(Message::SwitchTab(tab))
    .style(move |_, status| {
        let hovering = status == button::Status::Hovered;
        button::Style {
            background: if is_active {
                Some(iced::Background::Color(ACCENT))
            } else if hovering {
                Some(iced::Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
            } else {
                None
            },
            text_color: if is_active { Color::WHITE } else { TEXT_SECONDARY },
            border: Border { radius: 10.0.into(), width: 0.0, color: Color::TRANSPARENT },
            shadow: if is_active {
                Shadow {
                    color: Color { r: 1.0, g: 0.2, b: 0.2, a: 0.6 },
                    offset: Vector::new(0.0, 0.0),
                    blur_radius: 15.0,
                }
            } else {
                Shadow::default()
            },
            ..Default::default()
        }
    })
    .width(Length::Fill)
    .into()
}


async fn check_for_updates() -> UpdateResult {
    let client = reqwest::Client::new();
    
    let response = match client
        .get(GITHUB_RELEASES_API)
        .header("User-Agent", "ByStep-Launcher")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return UpdateResult::Error(e.to_string()),
    };
    
    if !response.status().is_success() {
        return UpdateResult::NoUpdate;
    }
    
    let release: serde_json::Value = match response.json().await {
        Ok(r) => r,
        Err(e) => return UpdateResult::Error(e.to_string()),
    };
    
    let latest_version = release.get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v');
    
    if latest_version.is_empty() || latest_version == CURRENT_VERSION {
        return UpdateResult::NoUpdate;
    }
    
    if let Some(assets) = release.get("assets").and_then(|a| a.as_array()) {
        for asset in assets {
            let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name == INSTALLER_NAME {
                if let Some(url) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
                    return UpdateResult::UpdateAvailable(
                        latest_version.to_string(),
                        url.to_string()
                    );
                }
            }
        }
    }
    
    UpdateResult::NoUpdate
}

async fn download_and_run_update(url: String) -> UpdateResult {
    let client = reqwest::Client::new();
    
    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return UpdateResult::Error(e.to_string()),
    };
    
    if !response.status().is_success() {
        return UpdateResult::Error("Не удалось скачать обновление".to_string());
    }
    
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => return UpdateResult::Error(e.to_string()),
    };
    
    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join(INSTALLER_NAME);
    
    if let Err(e) = std::fs::write(&installer_path, &bytes) {
        return UpdateResult::Error(e.to_string());
    }
    
    UpdateResult::Downloaded(installer_path)
}

async fn fetch_server_status() -> ServerStatus {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    
    let mut status = ServerStatus::default();
    
    let stream = match TcpStream::connect_timeout(
        &"144.31.169.7:25565".parse().unwrap(),
        Duration::from_secs(5)
    ) {
        Ok(s) => s,
        Err(_) => return status,
    };
    
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    
    let mut stream = stream;
    
    let mut handshake = Vec::new();
    handshake.push(0x00);
    write_varint(&mut handshake, 767);
    write_string(&mut handshake, "144.31.169.7");
    handshake.extend_from_slice(&25565u16.to_be_bytes());
    write_varint(&mut handshake, 1);
    
    let mut packet = Vec::new();
    write_varint(&mut packet, handshake.len() as i32);
    packet.extend(handshake);
    
    if stream.write_all(&packet).is_err() {
        return status;
    }
    
    let status_request = vec![0x01, 0x00];
    if stream.write_all(&status_request).is_err() {
        return status;
    }
    
    let mut length_buf = [0u8; 5];
    let mut length_bytes = 0;
    for i in 0..5 {
        if stream.read_exact(&mut length_buf[i..i+1]).is_err() {
            return status;
        }
        length_bytes += 1;
        if length_buf[i] & 0x80 == 0 {
            break;
        }
    }
    
    let (packet_length, _) = read_varint(&length_buf[..length_bytes]);
    if packet_length <= 0 || packet_length > 65535 {
        return status;
    }
    
    let mut response_data = vec![0u8; packet_length as usize];
    if stream.read_exact(&mut response_data).is_err() {
        return status;
    }
    
    let (_, id_len) = read_varint(&response_data);
    let (json_len, json_len_size) = read_varint(&response_data[id_len..]);
    let json_start = id_len + json_len_size;
    let json_end = json_start + json_len as usize;
    
    if json_end > response_data.len() {
        return status;
    }
    
    let json_str = match std::str::from_utf8(&response_data[json_start..json_end]) {
        Ok(s) => s,
        Err(_) => return status,
    };
    
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
        status.online = true;
        
        if let Some(players) = json.get("players") {
            status.players_online = players.get("online").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            status.players_max = players.get("max").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            
            if let Some(sample) = players.get("sample").and_then(|v| v.as_array()) {
                status.player_names = sample.iter()
                    .filter_map(|p| p.get("name").and_then(|n| n.as_str()))
                    .map(|s| s.to_string())
                    .collect();
            }
        }
    }
    
    status
}

fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_varint(buf, s.len() as i32);
    buf.extend_from_slice(s.as_bytes());
}

fn read_varint(data: &[u8]) -> (i32, usize) {
    let mut result = 0i32;
    let mut shift = 0;
    let mut bytes_read = 0;
    
    for &byte in data {
        bytes_read += 1;
        result |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    
    (result, bytes_read)
}
