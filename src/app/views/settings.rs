use iced::{
    Border, Color, Element, Length,
    widget::{button, column, container, row, slider, text, text_input, Space},
};
use crate::app::state::{Message, MinecraftLauncher};
use crate::app::styles::{ACCENT, BG_CARD, TEXT_PRIMARY, TEXT_SECONDARY, input_style, slider_style};

impl MinecraftLauncher {
    pub fn settings_view(&self) -> Element<'_, Message> {
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
}
