use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{column, container, row, text, Space},
};
use chrono::{Local, Datelike, NaiveDate};
use crate::app::state::{Message, MinecraftLauncher};
use crate::app::styles::{ACCENT, BG_CARD, TEXT_PRIMARY, TEXT_SECONDARY};

impl MinecraftLauncher {
    pub fn statistics_view(&self) -> Element<'_, Message> {
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
