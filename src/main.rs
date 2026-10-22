#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! winisland: overlay ala Dynamic Island untuk Windows.
//!
//! Aplikasi tray tanpa ikon taskbar dan tanpa entri Alt+Tab. Saat idle, jendela hanya
//! berupa garis tipis yang menempel di tepi atas layar. Saat kursor menghampirinya,
//! jendela membesar dan pill di dalamnya beranimasi (lihat `ui/main.slint`).
//!
//! - `media.rs`: memantau judul dan artis media lewat event SMTC (worker thread).
//! - `autostart.rs`: mengatur autostart lewat registry (`HKCU\...\Run`).
//! - `config.rs`: preferensi pengguna di registry (`HKCU\Software\winisland`), saat ini
//!   sakelar privasi "tampilkan isi notifikasi".
//! - `notif.rs` dan `notif_text.rs`: membaca notifikasi toast (daftar, tanpa event) setiap
//!   pill dibuka, lalu membersihkan dan mengelompokkannya. Hasilnya tampil sebagai lencana
//!   jumlah dan tampilan notifikasi di pill (langkah 10d).
//!
//! Renderer dikunci ke software lewat `BackendSelector` supaya pemakaian RAM tetap kecil.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, ModelRc, PhysicalPosition, PhysicalSize, Timer, TimerMode, VecModel};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, SW_HIDE, SW_SHOWNA,
    WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

slint::include_modules!();

mod core;
mod plugins;

use core::{autostart, config, fullscreen, monitor};
use plugins::notif::Fetch;
use plugins::notif_text::{age_secs, format_age, ticks_from_unix_secs, Snapshot};
use plugins::{calendar, gpu, media, notif, stats, tasks, timer};

/// Jumlah maksimum percobaan memasang style jendela. Handle jendela baru tersedia
/// setelah event loop berjalan, jadi pemasangan perlu diulang sampai berhasil.
const MAX_ATTEMPTS: u32 = 20;
/// Jeda antar percobaan memasang style jendela, dalam milidetik.
const RETRY_MS: u64 = 50;

/// Batas ukuran jendela overlay (dalam piksel logis).
/// Nilai aktual per state ditentukan oleh Slint sebagai sumber kebenaran (Langkah 12).
pub const MIN_WINDOW_W: f32 = 140.0;
pub const MIN_WINDOW_H: f32 = 4.0;
pub const PEEK_WINDOW_W: f32 = 175.0;
pub const PEEK_WINDOW_H: f32 = 40.0;
pub const MAX_EXPANDED_W: f32 = 480.0;
pub const MAX_EXPANDED_H: f32 = 300.0;

/// Mengambil waktu lokal sistem saat ini dalam format "HH:MM".
pub fn local_time_string() -> String {
    #[repr(C)]
    struct SystemTimeWin32 {
        w_year: u16,
        w_month: u16,
        w_day_of_week: u16,
        w_day: u16,
        w_hour: u16,
        w_minute: u16,
        w_second: u16,
        w_milliseconds: u16,
    }

    extern "system" {
        fn GetLocalTime(lp_system_time: *mut SystemTimeWin32);
    }

    let mut st = SystemTimeWin32 {
        w_year: 0,
        w_month: 0,
        w_day_of_week: 0,
        w_day: 0,
        w_hour: 0,
        w_minute: 0,
        w_second: 0,
        w_milliseconds: 0,
    };

    unsafe {
        GetLocalTime(&mut st);
    }

    format!("{:02}:{:02}", st.w_hour, st.w_minute)
}

/// Membatasi ukuran jendela logis ke dalam batas aman (tidak melebihi batas atas).
pub fn clamp_window_size(requested_w: f32, requested_h: f32) -> (f32, f32) {
    let w = requested_w.clamp(MIN_WINDOW_W, MAX_EXPANDED_W);
    let h = requested_h.clamp(MIN_WINDOW_H, MAX_EXPANDED_H);
    (w, h)
}

