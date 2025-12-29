use iced::{
    Alignment, Border, Color, Element, Length, Shadow, Vector,
    widget::{button, column, container, row, text, image, Space, pick_list, scrollable},
};
use crate::app::state::{LaunchState, Message, MinecraftLauncher, CHANGELOG};
use crate::app::styles::{ACCENT, BG_CARD, TEXT_PRIMARY, TEXT_SECONDARY};
use crate::minecraft::{GameVersion, ShaderQuality};

impl MinecraftLauncher {
    pub fn dashboard_view(&self) -> Element<'_, Message> {
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

        let status_widget = self.status_widget_view();
        let header_row = self.header_with_buttons();
        let server_status_widget = self.server_status_widget_view();

        column![
            header_row,
            Space::with_height(20),
            server_status_widget,
            Space::with_height(10),
            status_widget,
            Space::with_height(Length::Fill),
            self.bottom_panel(button_text, button_enabled)
        ].into()
    }

    fn header_with_buttons(&self) -> Element<'_, Message> {
        let update_icon = image::Handle::from_bytes(include_bytes!("../../icons8-обновление-96.png").to_vec());
        
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

        let changelog_button = button(
            container(text("?").size(14)).padding([6, 10])
        )
        .on_press(Message::ToggleChangelog)
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

        let changelog_panel: Element<'_, Message> = if self.show_changelog {
            container(
                scrollable(
                    column(
                        CHANGELOG.iter().map(|(ver, desc)| {
                            container(
                                column![
                                    text(format!("v{}", ver)).size(13).color(ACCENT),
                                    text(*desc).size(11).color(TEXT_SECONDARY),
                                ].spacing(2)
                            )
                            .padding([8, 10])
                            .width(Length::Fill)
                            .into()
                        }).collect::<Vec<_>>()
                    ).spacing(5)
                ).height(150)
            )
            .padding(10)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(Color { r: 0.08, g: 0.08, b: 0.1, a: 0.95 })),
                border: Border { radius: 10.0.into(), width: 1.0, color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 } },
                ..Default::default()
            })
            .width(250)
            .into()
        } else {
            Space::new(0, 0).into()
        };

        row![
            column![
                text("ГЛАВНАЯ").size(36).font(iced::Font::MONOSPACE).style(move |_| text::Style { color: Some(TEXT_PRIMARY) }),
                text("Добро пожаловать в ByStep").size(14).color(TEXT_SECONDARY),
            ],
            Space::with_width(Length::Fill),
            column![
                row![
                    update_button,
                    Space::with_width(8),
                    changelog_button,
                ],
                Space::with_height(5),
                changelog_panel,
            ].align_x(Alignment::End),
        ].align_y(Alignment::Start).into()
    }

    fn bottom_panel<'a>(&'a self, button_text: &'a str, button_enabled: bool) -> Element<'a, Message> {
        let versions: Vec<GameVersion> = GameVersion::all();
        let shader_qualities: Vec<ShaderQuality> = ShaderQuality::all();

        container(
            column![
                row![
                    column![
                        text("ВЕРСИЯ").size(11).color(TEXT_SECONDARY),
                        pick_list(
                            versions,
                            Some(self.selected_version),
                            Message::VersionChanged
                        )
                        .text_size(13)
                        .padding([8, 12])
                        .style(pick_list_style)
                        .menu_style(menu_style)
                    ].spacing(5).width(140),
                    Space::with_width(20),
                    column![
                        text("ШЕЙДЕРЫ").size(11).color(TEXT_SECONDARY),
                        pick_list(
                            shader_qualities,
                            Some(self.shader_quality),
                            Message::ShaderQualityChanged
                        )
                        .text_size(13)
                        .padding([8, 12])
                        .style(pick_list_style)
                        .menu_style(menu_style)
                    ].spacing(5).width(120),
                    Space::with_width(20),
                    column![
                        text("ОЗУ").size(11).color(TEXT_SECONDARY),
                        text(format!("{} ГБ", self.ram_gb)).size(14).color(ACCENT),
                    ].spacing(5),
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
        .into()
    }

    fn status_widget_view(&self) -> Element<'_, Message> {
        match &self.launch_state {
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
        }
    }

    fn server_status_widget_view(&self) -> Element<'_, Message> {
        container(
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
        .width(Length::Fill)
        .into()
    }
}

fn pick_list_style(_theme: &iced::Theme, _status: pick_list::Status) -> pick_list::Style {
    pick_list::Style {
        text_color: TEXT_PRIMARY,
        placeholder_color: TEXT_SECONDARY,
        handle_color: TEXT_SECONDARY,
        background: iced::Background::Color(Color { r: 0.08, g: 0.08, b: 0.1, a: 0.95 }),
        border: Border { radius: 8.0.into(), width: 0.5, color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.15 } },
    }
}

fn menu_style(_theme: &iced::Theme) -> iced::overlay::menu::Style {
    iced::overlay::menu::Style {
        text_color: TEXT_PRIMARY,
        background: iced::Background::Color(Color { r: 0.08, g: 0.08, b: 0.1, a: 0.98 }),
        border: Border { radius: 8.0.into(), width: 0.5, color: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.15 } },
        selected_text_color: Color::WHITE,
        selected_background: iced::Background::Color(ACCENT),
    }
}

impl std::fmt::Display for GameVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl std::fmt::Display for ShaderQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
