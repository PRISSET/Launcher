use iced::{
    Alignment, Border, Color, Element, Length, Shadow, Task, Theme, Vector,
    widget::{button, column, container, row, slider, text, text_input, image, stack, Space},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const ACCENT: Color = Color { r: 0.14, g: 0.77, b: 0.37, a: 1.0 }; 
const BG_SIDEBAR: Color = Color { r: 0.05, g: 0.05, b: 0.07, a: 0.98 };
const BG_CARD: Color = Color { r: 0.08, g: 0.08, b: 0.1, a: 0.85 };
const TEXT_PRIMARY: Color = Color { r: 0.98, g: 0.98, b: 1.0, a: 1.0 };
const TEXT_SECONDARY: Color = Color { r: 0.7, g: 0.73, b: 0.78, a: 1.0 };

pub fn main() -> iced::Result {
    iced::application("ByStep Launcher", MinecraftLauncher::update, MinecraftLauncher::view)
        .theme(MinecraftLauncher::theme)
        .run_with(MinecraftLauncher::new)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LauncherSettings {
    nickname: String,
    ram_gb: u32,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            ram_gb: 4,
        }
    }
}

struct MinecraftLauncher {
    nickname: String,
    ram_gb: u32,
    is_launching: bool,
    active_tab: Tab,
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
    LaunchGame,
    SwitchTab(Tab),
}

impl MinecraftLauncher {
    fn new() -> (Self, Task<Message>) {
        let settings = Self::load_settings().unwrap_or_default();
        (
            Self {
                nickname: settings.nickname,
                ram_gb: settings.ram_gb,
                is_launching: false,
                active_tab: Tab::Dashboard,
            },
            iced::window::get_latest().and_then(|window| {
                iced::window::change_mode(window, iced::window::Mode::Fullscreen)
            }),
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
            Message::LaunchGame => {
                if !self.nickname.is_empty() {
                    self.is_launching = true;
                }
            }
            Message::SwitchTab(tab) => {
                self.active_tab = tab;
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let bg_handle = image::Handle::from_bytes(include_bytes!("background.png").to_vec());
        let avatar_handle = image::Handle::from_bytes(include_bytes!("avatar.png").to_vec());

        let sidebar = container(
            column![
                container(
                    column![
                        container(
                            image(avatar_handle.clone())
                                .width(80)
                                .height(80)
                                .content_fit(iced::ContentFit::Cover)
                        )
                        .style(move |_| container::Style {
                            border: Border { radius: 40.0.into(), width: 3.0, color: ACCENT },
                            ..Default::default()
                        })
                        .clip(true),
                        Space::with_height(10),
                        text(if self.nickname.is_empty() { "Гость" } else { &self.nickname })
                            .size(18).style(move |_| text::Style { color: Some(TEXT_PRIMARY) }),
                        text("PREMIUM").size(10).color(ACCENT),
                    ].spacing(5).align_x(Alignment::Center)
                ).width(Length::Fill).padding(iced::Padding { top: 20.0, right: 0.0, bottom: 30.0, left: 0.0 }),

                sidebar_button("ГЛАВНАЯ", Tab::Dashboard, &self.active_tab),
                sidebar_button("НАСТРОЙКИ", Tab::Settings, &self.active_tab),
                
                Space::with_height(Length::Fill),
                
                text("ByStep v4.0").size(10).color(Color { r: 0.3, g: 0.3, b: 0.3, a: 1.0 }),
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
        column![
            text("ГЛАВНАЯ").size(36).font(iced::Font::MONOSPACE).style(move |_| text::Style { color: Some(TEXT_PRIMARY) }),
            text("Добро пожаловать в ByStep").size(14).color(TEXT_SECONDARY),

            Space::with_height(Length::Fill),

            container(
                column![
                    row![
                        column![
                            text("ВЕРСИЯ").size(11).color(TEXT_SECONDARY),
                            text("1.21.1").size(14).color(TEXT_PRIMARY),
                        ].spacing(3),
                        Space::with_width(40),
                        column![
                            text("ОЗУ").size(11).color(TEXT_SECONDARY),
                            text(format!("{} ГБ", self.ram_gb)).size(14).color(ACCENT),
                        ].spacing(3),
                        Space::with_width(Length::Fill),

                        button(
                            container(text(if self.is_launching { "ЗАГРУЗКА..." } else { "ИГРАТЬ" }).size(18))
                                .padding([12, 50])
                        )
                        .on_press(Message::LaunchGame)
                        .style(move |_, status| {
                            let active = status == button::Status::Hovered;
                            button::Style {
                                background: Some(iced::Background::Color(if active { Color { r: 0.18, g: 0.85, b: 0.42, a: 1.0 } } else { ACCENT })),
                                text_color: Color::WHITE,
                                border: Border { radius: 10.0.into(), width: 0.0, color: Color::TRANSPARENT },
                                shadow: Shadow {
                                    color: Color { r: 0.14, g: 0.77, b: 0.37, a: 0.4 },
                                    offset: Vector::new(0.0, 4.0),
                                    blur_radius: 20.0,
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
                            .style(move |_, status| {
                                let focused = status == text_input::Status::Focused;
                                text_input::Style {
                                    background: iced::Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.3 }),
                                    border: Border {
                                        radius: 8.0.into(),
                                        color: if focused { ACCENT } else { Color::TRANSPARENT },
                                        width: 1.0,
                                    },
                                    value: TEXT_PRIMARY,
                                    placeholder: TEXT_SECONDARY,
                                    icon: Color::TRANSPARENT,
                                    selection: Color { r: 0.14, g: 0.77, b: 0.37, a: 0.3 },
                                }
                            })
                    ].spacing(8),

                    Space::with_height(30),

                    column![
                        row![
                            text("ПАМЯТЬ (ГБ)").size(12).color(TEXT_SECONDARY),
                            Space::with_width(Length::Fill),
                            text(format!("{}", self.ram_gb)).size(14).color(ACCENT),
                        ],
                        slider(2..=16, self.ram_gb, Message::RamChanged)
                            .step(1u32)
                            .style(move |_, _| slider::Style {
                                rail: slider::Rail {
                                    backgrounds: (
                                        iced::Background::Color(ACCENT),
                                        iced::Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 })
                                    ),
                                    width: 4.0,
                                    border: Border { radius: 2.0.into(), width: 0.0, color: Color::TRANSPARENT },
                                },
                                handle: slider::Handle {
                                    shape: slider::HandleShape::Circle { radius: 8.0 },
                                    background: iced::Background::Color(TEXT_PRIMARY),
                                    border_width: 0.0,
                                    border_color: Color::TRANSPARENT,
                                },
                            })
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
            let settings = LauncherSettings { nickname: self.nickname.clone(), ram_gb: self.ram_gb };
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
                Some(iced::Background::Color(Color { r: 0.14, g: 0.77, b: 0.37, a: 0.2 }))
            } else if hovering {
                Some(iced::Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
            } else {
                None
            },
            text_color: if is_active { ACCENT } else { TEXT_SECONDARY },
            border: Border { radius: 10.0.into(), width: 0.0, color: Color::TRANSPARENT },
            shadow: Shadow { ..Shadow::default() },
            ..Default::default()
        }
    })
    .width(Length::Fill)
    .into()
}
