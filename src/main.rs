#![windows_subsystem = "windows"]

mod minecraft;

use iced::{
    Alignment, Border, Color, Element, Length, Shadow, Subscription, Task, Theme, Vector,
    widget::{button, column, container, row, slider, text, text_input, image, stack, Space, toggler},
    window,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use minecraft::{MinecraftInstaller, get_game_directory, build_launch_command};

const ACCENT: Color = Color { r: 0.85, g: 0.15, b: 0.15, a: 1.0 }; 
const BG_SIDEBAR: Color = Color { r: 0.05, g: 0.05, b: 0.07, a: 0.98 };
const BG_CARD: Color = Color { r: 0.08, g: 0.08, b: 0.1, a: 0.85 };
const TEXT_PRIMARY: Color = Color { r: 0.98, g: 0.98, b: 1.0, a: 1.0 };
const TEXT_SECONDARY: Color = Color { r: 0.7, g: 0.73, b: 0.78, a: 1.0 };
const SERVER_ADDRESS: &str = "144.31.169.7:25565";

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

#[derive(Debug, Clone)]
enum LaunchState {
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
}

#[derive(Debug, Clone, PartialEq)]
enum Tab {
    Dashboard,
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
}

impl MinecraftLauncher {
    fn new() -> (Self, Task<Message>) {
        let settings = Self::load_settings().unwrap_or_default();
        (
            Self {
                nickname: settings.nickname,
                ram_gb: settings.ram_gb,
                shaders_enabled: settings.shaders_enabled,
                launch_state: LaunchState::Idle,
                active_tab: Tab::Dashboard,
                game_running: Arc::new(AtomicBool::new(false)),
            },
            Task::none(),
        )
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
                    Ok(_) => self.launch_state = LaunchState::Playing,
                    Err(e) => self.launch_state = LaunchState::Error(e),
                }
            }
            Message::GameExited => {
                self.launch_state = LaunchState::Idle;
                self.game_running.store(false, Ordering::SeqCst);
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.game_running.load(Ordering::SeqCst) {
            let nickname = self.nickname.clone();
            let ram_gb = self.ram_gb;
            let shaders_enabled = self.shaders_enabled;
            
            Subscription::run_with_id(
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
                    
                    // Скачиваем/обновляем моды
                    let _ = output.send(Message::InstallProgress("Проверка модов...".into(), 0.82)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    
                    if let Err(e) = installer.download_mods().await {
                        // Не критичная ошибка - продолжаем запуск
                        let _ = output.send(Message::InstallProgress(format!("Моды: {}", e), 0.85)).await;
                    } else {
                        let _ = output.send(Message::InstallProgress("Моды обновлены!".into(), 0.85)).await;
                    }
                    
                    // Скачиваем/обновляем шейдерпаки
                    let _ = output.send(Message::InstallProgress("Проверка шейдеров...".into(), 0.86)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    
                    if let Err(e) = installer.download_shaderpacks().await {
                        let _ = output.send(Message::InstallProgress(format!("Шейдеры: {}", e), 0.88)).await;
                    } else {
                        let _ = output.send(Message::InstallProgress("Шейдеры обновлены!".into(), 0.88)).await;
                    }
                    
                    // Скачиваем/обновляем текстурпаки
                    let _ = output.send(Message::InstallProgress("Проверка текстурпаков...".into(), 0.90)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    
                    if let Err(e) = installer.download_resourcepacks().await {
                        let _ = output.send(Message::InstallProgress(format!("Текстуры: {}", e), 0.92)).await;
                    } else {
                        let _ = output.send(Message::InstallProgress("Текстуры обновлены!".into(), 0.92)).await;
                    }
                    
                    // Скачиваем конфиг FancyMenu (кастомный фон)
                    let _ = output.send(Message::InstallProgress("Настройка меню...".into(), 0.93)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    
                    if let Err(e) = installer.download_fancymenu_config().await {
                        let _ = output.send(Message::InstallProgress(format!("Меню: {}", e), 0.94)).await;
                    } else {
                        let _ = output.send(Message::InstallProgress("Меню настроено!".into(), 0.94)).await;
                    }
                    
                    // Настраиваем шейдеры
                    let _ = output.send(Message::InstallProgress("Настройка шейдеров...".into(), 0.94)).await;
                    let _ = minecraft::configure_shaders(&game_dir, shaders_enabled);
                    
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let _ = output.send(Message::InstallProgress("Запуск игры...".into(), 0.96)).await;
                    
                    // Создаём options.txt с русским языком если его нет
                    let options_path = game_dir.join("options.txt");
                    if !options_path.exists() {
                        let options_content = "lang:ru_ru\nresourcePacks:[\"vanilla\",\"file/Actually-3D-Stuff-1.21.zip\"]\n";
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
                                    
                                    // Ждём в отдельном потоке
                                    tokio::task::spawn_blocking(move || {
                                        let _ = child.wait();
                                    }).await.ok();
                                    
                                    let _ = output.send(Message::GameExited).await;
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
            )
        } else {
            Subscription::none()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let bg_handle = image::Handle::from_bytes(include_bytes!("background.png").to_vec());
        let icon_handle = image::Handle::from_bytes(include_bytes!("icon.png").to_vec());

        let sidebar = container(
            column![
                container(
                    column![
                        container(
                            image(icon_handle.clone())
                                .width(80)
                                .height(80)
                                .content_fit(iced::ContentFit::Cover)
                        )
                        .width(80)
                        .height(80)
                        .style(move |_| container::Style {
                            border: Border { radius: 40.0.into(), width: 3.0, color: ACCENT },
                            ..Default::default()
                        })
                        .clip(true),
                        Space::with_height(10),
                        container(
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
                            .size(16)
                            .style(move |_| text::Style { color: Some(TEXT_PRIMARY) })
                        ).width(Length::Fill).center_x(Length::Fill),
                        text("PREMIUM").size(10).color(ACCENT),
                    ].spacing(5).align_x(Alignment::Center).width(Length::Fill)
                ).width(Length::Fill).padding(iced::Padding { top: 20.0, right: 0.0, bottom: 30.0, left: 0.0 }),

                sidebar_button("ГЛАВНАЯ", Tab::Dashboard, &self.active_tab),
                sidebar_button("НАСТРОЙКИ", Tab::Settings, &self.active_tab),
                
                Space::with_height(Length::Fill),
                
                text("ByStep v1.0").size(10).color(Color { r: 0.3, g: 0.3, b: 0.3, a: 1.0 }),
            ]
            .padding(25)
            .spacing(8)
        )
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(BG_SIDEBAR)),
            ..Default::default()
        });

        let content_area = stack![
            image(bg_handle)
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Cover),
            
            container(Space::new(Length::Fill, Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(Color { r: 0.0, g: 0.0, b: 0.02, a: 0.55 })),
                    ..Default::default()
                }),

            container(
                match self.active_tab {
                    Tab::Dashboard => self.dashboard_view(),
                    Tab::Settings => self.settings_view(),
                }
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(40)
        ];

        row![
            sidebar,
            container(content_area).width(Length::FillPortion(4))
        ].into()
    }

    fn dashboard_view(&self) -> Element<'_, Message> {
        let (button_text, button_enabled) = match &self.launch_state {
            LaunchState::Idle => ("ИГРАТЬ", !self.nickname.is_empty()),
            LaunchState::Installing { .. } => ("УСТАНОВКА...", false),
            LaunchState::Launching => ("ЗАПУСК...", false),
            LaunchState::Playing => ("В ИГРЕ", false),
            LaunchState::Error(_) => ("ПОВТОРИТЬ", true),
        };

        let status_widget: Element<'_, Message> = match &self.launch_state {
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

        column![
            text("ГЛАВНАЯ").size(36).font(iced::Font::MONOSPACE).style(move |_| text::Style { color: Some(TEXT_PRIMARY) }),
            text("Добро пожаловать в ByStep").size(14).color(TEXT_SECONDARY),

            Space::with_height(20),
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
                                        color: Color { r: 0.85, g: 0.15, b: 0.15, a: 0.4 },
                                        offset: Vector::new(0.0, 4.0),
                                        blur_radius: 20.0,
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
                Some(iced::Background::Color(Color { r: 0.85, g: 0.15, b: 0.15, a: 0.2 }))
            } else if hovering {
                Some(iced::Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
            } else {
                None
            },
            text_color: if is_active { ACCENT } else { TEXT_SECONDARY },
            border: Border { radius: 10.0.into(), width: 0.0, color: Color::TRANSPARENT },
            shadow: Shadow::default(),
            ..Default::default()
        }
    })
    .width(Length::Fill)
    .into()
}