/// Mengambil ukuran jendela logis saat ini dari properti Slint.
fn current_window_size(ui: &MainWindow) -> (f32, f32) {
    if ui.get_expanded() {
        clamp_window_size(ui.get_requested_width(), ui.get_requested_height())
    } else if ui.get_peek() {
        (PEEK_WINDOW_W, PEEK_WINDOW_H)
    } else {
        (MIN_WINDOW_W, MIN_WINDOW_H)
    }
}
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

/// Ukuran dalam piksel logis; dikonversi ke fisik sesuai DPI, lalu dipusatkan di tepi atas monitor pilihan.
fn place(ui: &MainWindow, w: f32, h: f32) {
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
fn re_place(ui: &MainWindow) {
    let (w, h) = current_window_size(ui);
    place(ui, w, h);
}

/// Memasang style overlay lalu menempatkan jendela dalam keadaan idle.
/// Bila handle jendela belum tersedia, diulang setiap `RETRY_MS` sampai `MAX_ATTEMPTS`.
fn try_apply(weak: slint::Weak<MainWindow>, attempt: u32) {
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

/// Penjaga agar hanya satu pembacaan notifikasi berjalan pada satu waktu.
static NOTIF_FETCHING: AtomicBool = AtomicBool::new(false);

/// Mengembalikan `NOTIF_FETCHING` ke `false` saat di-drop, termasuk bila thread pembaca
/// panic, supaya pembacaan berikutnya tidak terkunci selamanya.
struct NotifFetchGuard;

impl Drop for NotifFetchGuard {
    fn drop(&mut self) {
        NOTIF_FETCHING.store(false, Ordering::Release);
    }
}

/// Detik sejak epoch Unix untuk saat ini; 0 bila jam sistem lebih awal dari 1970.
fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Membaca notifikasi di thread latar (pembacaan WinRT memblokir 12-50 ms), lalu mengirim
/// hasilnya ke UI. Dilewati bila pembacaan sebelumnya belum selesai.
/// Isi pesan hanya dibaca bila sakelar privasi (`config::show_body`) aktif.
fn refresh_notifications(weak: slint::Weak<MainWindow>) {
    if NOTIF_FETCHING.swap(true, Ordering::AcqRel) {
        return;
    }
    let include_body = config::show_body();
    std::thread::spawn(move || {
        let _guard = NotifFetchGuard;
        let snapshot = match notif::fetch(include_body) {
            Ok(Fetch::Ready(s)) => Some(s),
            Ok(Fetch::NoAccess) => {
                eprintln!("[notif] access not granted");
                None
            }
            Err(e) => {
                eprintln!("[notif] FAILED: {e}");
                None
            }
        };
        let now = ticks_from_unix_secs(unix_now_secs());
        let _ = weak.upgrade_in_event_loop(move |ui| apply_notifications(&ui, snapshot, now));
    });
}

/// Mengisi properti notifikasi di UI; dipanggil di thread UI. `None` (akses ditolak atau
/// pembacaan gagal) mengosongkan daftar sehingga lencana tersembunyi.
///
/// Sakelar privasi dibaca SAAT INI, bukan saat pembacaan dimulai: bila pengguna baru saja
/// memilih Hidden, isi dari pembacaan yang masih berjalan tidak boleh sampai ke layar.
fn apply_notifications(ui: &MainWindow, snapshot: Option<Snapshot>, now: i64) {
    let show_body = config::show_body();
    let (total, rows) = match snapshot {
        Some(s) => {
            let rows: Vec<NotifRow> = s
                .items
                .iter()
                .map(|it| {
                    let header = if it.count > 1 {
                        format!("{} ×{}", it.app, it.count)
                    } else {
                        it.app.clone()
                    };
                    let body = if show_body {
                        it.body.clone()
                    } else {
                        String::new()
                    };
                    NotifRow {
                        header: header.into(),
                        title: it.title.clone().into(),
                        body: body.into(),
                        age: format_age(age_secs(it.time, now)).into(),
                    }
                })
                .collect();
            (s.total as i32, rows)
        }
        None => (0, Vec::new()),
    };
    let len = rows.len() as i32;
    ui.set_notifs(ModelRc::new(VecModel::from(rows)));
    ui.set_notif_total(total);
    // jaga indeks tetap valid bila daftar menyusut saat sedang dilihat
    if ui.get_notif_index() >= len {
        ui.set_notif_index(0);
    }
}

/// Memperbarui statistik sistem (CPU %, RAM %, GPU %, status baterai, dan tanggal lokal) pada UI.
fn update_system_stats(
    ui: &MainWindow,
    cpu_tracker: &mut stats::CpuTracker,
    gpu_tracker: &mut gpu::GpuTracker,
) {
    let cpu = cpu_tracker.sample();
    ui.set_cpu_pct(cpu as i32);
    ui.set_ram_pct(stats::get_ram_load() as i32);
    if gpu_tracker.is_available() {
        ui.set_has_gpu(true);
        ui.set_gpu_pct(gpu_tracker.sample() as i32);
    } else {
        ui.set_has_gpu(false);
    }
    if let Some(bat) = stats::get_battery_info() {
        ui.set_has_battery(true);
        ui.set_battery_pct(bat.percent as i32);
        ui.set_battery_charging(bat.charging);
    } else {
        ui.set_has_battery(false);
    }
    ui.set_date_text(stats::local_date_string().into());
}

/// Memperbarui properti kalender di UI dari CalendarState.
fn apply_calendar(ui: &MainWindow, state: &calendar::CalendarState) {
    ui.set_cal_title(calendar::format_month_title(state.view_year, state.view_month).into());
    let grid = state.generate_grid();
    let cells: Vec<CalendarCell> = grid
        .into_iter()
        .map(|cell| CalendarCell {
            text: cell.text.into(),
            is_current_month: cell.is_current_month,
            is_today: cell.is_today,
        })
        .collect();
    ui.set_cal_cells(ModelRc::new(VecModel::from(cells)));
}

/// Memperbarui properti timer countdown di UI.
fn apply_timer(ui: &MainWindow, timer: &timer::CountdownTimer) {
    ui.set_timer_display(timer.format_time().into());
    ui.set_timer_running(timer.is_running());
    ui.set_timer_paused(timer.is_paused());
    ui.set_timer_finished(timer.is_finished());
    ui.set_timer_progress(timer.progress());
}

/// Memperbarui properti stopwatch di UI.
fn apply_stopwatch(ui: &MainWindow, sw: &timer::Stopwatch) {
    ui.set_stopwatch_display(sw.format_time().into());
    ui.set_stopwatch_running(sw.is_running());
    ui.set_stopwatch_paused(sw.is_paused());
}

/// Memperbarui daftar tugas dan ringkasan jumlah tugas di UI.
fn apply_tasks(ui: &MainWindow, mgr: &tasks::TaskManager) {
    let rows: Vec<TaskRow> = mgr
        .tasks()
        .iter()
        .map(|t| TaskRow {
            id: t.id as i32,
            text: t.text.as_str().into(),
            done: t.done,
        })
        .collect();
    ui.set_tasks(ModelRc::new(VecModel::from(rows)));
    ui.set_tasks_remaining(mgr.remaining_count() as i32);
    ui.set_tasks_completed(mgr.completed_count() as i32);
}

fn main() -> Result<(), slint::PlatformError> {
    // baca preferensi dari registry dulu: sakelar privasi bawaannya tersembunyi
    config::load();

    // kunci backend winit dan renderer software (RAM idle jauh lebih kecil daripada femtovg)
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;

    let ui = MainWindow::new()?;
    ui.show()?;

    // --- media: langganan event SMTC lewat worker thread
    // `media_ctl` meminta worker membaca ulang (saat hover, sebagai jaring pengaman)
    // dan mengirim perintah tombol.
    let media_ctl = {
        let weak = ui.as_weak();
        media::spawn_watcher(move |info| {
            let (title, artist, playing, has_media) = match info {
                Some(m) => (m.title, m.artist, m.playing, true),
                None => ("Nothing playing".to_string(), String::new(), false, false),
            };
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_track_title(title.into());
                ui.set_track_artist(artist.into());
                ui.set_playing(playing);
                ui.set_has_media(has_media);
            });
        })
    };

    // --- tombol kontrol media: perintah dikirim ke worker, tidak memblokir UI
    {
        let ctl = media_ctl.clone();
        ui.on_play_pause(move || ctl.toggle_play_pause());
        let ctl = media_ctl.clone();
        ui.on_skip_next(move || ctl.skip_next());
        let ctl = media_ctl.clone();
        ui.on_skip_previous(move || ctl.skip_previous());
    }

    // --- hover peek & click expand (model 3-state) ---
    // Saat hover: masuk ke mode Peek (tampilkan jam, jendela tipis 34 px).
    // Saat diklik: masuk ke mode Expanded (dynamic island penuh).
    // Saat mouse keluar: collapse kembali ke garis idle 4 px setelah `SHRINK_DELAY_MS`.
    // --- timer & stopwatch state (Langkah 15) ---
    let countdown_timer = Rc::new(RefCell::new(timer::CountdownTimer::default()));
    let stopwatch = Rc::new(RefCell::new(timer::Stopwatch::new()));
    let alarm_timer = Rc::new(Timer::default());
    let sw_tick_timer = Rc::new(Timer::default());

    apply_timer(&ui, &countdown_timer.borrow());
    apply_stopwatch(&ui, &stopwatch.borrow());

    let shrink_timer = Rc::new(Timer::default());
    let clock_timer = Rc::new(Timer::default());
    let cpu_tracker = Rc::new(RefCell::new(stats::CpuTracker::new()));
    let gpu_tracker = Rc::new(RefCell::new(gpu::GpuTracker::new()));
    {
        let weak = ui.as_weak();
        let shrink_t = shrink_timer.clone();
        let clock_t = clock_timer.clone();
        let cpu_tracker_tick = cpu_tracker.clone();
        let gpu_tracker_tick = gpu_tracker.clone();
        let cpu_tracker_exit = cpu_tracker.clone();
        let gpu_tracker_exit = gpu_tracker.clone();
        let ct_hover = countdown_timer.clone();
        let sw_tick_hover = sw_tick_timer.clone();
        ui.on_hover_changed(move |hovered| {
            let Some(ui) = weak.upgrade() else { return };
            if hovered {
                shrink_t.stop();
                if !ui.get_expanded() {
                    // Masuk ke mode Peek: tampilkan jam, besarkan jendela ke ukuran Peek
                    ui.set_peek(true);
                    ui.set_clock_text(local_time_string().into());
                    place(&ui, PEEK_WINDOW_W, PEEK_WINDOW_H);

                    // Mulai timer jam 1 detik selama terlihat
                    let clock_weak = weak.clone();
                    let cpu_tracker_tick = cpu_tracker_tick.clone();
                    let gpu_tracker_tick = gpu_tracker_tick.clone();
                    let ct_tick = ct_hover.clone();
                    clock_t.start(TimerMode::Repeated, Duration::from_secs(1), move || {
                        if let Some(ui) = clock_weak.upgrade() {
                            ui.set_clock_text(local_time_string().into());
                            // Pembaruan countdown timer saat sedang berjalan di peek atau expanded
                            if ct_tick.borrow().is_running() {
                                ct_tick.borrow_mut().check_finished();
                                apply_timer(&ui, &ct_tick.borrow());
                            }
                            // Statistik sistem hanya diperbarui saat expanded (0% CPU idle saat peek/idle)
                            if ui.get_expanded() {
                                let mut tracker = cpu_tracker_tick.borrow_mut();
                                let mut gpu = gpu_tracker_tick.borrow_mut();
                                update_system_stats(&ui, &mut tracker, &mut gpu);
                            }
                        }
                    });
                }
            } else {
                // Kursor keluar: collapse ke Idle
                ui.set_expanded(false);
                ui.set_peek(false);
                clock_t.stop(); // matikan timer jam saat idle (0% CPU idle)
                sw_tick_hover.stop(); // matikan tick stopwatch saat idle (0% CPU idle)
                cpu_tracker_exit.borrow_mut().reset(); // reset delta CPU tracker
                gpu_tracker_exit.borrow_mut().reset(); // reset delta GPU tracker

                let weak2 = weak.clone();
                shrink_t.start(
                    TimerMode::SingleShot,
                    Duration::from_millis(SHRINK_DELAY_MS),
                    move || {
                        if let Some(ui) = weak2.upgrade() {
                            place(&ui, MIN_WINDOW_W, MIN_WINDOW_H);
                        }
                    },
                );
            }
        });
    }

    // --- callback saat pill di mode Peek diklik untuk expand ---
    {
        let weak = ui.as_weak();
        let media_ctl = media_ctl.clone();
        let cpu_tracker_expand = cpu_tracker.clone();
        let gpu_tracker_expand = gpu_tracker.clone();
        ui.on_expand_clicked(move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_expanded(true);
            let (w, h) = current_window_size(&ui);
            place(&ui, w, h);

            // Inisialisasi statistik sistem seketika saat dibuka
            {
                let mut tracker = cpu_tracker_expand.borrow_mut();
                tracker.reset();
                let _ = tracker.sample();
                let mut gpu = gpu_tracker_expand.borrow_mut();
                gpu.reset();
                update_system_stats(&ui, &mut tracker, &mut gpu);
            }

            media_ctl.refresh(); // minta worker membaca ulang (jaring pengaman)
            refresh_notifications(weak.clone()); // baca notifikasi di thread latar
        });
    }

    // --- penyesuaian ukuran dinamis dari Slint (Langkah 12) ---
    // Saat tab berpindah atau konten berubah saat expanded, Slint meminta Rust
    // menyesuaikan ukuran jendela secara dinamis.
    {
        let weak = ui.as_weak();
        ui.on_request_window_resize(move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.get_expanded() {
                let (w, h) = current_window_size(&ui);
                place(&ui, w, h);
            }
        });
    }

    // --- kalender (Langkah 14) ---
    let cal_state = Rc::new(RefCell::new(calendar::CalendarState::new()));
    apply_calendar(&ui, &cal_state.borrow());
    {
        let weak = ui.as_weak();
        let cal = cal_state.clone();
        ui.on_cal_prev(move || {
            let Some(ui) = weak.upgrade() else { return };
            cal.borrow_mut().prev_month();
            apply_calendar(&ui, &cal.borrow());
        });
    }
    {
        let weak = ui.as_weak();
        let cal = cal_state.clone();
        ui.on_cal_next(move || {
            let Some(ui) = weak.upgrade() else { return };
            cal.borrow_mut().next_month();
            apply_calendar(&ui, &cal.borrow());
        });
    }
    {
        let weak = ui.as_weak();
        let cal = cal_state.clone();
        ui.on_cal_today(move || {
            let Some(ui) = weak.upgrade() else { return };
            cal.borrow_mut().reset_to_today();
            apply_calendar(&ui, &cal.borrow());
        });
    }

    // --- timer & stopwatch callbacks (Langkah 15) ---

    // Timer Start
    {
        let weak = ui.as_weak();
        let ct = countdown_timer.clone();
        let at = alarm_timer.clone();
        ui.on_timer_start(move || {
            let Some(ui) = weak.upgrade() else { return };
            ct.borrow_mut().start();
            apply_timer(&ui, &ct.borrow());

            // Pasang single-shot alarm saat target waktu tercapai (0% CPU idle, tanpa polling)
            let rem = ct.borrow().remaining();
            let wake_weak = weak.clone();
            let wake_ct = ct.clone();
            at.start(TimerMode::SingleShot, rem, move || {
                if let Some(ui) = wake_weak.upgrade() {
                    wake_ct.borrow_mut().check_finished();
                    apply_timer(&ui, &wake_ct.borrow());
                    // Bangunkan overlay dan tampilkan alarm di tab Timer
                    ui.set_expanded(true);
                    ui.set_active_tab(2);
                    let (w, h) = current_window_size(&ui);
                    place(&ui, w, h);
                }
            });
        });
    }

    // Timer Pause
    {
        let weak = ui.as_weak();
        let ct = countdown_timer.clone();
        let at = alarm_timer.clone();
        ui.on_timer_pause(move || {
            let Some(ui) = weak.upgrade() else { return };
            ct.borrow_mut().pause();
            at.stop(); // hentikan alarm single-shot saat dijeda
            apply_timer(&ui, &ct.borrow());
        });
    }

    // Timer Reset
    {
        let weak = ui.as_weak();
        let ct = countdown_timer.clone();
        let at = alarm_timer.clone();
        ui.on_timer_reset(move || {
            let Some(ui) = weak.upgrade() else { return };
            ct.borrow_mut().reset();
            at.stop();
            apply_timer(&ui, &ct.borrow());
        });
    }

    // Timer Set Duration (Preset menit)
    {
        let weak = ui.as_weak();
        let ct = countdown_timer.clone();
        let at = alarm_timer.clone();
        ui.on_timer_set_min(move |mins| {
            let Some(ui) = weak.upgrade() else { return };
            at.stop();
            ct.borrow_mut()
                .set_duration(Duration::from_secs((mins.max(1) as u64) * 60));
            apply_timer(&ui, &ct.borrow());
        });
    }

    // Timer Adjust Duration (+/- menit)
    {
        let weak = ui.as_weak();
        let ct = countdown_timer.clone();
        let at = alarm_timer.clone();
        ui.on_timer_adjust_min(move |delta| {
            let Some(ui) = weak.upgrade() else { return };
            at.stop();
            ct.borrow_mut().adjust_minutes(delta);
            apply_timer(&ui, &ct.borrow());
        });
    }

    // Stopwatch Start
    {
        let weak = ui.as_weak();
        let sw = stopwatch.clone();
        let sw_tick = sw_tick_timer.clone();
        ui.on_stopwatch_start(move || {
            let Some(ui) = weak.upgrade() else { return };
            sw.borrow_mut().start();
            apply_stopwatch(&ui, &sw.borrow());

            // Timer pembaruan cepat (50ms) hanya berjalan saat stopwatch aktif
            let tick_weak = weak.clone();
            let tick_sw = sw.clone();
            sw_tick.start(TimerMode::Repeated, Duration::from_millis(50), move || {
                if let Some(ui) = tick_weak.upgrade() {
                    if tick_sw.borrow().is_running()
                        && ui.get_expanded()
                        && ui.get_active_tab() == 2
                    {
                        apply_stopwatch(&ui, &tick_sw.borrow());
                    }
                }
            });
        });
    }

    // Stopwatch Pause
    {
        let weak = ui.as_weak();
        let sw = stopwatch.clone();
        let sw_tick = sw_tick_timer.clone();
        ui.on_stopwatch_pause(move || {
            let Some(ui) = weak.upgrade() else { return };
            sw.borrow_mut().pause();
            sw_tick.stop();
            apply_stopwatch(&ui, &sw.borrow());
        });
    }

    // Stopwatch Reset
    {
        let weak = ui.as_weak();
        let sw = stopwatch.clone();
        let sw_tick = sw_tick_timer.clone();
        ui.on_stopwatch_reset(move || {
            let Some(ui) = weak.upgrade() else { return };
            sw.borrow_mut().reset();
            sw_tick.stop();
            apply_stopwatch(&ui, &sw.borrow());
        });
    }

    // --- tasks (Langkah 17) ---
    let task_mgr = Rc::new(RefCell::new(tasks::TaskManager::new()));
    apply_tasks(&ui, &task_mgr.borrow());

    {
        let weak = ui.as_weak();
        let tm = task_mgr.clone();
        ui.on_task_add(move |text| {
            let Some(ui) = weak.upgrade() else { return };
            tm.borrow_mut().add_task(&text);
            apply_tasks(&ui, &tm.borrow());
        });
    }

    {
        let weak = ui.as_weak();
        let tm = task_mgr.clone();
        ui.on_task_toggle(move |id| {
            let Some(ui) = weak.upgrade() else { return };
            tm.borrow_mut().toggle_task(id as u64);
            apply_tasks(&ui, &tm.borrow());
        });
    }

    {
        let weak = ui.as_weak();
        let tm = task_mgr.clone();
        ui.on_task_delete(move |id| {
            let Some(ui) = weak.upgrade() else { return };
            tm.borrow_mut().remove_task(id as u64);
            apply_tasks(&ui, &tm.borrow());
        });
    }

    {
        let weak = ui.as_weak();
        let tm = task_mgr.clone();
        ui.on_task_clear_completed(move || {
            let Some(ui) = weak.upgrade() else { return };
            tm.borrow_mut().clear_completed();
            apply_tasks(&ui, &tm.borrow());
        });
    }

    // --- tray ---
    let tray = AppTray::new()?;

    // status sembunyikan jendela: oleh pengguna (manual) atau oleh aplikasi fullscreen
    let user_hidden = Rc::new(Cell::new(false));
    let fs_suppressed = Rc::new(Cell::new(false));

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

    // sakelar privasi isi notifikasi: menu mengikuti keadaan di memori
    tray.set_show_notif_body(config::show_body());
    {
        let tray_weak = tray.as_weak();
        let ui_weak = ui.as_weak();
        tray.on_toggle_notif_body(move |wanted| {
            // memori diubah lebih dulu di dalam set_show_body, jadi berlaku seketika
            // walau penyimpanan ke registry gagal
            if let Err(e) = config::set_show_body(wanted) {
                eprintln!("[config] failed to save the notification body setting: {e}");
            }
            if let Some(tray) = tray_weak.upgrade() {
                tray.set_show_notif_body(config::show_body());
            }
            if let Some(ui) = ui_weak.upgrade() {
                // buang notifikasi yang sudah tampil supaya isi lama tidak tersisa;
                // hover berikutnya membaca ulang, atau sekarang bila pill sedang terbuka
                apply_notifications(&ui, None, 0);
                if ui.get_expanded() {
                    refresh_notifications(ui_weak.clone());
                }
            }
        });
    }

    // status awal label display di menu tray
    tray.set_display_label(monitor::monitor_label(config::monitor_index()).into());
    {
        let tray_weak = tray.as_weak();
        let ui_weak = ui.as_weak();
        tray.on_cycle_display(move || {
            let next_idx = monitor::next_monitor_index(config::monitor_index());
            if let Err(e) = config::set_monitor_index(next_idx) {
                eprintln!("[config] failed to save monitor index: {e}");
            }
            if let Some(tray) = tray_weak.upgrade() {
                tray.set_display_label(monitor::monitor_label(next_idx).into());
            }
            if let Some(ui) = ui_weak.upgrade() {
                re_place(&ui);
            }
        });
    }

    // status awal sakelar sembunyikan saat fullscreen di menu tray
    tray.set_hide_on_fullscreen(config::hide_on_fullscreen());
    {
        let tray_weak = tray.as_weak();
        let ui_weak = ui.as_weak();
        let user_hidden = user_hidden.clone();
        let fs_suppressed = fs_suppressed.clone();
        tray.on_toggle_hide_on_fullscreen(move |wanted| {
            if let Err(e) = config::set_hide_on_fullscreen(wanted) {
                eprintln!("[config] failed to save hide on fullscreen setting: {e}");
            }
            if let Some(tray) = tray_weak.upgrade() {
                tray.set_hide_on_fullscreen(config::hide_on_fullscreen());
            }
            if let Some(ui) = ui_weak.upgrade() {
                if !wanted && fs_suppressed.get() {
                    // Pengguna mematikan sakelar saat sedang fullscreen: buka kembali
                    fs_suppressed.set(false);
                    if !user_hidden.get() {
                        ui.set_expanded(false);
                        ui.set_peek(false);
                        let _ = ui.show();
                        try_apply(ui_weak.clone(), 1);
                    }
                }
            }
        });
    }

    // Tampilkan atau sembunyikan overlay dari menu tray.
    let toggle = {
        let weak = ui.as_weak();
        let user_hidden = user_hidden.clone();
        let fs_suppressed = fs_suppressed.clone();
        Rc::new(move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.window().is_visible() {
                user_hidden.set(true);
                let _ = ui.hide();
            } else {
                user_hidden.set(false);
                // Hanya buka jika tidak sedang di-suppress oleh fullscreen
                if !fs_suppressed.get() || !config::hide_on_fullscreen() {
                    ui.set_expanded(false);
                    ui.set_peek(false);
                    let _ = ui.show();
                    // jendela bisa dibuat ulang saat show(): pasang style lagi
                    try_apply(weak.clone(), 1);
                }
            }
        })
    };
    {
        let t = toggle.clone();
        tray.on_toggle(move || t());
    }

    // --- deteksi aplikasi fullscreen ---
    // Memeriksa secara berkala (500 ms) apakah ada aplikasi atau game layar penuh
    // yang sedang aktif di monitor tempat overlay berada. Jika ada, overlay disembunyikan.
    let _fs_timer = {
        let timer = Timer::default();
        let weak = ui.as_weak();
        let user_hidden = user_hidden.clone();
        let fs_suppressed = fs_suppressed.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(500), move || {
            let Some(ui) = weak.upgrade() else { return };
            if !config::hide_on_fullscreen() {
                if fs_suppressed.get() {
                    fs_suppressed.set(false);
                    if !user_hidden.get() {
                        ui.set_expanded(false);
                        ui.set_peek(false);
                        let _ = ui.show();
                        try_apply(weak.clone(), 1);
                    }
                }
                return;
            }

            let our_hwnd = hwnd_of(ui.window());
            let is_fs =
                fullscreen::is_foreground_fullscreen_on_monitor(our_hwnd, config::monitor_index());

            if is_fs != fs_suppressed.get() {
                fs_suppressed.set(is_fs);
                if !user_hidden.get() {
                    if is_fs {
                        ui.set_expanded(false);
                        ui.set_peek(false);
                        let _ = ui.hide();
                    } else {
                        ui.set_expanded(false);
                        ui.set_peek(false);
                        let _ = ui.show();
                        try_apply(weak.clone(), 1);
                    }
                }
            }
        });
        timer
    };

    tray.on_quit(|| {
        let _ = slint::quit_event_loop();
    });

    // uji langkah 10c (hanya build debug): baca notifikasi dan cetak ke konsol
    #[cfg(debug_assertions)]
    std::thread::spawn(notif::debug_run);

    // pasang style jendela pertama kali, dari dalam event loop (handle jendela baru tersedia di sana)
    let weak = ui.as_weak();
    Timer::single_shot(Duration::from_millis(RETRY_MS), move || try_apply(weak, 1));

    // ikon tray menjaga proses tetap hidup saat jendela disembunyikan
    slint::run_event_loop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_window_size_normal() {
        let (w, h) = clamp_window_size(380.0, 160.0);
        assert_eq!(w, 380.0);
        assert_eq!(h, 160.0);
    }

    #[test]
    fn test_clamp_window_size_max_bounds() {
        let (w, h) = clamp_window_size(600.0, 500.0);
        assert_eq!(w, MAX_EXPANDED_W);
        assert_eq!(h, MAX_EXPANDED_H);
    }

    #[test]
    fn test_clamp_window_size_min_bounds() {
        let (w, h) = clamp_window_size(50.0, 1.0);
        assert_eq!(w, MIN_WINDOW_W);
        assert_eq!(h, MIN_WINDOW_H);
    }

    #[test]
    fn test_local_time_format() {
        let s = local_time_string();
        assert_eq!(s.len(), 5);
        assert_eq!(&s[2..3], ":");
        let h: u32 = s[0..2].parse().unwrap();
        let m: u32 = s[3..5].parse().unwrap();
        assert!(h < 24);
        assert!(m < 60);
    }
}
