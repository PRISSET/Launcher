use iced::{Subscription, time};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::app::state::{Message, MinecraftLauncher, SERVER_ADDRESS};
use crate::app::utils::fetch_server_status;
use crate::minecraft::{MinecraftInstaller, get_game_directory, build_launch_command, configure_shaders};

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
                        let _ = output.send(Message::InstallProgress("Установка 1.21.1 Fabric...".into(), 0.15)).await;
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
                    let _ = configure_shaders(&game_dir, shaders_enabled);
                    
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let _ = output.send(Message::InstallProgress("Запуск игры...".into(), 0.96)).await;
                    
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
}
