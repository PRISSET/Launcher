use iced::Task;
use std::sync::atomic::Ordering;
use discord_rich_presence::{activity, DiscordIpc};
use crate::app::state::{LaunchState, Message, MinecraftLauncher, UpdateResult};
use crate::app::utils::{check_for_updates, download_and_run_update};

impl MinecraftLauncher {
    pub fn update(&mut self, message: Message) -> Task<Message> {
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
                    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
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

    pub fn update_discord_presence(&self, state: &str, details: &str) {
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

    pub fn clear_discord_presence(&self) {
        if let Ok(mut guard) = self.discord_client.lock() {
            if let Some(client) = guard.as_mut() {
                let _ = client.clear_activity();
            }
        }
    }
}
