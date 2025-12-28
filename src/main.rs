#![windows_subsystem = "windows"]

mod minecraft;
mod app;

use iced::window;
use app::{MinecraftLauncher, load_icon};

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
