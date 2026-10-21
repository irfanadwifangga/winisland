#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use core::ffi::c_void;
use std::rc::Rc;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, PhysicalPosition, PhysicalSize, Timer, TimerMode};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, GetWindowLongPtrW, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE,
    SM_CXSCREEN, SW_HIDE, SW_SHOWNA, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

slint::include_modules!();
mod autostart;
mod media;

#[cfg(debug_assertions)]
mod notif_probe;

const MAX_ATTEMPTS: u32 = 20;
const RETRY_MS: u64 = 50;

const IDLE_W: f32 = 140.0;
const IDLE_H: f32 = 4.0;
const FULL_W: f32 = 360.0;
const FULL_H: f32 = 108.0;
const SHRINK_DELAY_MS: u64 = 280; // sedikit di atas durasi animasi (250 ms)

fn hwnd_of(window: &slint::Window) -> Option<HWND> {
    let handle = window.window_handle();
    let wh = handle.window_handle().ok()?;
    match wh.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut c_void)),
        _ => None,
    }
}

fn apply_overlay_style(ui: &MainWindow) -> bool {
    let Some(hwnd) = hwnd_of(ui.window()) else {
        eprintln!("[overlay] HWND belum tersedia");
        return false;
    };
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let _ = ShowWindow(hwnd, SW_HIDE);
        let wanted = (before | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) & !WS_EX_APPWINDOW.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted as isize);
        let _ = ShowWindow(hwnd, SW_SHOWNA);
        let now = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        eprintln!("[overlay] exstyle sebelum={before:#x} sesudah={now:#x}");
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

static FETCHING: AtomicBool = AtomicBool::new(false);

/// Baca info media di thread latar, lalu kirim hasilnya ke UI.
/// Lewati bila pembacaan sebelumnya belum selesai (mencegah tumpukan thread
/// saat kursor keluar-masuk dengan cepat).
fn refresh_media(weak: slint::Weak<MainWindow>) {
    if FETCHING.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(move || {
        let (title, artist) = match media::read_now_playing() {
            Ok(Some(m)) => (m.title, m.artist),
            Ok(None) => ("Nothing playing".to_string(), String::new()),
            Err(e) => {
                eprintln!("[media] GAGAL: {e}");
                ("Nothing playing".to_string(), String::new())
            }
        };
        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.set_track_title(title.into());
            ui.set_track_artist(artist.into());
        });
        FETCHING.store(false, Ordering::Release);
    });
}

fn try_apply(weak: slint::Weak<MainWindow>, attempt: u32) {
    let Some(ui) = weak.upgrade() else { return };
    if apply_overlay_style(&ui) {
        place(&ui, IDLE_W, IDLE_H);
        eprintln!("[overlay] siap pada percobaan ke-{attempt}");
    } else if attempt < MAX_ATTEMPTS {
        Timer::single_shot(Duration::from_millis(RETRY_MS), move || {
            try_apply(weak, attempt + 1)
        });
    } else {
        eprintln!("[overlay] GAGAL setelah {MAX_ATTEMPTS} percobaan");
    }
}

fn main() -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
    .backend_name("winit".into())
    .renderer_name("software".into())
    .select()?;
    
    let ui = MainWindow::new()?;
    ui.show()?;

    // --- hover expand/collapse
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
                refresh_media(weak.clone()); // 9b: baca media di latar
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
                eprintln!("[autostart] gagal mengubah entri: {e}");
            }
            // selalu baca ulang dari registry agar centang menunjukkan keadaan sebenarnya
            tray.set_autostart(autostart::is_enabled());
        });
    }

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
            eprintln!("[notif] GAGAL: {e}");
        }
    });
    
     // pasang style jendela pertama kali (sama seperti sebelumnya)
    let weak = ui.as_weak();
    Timer::single_shot(Duration::from_millis(RETRY_MS), move || try_apply(weak, 1));

    slint::run_event_loop()
}