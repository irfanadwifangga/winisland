//! Adapter penempatan dan style jendela overlay Slint pada Windows.

use std::ffi::c_void;
use std::time::Duration;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, PhysicalPosition, PhysicalSize, Timer};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetForegroundWindow, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, SW_HIDE,
    SW_SHOWNA, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::core::{config, monitor};
use crate::MainWindow;

/// Jumlah maksimum percobaan memasang style jendela. Handle jendela baru tersedia
/// setelah event loop berjalan, jadi pemasangan perlu diulang sampai berhasil.
const MAX_ATTEMPTS: u32 = 20;
/// Jeda antar percobaan memasang style jendela, dalam milidetik.
pub(crate) const RETRY_MS: u64 = 50;

/// Batas ukuran jendela overlay (dalam piksel logis).
/// Nilai aktual per state ditentukan oleh Slint sebagai sumber kebenaran (Langkah 12).
pub(crate) const MIN_WINDOW_W: f32 = 140.0;
pub(crate) const MIN_WINDOW_H: f32 = 4.0;
pub(crate) const PEEK_WINDOW_W: f32 = 175.0;
pub(crate) const PEEK_WINDOW_H: f32 = 40.0;
pub(crate) const VOLUME_OSD_WINDOW_W: f32 = 220.0;
pub(crate) const VOLUME_OSD_WINDOW_H: f32 = 44.0;
pub(crate) const MAX_EXPANDED_W: f32 = 480.0;
// 320: tab Calendar butuh 300 px (bulan enam minggu) + celah floating 8 px.
pub(crate) const MAX_EXPANDED_H: f32 = 320.0;

/// Membatasi ukuran jendela logis ke dalam batas aman (tidak melebihi batas atas).
pub(crate) fn clamp_window_size(requested_w: f32, requested_h: f32) -> (f32, f32) {
    let w = requested_w.clamp(MIN_WINDOW_W, MAX_EXPANDED_W);
    let h = requested_h.clamp(MIN_WINDOW_H, MAX_EXPANDED_H);
    (w, h)
}

/// Mengambil ukuran jendela logis saat ini dari properti Slint.
pub(crate) fn current_window_size(ui: &MainWindow) -> (f32, f32) {
    if ui.get_expanded() {
        clamp_window_size(ui.get_requested_width(), ui.get_requested_height())
    } else if ui.get_show_volume_osd() {
        (VOLUME_OSD_WINDOW_W, VOLUME_OSD_WINDOW_H)
    } else if ui.get_peek() {
        (PEEK_WINDOW_W, PEEK_WINDOW_H)
    } else {
        (MIN_WINDOW_W, MIN_WINDOW_H)
    }
}
/// Jeda sebelum jendela mengecil setelah collapse, dalam milidetik.
/// Sedikit di atas durasi animasi (250 ms) supaya animasi tidak terpotong.
pub(crate) const SHRINK_DELAY_MS: u64 = 280;

/// Mengambil HWND Win32 dari jendela Slint.
/// Mengembalikan `None` bila handle belum tersedia (jendela belum dibuat oleh event loop).
pub(crate) fn hwnd_of(window: &slint::Window) -> Option<HWND> {
    let handle = window.window_handle();
    let wh = handle.window_handle().ok()?;
    match wh.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut c_void)),
        _ => None,
    }
}

/// Memasang style jendela overlay:
/// - `WS_EX_TOOLWINDOW`: tidak muncul di taskbar dan Alt+Tab,
/// - `WS_EX_NOACTIVATE`: tidak mencuri fokus saat diklik,
/// - `WS_EX_APPWINDOW` dilepas.
///
/// Mengembalikan `true` bila kedua style yang diinginkan terpasang.
fn apply_overlay_style(ui: &MainWindow) -> bool {
    let Some(hwnd) = hwnd_of(ui.window()) else {
        eprintln!("[overlay] HWND not available yet");
        return false;
    };
    // SAFETY: HWND berasal dari jendela Slint yang masih hidup; pemanggilan tetap di thread UI.
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        // jendela disembunyikan dulu supaya taskbar membaca ulang style-nya
        let _ = ShowWindow(hwnd, SW_HIDE);
        let wanted = (before | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) & !WS_EX_APPWINDOW.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted as isize);
        let _ = ShowWindow(hwnd, SW_SHOWNA);
        let now = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        eprintln!("[overlay] exstyle before={before:#x} after={now:#x}");
        now & WS_EX_TOOLWINDOW.0 != 0 && now & WS_EX_NOACTIVATE.0 != 0
    }
}

/// Memindahkan fokus keyboard sistem ke jendela overlay.
///
/// Jendela memakai `WS_EX_NOACTIVATE`, jadi klik tidak pernah mengaktifkannya dan
/// pengetikan (mis. kolom input Tasks) tidak pernah sampai. Windows mengizinkan
/// proses memanggil ini saat sedang menerima input mouse — tepat setelah klik.
pub(crate) fn focus_overlay(ui: &MainWindow) {
    let Some(hwnd) = hwnd_of(ui.window()) else {
        return;
    };
    // SAFETY: HWND milik jendela Slint yang masih hidup; dipanggil dari thread UI.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
    }
}

/// Ukuran dalam piksel logis; dikonversi ke fisik sesuai DPI, lalu dipusatkan di tepi atas monitor pilihan.
pub(crate) fn place(ui: &MainWindow, w: f32, h: f32) {
    let sf = ui.window().scale_factor();
    let pw = (w * sf).round() as u32;
    let ph = (h * sf).round() as u32;
    let mon = monitor::get_monitor(config::monitor_index());
    let x = mon.left + (mon.width - pw as i32) / 2;
    let y = mon.top;
    ui.window().set_size(PhysicalSize::new(pw, ph));
    ui.window().set_position(PhysicalPosition::new(x, y));
    eprintln!(
        "[overlay] place {pw}x{ph} on display {} at ({x}, {y})",
        config::monitor_index()
    );
}

/// Menempatkan ulang jendela di monitor pilihan dengan ukuran yang sesuai status expanded saat ini.
pub(crate) fn re_place(ui: &MainWindow) {
    let (w, h) = current_window_size(ui);
    place(ui, w, h);
}

/// Memasang style overlay lalu menempatkan jendela dalam keadaan idle.
/// Bila handle jendela belum tersedia, diulang setiap `RETRY_MS` sampai `MAX_ATTEMPTS`.
pub(crate) fn try_apply(weak: slint::Weak<MainWindow>, attempt: u32) {
    let Some(ui) = weak.upgrade() else { return };
    if apply_overlay_style(&ui) {
        place(&ui, MIN_WINDOW_W, MIN_WINDOW_H);
        eprintln!("[overlay] ready on attempt {attempt}");
    } else if attempt < MAX_ATTEMPTS {
        Timer::single_shot(Duration::from_millis(RETRY_MS), move || {
            try_apply(weak, attempt + 1)
        });
    } else {
        eprintln!("[overlay] FAILED after {MAX_ATTEMPTS} attempts");
    }
}
