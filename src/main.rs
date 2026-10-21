#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::PhysicalPosition;
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN};

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    ui.show()?;

    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let win_w = ui.window().size().width as i32;
    ui.window()
        .set_position(PhysicalPosition::new((screen_w - win_w) / 2, 0));

    ui.run()
}