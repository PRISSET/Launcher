use iced::{
    Alignment, Border, Color, Element, Length, Shadow, Theme, Vector,
    widget::{button, column, container, row, text, image, stack, Space},
};
use crate::app::state::{Message, MinecraftLauncher, Tab};
use crate::app::styles::{ACCENT, TEXT_PRIMARY, TEXT_SECONDARY};

impl MinecraftLauncher {
    pub fn view(&self) -> Element<'_, Message> {
        let bg_handle = if !self.gif_frames.is_empty() {
            self.gif_frames[self.current_frame].clone()
        } else {
            image::Handle::from_bytes(include_bytes!("../../background.png").to_vec())
        };
        
        let avatar_handle = if !self.avatar_frames.is_empty() {
            self.avatar_frames[self.current_frame % self.avatar_frames.len()].clone()
        } else {
            image::Handle::from_bytes(include_bytes!("../icon.png").to_vec())
        };

        let sidebar = self.sidebar_view(avatar_handle);
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

            row![sidebar, content_area]
        ];

        let crash_dialog: Element<'_, Message> = if self.show_crash_dialog {
            self.crash_dialog_view()
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

    fn sidebar_view(&self, avatar_handle: image::Handle) -> Element<'_, Message> {
        container(
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
                
                text("ByStep v1.1.0").size(10).color(Color { r: 0.4, g: 0.4, b: 0.4, a: 1.0 }),
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
        })
        .into()
    }

    fn crash_dialog_view(&self) -> Element<'_, Message> {
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
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
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
