#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! winisland: overlay ala Dynamic Island untuk Windows.
//!
//! Aplikasi tray tanpa ikon taskbar dan tanpa entri Alt+Tab. Saat idle, jendela hanya
//! berupa garis tipis yang menempel di tepi atas layar. Saat kursor menghampirinya,
//! jendela membesar dan pill di dalamnya beranimasi (lihat `ui/main.slint`).
//!
//! - `media.rs`: memantau judul dan artis media lewat event SMTC (worker thread).
//! - `autostart.rs`: mengatur autostart lewat registry (`HKCU\...\Run`).
//! - `notif_probe.rs`: probe sementara untuk `UserNotificationListener` (hanya build debug).
//!
//! Renderer dikunci ke software lewat `BackendSelector` supaya pemakaian RAM tetap kecil.

use core::ffi::c_void;
use std::rc::Rc;
use std::time::Duration;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, PhysicalPosition, PhysicalSize, Timer, TimerMode};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, GetWindowLongPtrW, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, SM_CXSCREEN,
    SW_HIDE, SW_SHOWNA, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

slint::include_modules!();
mod autostart;
mod media;

#[cfg(debug_assertions)]
mod notif_probe;

/// Jumlah maksimum percobaan memasang style jendela. Handle jendela baru tersedia
/// setelah event loop berjalan, jadi pemasangan perlu diulang sampai berhasil.
const MAX_ATTEMPTS: u32 = 20;
/// Jeda antar percobaan memasang style jendela, dalam milidetik.
const RETRY_MS: u64 = 50;

/// Lebar jendela saat idle (garis tipis di tepi atas), dalam piksel logis.
const IDLE_W: f32 = 140.0;
/// Tinggi jendela saat idle, dalam piksel logis.
const IDLE_H: f32 = 4.0;
/// Lebar jendela saat expanded, dalam piksel logis.
const FULL_W: f32 = 360.0;
/// Tinggi jendela saat expanded: pill 100 px ditambah celah 8 px dari tepi atas (nilai 8 juga ada di `ui/main.slint`).
const FULL_H: f32 = 108.0;
/// Jeda sebelum jendela mengecil setelah collapse, dalam milidetik.
/// Sedikit di atas durasi animasi (250 ms) supaya animasi tidak terpotong.
const SHRINK_DELAY_MS: u64 = 280;

/// Mengambil HWND Win32 dari jendela Slint.
/// Mengembalikan `None` bila handle belum tersedia (jendela belum dibuat oleh event loop).
fn hwnd_of(window: &slint::Window) -> Option<HWND> {
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

/// Ukuran dalam piksel logis; dikonversi ke fisik sesuai DPI, lalu dipusatkan di tepi atas.
fn place(ui: &MainWindow, w: f32, h: f32) {
    let sf = ui.window().scale_factor();
    let pw = (w * sf).round() as u32;
    let ph = (h * sf).round() as u32;
    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    ui.window().set_size(PhysicalSize::new(pw, ph));
    ui.window()
        .set_position(PhysicalPosition::new((screen_w - pw as i32) / 2, 0));
    eprintln!("[overlay] place {pw}x{ph}");
}

/// Memasang style overlay lalu menempatkan jendela dalam keadaan idle.
/// Bila handle jendela belum tersedia, diulang setiap `RETRY_MS` sampai `MAX_ATTEMPTS`.
fn try_apply(weak: slint::Weak<MainWindow>, attempt: u32) {
    let Some(ui) = weak.upgrade() else { return };
    if apply_overlay_style(&ui) {
        place(&ui, IDLE_W, IDLE_H);
        eprintln!("[overlay] ready on attempt {attempt}");
    } else if attempt < MAX_ATTEMPTS {
        Timer::single_shot(Duration::from_millis(RETRY_MS), move || {
            try_apply(weak, attempt + 1)
        });
    } else {
        eprintln!("[overlay] FAILED after {MAX_ATTEMPTS} attempts");
    }
}

fn main() -> Result<(), slint::PlatformError> {
    // kunci backend winit dan renderer software (RAM idle jauh lebih kecil daripada femtovg)
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;

    let ui = MainWindow::new()?;
    ui.show()?;

    // --- media: langganan event SMTC lewat worker thread
    // `nudge` meminta worker membaca ulang (dipakai saat hover sebagai jaring pengaman).
    let nudge = {
        let weak = ui.as_weak();
        media::spawn_watcher(move |info| {
            let (title, artist) = match info {
                Some(m) => (m.title, m.artist),
                None => ("Nothing playing".to_string(), String::new()),
            };
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_track_title(title.into());
                ui.set_track_artist(artist.into());
            });
        })
    };

    // --- hover expand/collapse
    // Saat expand, jendela membesar dulu baru konten beranimasi. Saat collapse, konten
    // mengecil dulu dan jendela baru mengecil setelah `SHRINK_DELAY_MS`.
    let shrink_timer = Rc::new(Timer::default());
    {
        let weak = ui.as_weak();
        let timer = shrink_timer.clone();
        ui.on_hover_changed(move |hovered| {
            let Some(ui) = weak.upgrade() else { return };
            if hovered {
                timer.stop();
                place(&ui, FULL_W, FULL_H); // jendela membesar dulu
                ui.set_expanded(true); // baru konten beranimasi
                let _ = nudge.send(true); // minta worker membaca ulang (jaring pengaman)
            } else {
                ui.set_expanded(false); // konten mengecil dulu
                let weak2 = weak.clone();
                timer.start(
                    TimerMode::SingleShot,
                    Duration::from_millis(SHRINK_DELAY_MS),
                    move || {
                        if let Some(ui) = weak2.upgrade() {
                            place(&ui, IDLE_W, IDLE_H); // baru jendela mengecil
                        }
                    },
                );
            }
        });
    }

    // --- tray ---
    let tray = AppTray::new()?;

    // status awal centang menu mengikuti isi registry
    tray.set_autostart(autostart::is_enabled());
    {
        let weak = tray.as_weak();
        tray.on_toggle_autostart(move |wanted| {
            let Some(tray) = weak.upgrade() else { return };
            if let Err(e) = autostart::set_enabled(wanted) {
                eprintln!("[autostart] failed to change the entry: {e}");
            }
            // selalu baca ulang dari registry agar centang menunjukkan keadaan sebenarnya
            tray.set_autostart(autostart::is_enabled());
        });
    }

    // Tampilkan atau sembunyikan overlay dari menu tray.
    let toggle = {
        let weak = ui.as_weak();
        Rc::new(move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.window().is_visible() {
                let _ = ui.hide();
            } else {
                ui.set_expanded(false);
                let _ = ui.show();
                // jendela bisa dibuat ulang saat show(): pasang style lagi
                try_apply(weak.clone(), 1);
            }
        })
    };
    {
        let t = toggle.clone();
        tray.on_toggle(move || t());
    }

    tray.on_quit(|| {
        let _ = slint::quit_event_loop();
    });

    // probe langkah 10 (hanya build debug)
    #[cfg(debug_assertions)]
    std::thread::spawn(|| {
        if let Err(e) = notif_probe::probe() {
            eprintln!("[notif] FAILED: {e}");
        }
    });

    // pasang style jendela pertama kali, dari dalam event loop (handle jendela baru tersedia di sana)
    let weak = ui.as_weak();
    Timer::single_shot(Duration::from_millis(RETRY_MS), move || try_apply(weak, 1));

    // ikon tray menjaga proses tetap hidup saat jendela disembunyikan
    slint::run_event_loop()
}
