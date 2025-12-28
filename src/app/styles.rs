use iced::{Border, Color, Theme, widget::{slider, text_input}};

pub const ACCENT: Color = Color { r: 0.85, g: 0.15, b: 0.15, a: 1.0 };
pub const BG_SIDEBAR: Color = Color { r: 0.05, g: 0.05, b: 0.07, a: 0.98 };
pub const BG_CARD: Color = Color { r: 0.08, g: 0.08, b: 0.1, a: 0.85 };
pub const TEXT_PRIMARY: Color = Color { r: 0.98, g: 0.98, b: 1.0, a: 1.0 };
pub const TEXT_SECONDARY: Color = Color { r: 0.7, g: 0.73, b: 0.78, a: 1.0 };

pub fn input_style(_: &Theme, status: text_input::Status) -> text_input::Style {
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
        selection: Color { r: 0.85, g: 0.15, b: 0.15, a: 0.3 },
    }
}

pub fn slider_style(_: &Theme, _: slider::Status) -> slider::Style {
    slider::Style {
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
    }
}
