use iced::{Subscription, time};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::app::state::{Message, MinecraftLauncher, SERVER_ADDRESS};
use crate::app::utils::fetch_server_status;
use crate::minecraft::{MinecraftInstaller, get_versioned_game_directory, build_launch_command, configure_shaders};

impl MinecraftLauncher {
    pub fn subscription(&self) -> Subscription<Message> {
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
            let selected_version = self.selected_version;
            let shader_quality = self.shader_quality;
            
            let game_sub = Subscription::run_with_id(
                "game-launcher",
                iced::stream::channel(100, move |mut output| async move {
                    use iced::futures::SinkExt;
                    
                    let _ = output.send(Message::InstallProgress("Подготовка...".into(), 0.05)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    let game_dir = get_versioned_game_directory(selected_version);
                    if let Err(e) = std::fs::create_dir_all(&game_dir) {
                        let _ = output.send(Message::LaunchComplete(Err(e.to_string()))).await;
                        return;
                    }
                    
                    let installer = MinecraftInstaller::new(game_dir.clone(), selected_version);
                    
                    let _ = output.send(Message::InstallProgress("Проверка установки...".into(), 0.1)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    let is_installed = installer.is_installed().await;
                    
                    if !is_installed {
                        let _ = output.send(Message::InstallProgress(format!("Установка {}...", selected_version.display_name()), 0.15)).await;
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
                    
                    if let Err(e) = installer.download_shaderpacks(shader_quality).await {
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
                    let _ = configure_shaders(&game_dir, shader_quality, selected_version);
                    
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let _ = output.send(Message::InstallProgress("Запуск игры...".into(), 0.96)).await;
                    
                    let cmd_result = build_launch_command(&game_dir, &nickname, ram_gb, Some(SERVER_ADDRESS), selected_version);
                    
                    match cmd_result {
                        Ok(mut cmd) => {
                            match cmd.spawn() {
                                Ok(mut child) => {
                                    let _ = output.send(Message::InstallProgress("Игра запущена!".into(), 1.0)).await;
                                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                    let _ = output.send(Message::LaunchComplete(Ok(()))).await;
                                    
                                    let game_dir_clone = game_dir.clone();
                                    let exit_status = tokio::task::spawn_blocking(move || {
                                        child.wait()
                                    }).await;
                                    
                                    let crashed = match &exit_status {
                                        Ok(Ok(status)) => !status.success(),
                                        _ => true,
                                    };
                                    
                                    if crashed {
                                        let crash_log = read_crash_log(&game_dir_clone);
                                        if let Some(log) = crash_log {
                                            let _ = output.send(Message::GameCrashedWithLog(log)).await;
                                        } else {
                                            let _ = output.send(Message::GameCrashed).await;
                                        }
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
}

fn read_crash_log(game_dir: &std::path::Path) -> Option<String> {
    let crash_reports_dir = game_dir.join("crash-reports");
    let mut latest_crash: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    
    if crash_reports_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&crash_reports_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "txt") {
                    if let Ok(metadata) = path.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if latest_crash.as_ref().map_or(true, |(t, _)| modified > *t) {
                                latest_crash = Some((modified, path));
                            }
                        }
                    }
                }
            }
        }
    }
    
    if let Some((_, path)) = latest_crash {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let truncated = if content.len() > 5000 {
                format!("{}...\n[Лог обрезан]", &content[..5000])
            } else {
                content
            };
            return Some(truncated);
        }
    }
    
    let logs_dir = game_dir.join("logs");
    let latest_log = logs_dir.join("latest.log");
    if latest_log.exists() {
        if let Ok(content) = std::fs::read_to_string(&latest_log) {
            let lines: Vec<&str> = content.lines().collect();
            let last_lines: Vec<&str> = lines.iter().rev().take(100).rev().cloned().collect();
            return Some(last_lines.join("\n"));
        }
    }
    
    None
}
