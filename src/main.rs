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
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};

slint::include_modules!();

mod bindings;
mod core;
mod plugins;

use crate::core::window::{
    current_window_size, focus_overlay, hwnd_of, place, re_place, try_apply, MIN_WINDOW_H,
    MIN_WINDOW_W, PEEK_WINDOW_H, PEEK_WINDOW_W, RETRY_MS, SHRINK_DELAY_MS, VOLUME_OSD_WINDOW_H,
    VOLUME_OSD_WINDOW_W,
};
use crate::core::{autostart, config, fullscreen, monitor};
use crate::plugins::{gpu, stats, timer, volume};
// `notif::debug_run` hanya ada di build debug; impor tidak boleh bocor ke build rilis.
#[cfg(debug_assertions)]
use crate::plugins::notif;

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

/// Menjalankan ulang tick stopwatch (50 ms) bila stopwatch berjalan dan tab Timer
/// sedang tampil. Dipanggil saat stopwatch dimulai, saat pill di-expand ulang, dan
/// saat kembali ke tab Timer, supaya tampilan tidak membeku setelah collapse.
fn start_stopwatch_tick(ui: &MainWindow, sw: &Rc<RefCell<timer::Stopwatch>>, sw_tick: &Rc<Timer>) {
    if !sw.borrow().is_running() || !ui.get_expanded() || ui.get_active_tab() != 2 {
        return;
    }
    let tick_weak = ui.as_weak();
    let tick_sw = sw.clone();
    sw_tick.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        if let Some(ui) = tick_weak.upgrade() {
            // gambar hanya saat stopwatch berjalan dan tab Timer sedang terlihat
            if tick_sw.borrow().is_running() && ui.get_expanded() && ui.get_active_tab() == 2 {
                apply_stopwatch(&ui, &tick_sw.borrow());
            }
        }
    });
}

/// Menjalankan ulang tick countdown (250 ms) bila countdown berjalan dan tab Timer
/// sedang tampil. Tick 250 ms menjaga tampilan detik tidak melompat (fasa tick jam
/// 1 detik bergantung pada saat hover dimulai).
fn start_countdown_tick(
    ui: &MainWindow,
    ct: &Rc<RefCell<timer::CountdownTimer>>,
    ct_tick: &Rc<Timer>,
) {
    if !ct.borrow().is_running() || !ui.get_expanded() || ui.get_active_tab() != 2 {
        return;
    }
    let tick_weak = ui.as_weak();
    let tick_ct = ct.clone();
    ct_tick.start(TimerMode::Repeated, Duration::from_millis(250), move || {
        if let Some(ui) = tick_weak.upgrade() {
            // gambar hanya saat countdown berjalan dan tab Timer sedang terlihat
            if tick_ct.borrow().is_running() && ui.get_expanded() && ui.get_active_tab() == 2 {
                tick_ct.borrow_mut().check_finished();
                apply_timer(&ui, &tick_ct.borrow());
            }
        }
    });
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

    let media_ctl = bindings::media::register(&ui);

    // --- hover peek & click expand (model 3-state) ---
    // Saat hover: masuk ke mode Peek (tampilkan jam, jendela tipis 34 px).
    // Saat diklik: masuk ke mode Expanded (dynamic island penuh).
    // Saat mouse keluar: collapse kembali ke garis idle 4 px setelah `SHRINK_DELAY_MS`.
    // status sembunyikan jendela: oleh pengguna (manual) atau oleh aplikasi fullscreen.
    // Dibuat di sini karena alarm juga tidak boleh memunculkan overlay saat tersembunyi.
    let user_hidden = Rc::new(Cell::new(false));
    let fs_suppressed = Rc::new(Cell::new(false));

    // --- timer & stopwatch state (Langkah 15) ---
    let countdown_timer = Rc::new(RefCell::new(timer::CountdownTimer::default()));
    let stopwatch = Rc::new(RefCell::new(timer::Stopwatch::new()));
    let alarm_timer = Rc::new(Timer::default());
    let sw_tick_timer = Rc::new(Timer::default());
    let ct_tick_timer = Rc::new(Timer::default());

    apply_timer(&ui, &countdown_timer.borrow());
    apply_stopwatch(&ui, &stopwatch.borrow());

    let shrink_timer = Rc::new(Timer::default());
    let clock_timer = Rc::new(Timer::default());
    let cpu_tracker = Rc::new(RefCell::new(stats::CpuTracker::new()));
    let gpu_tracker = Rc::new(RefCell::new(gpu::GpuTracker::new()));

    // --- watcher volume master sistem (0% CPU idle, event-driven) ---
    let volume_seq = Arc::new(AtomicU64::new(0));
    let volume_weak = ui.as_weak();
    let volume_seq_watcher = volume_seq.clone();

    let _volume_watcher = match volume::VolumeWatcher::start(move |state, is_initial| {
        let weak = volume_weak.clone();
        let seq = volume_seq_watcher.clone();
        let this_seq = seq.fetch_add(1, Ordering::SeqCst) + 1;

        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.set_volume_level(state.level as i32);
            ui.set_volume_muted(state.muted);

            // Tampilkan popup OSD hanya jika bukan inisialisasi awal dan island tidak expanded
            if !is_initial && !ui.get_expanded() {
                ui.set_show_volume_osd(true);
                place(&ui, VOLUME_OSD_WINDOW_W, VOLUME_OSD_WINDOW_H);

                // Auto-hide popup OSD setelah 2 detik
                let weak_timer = ui.as_weak();
                let seq_check = seq.clone();
                Timer::single_shot(Duration::from_secs(2), move || {
                    if seq_check.load(Ordering::SeqCst) == this_seq {
                        if let Some(ui) = weak_timer.upgrade() {
                            ui.set_show_volume_osd(false);
                            if !ui.get_expanded() && !ui.get_peek() {
                                place(&ui, MIN_WINDOW_W, MIN_WINDOW_H);
                            } else if ui.get_peek() && !ui.get_expanded() {
                                place(&ui, PEEK_WINDOW_W, PEEK_WINDOW_H);
                            }
                        }
                    }
                });
            }
        });
    }) {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!("[volume] Gagal memulai watcher volume audio: {e}");
            None
        }
    };
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
                ui.set_show_volume_osd(false);
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
        let ct_expand = countdown_timer.clone();
        let sw_expand = stopwatch.clone();
        let sw_tick_expand = sw_tick_timer.clone();
        let ct_tick_expand = ct_tick_timer.clone();
        ui.on_expand_clicked(move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_show_volume_osd(false);
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
            bindings::notif::refresh(weak.clone()); // baca notifikasi di thread latar
                                                    // tampilan timer dan stopwatch diperbarui seketika saat dibuka ulang
            if ct_expand.borrow().is_running() {
                ct_expand.borrow_mut().check_finished();
                apply_timer(&ui, &ct_expand.borrow());
            }
            start_stopwatch_tick(&ui, &sw_expand, &sw_tick_expand);
            start_countdown_tick(&ui, &ct_expand, &ct_tick_expand);
        });
    }

    // --- penyesuaian ukuran dinamis dari Slint (Langkah 12) ---
    // Saat tab berpindah atau konten berubah saat expanded, Slint meminta Rust
    // menyesuaikan ukuran jendela secara dinamis.
    {
        let weak = ui.as_weak();
        let sw_resize = stopwatch.clone();
        let sw_tick_resize = sw_tick_timer.clone();
        let ct_resize = countdown_timer.clone();
        let ct_tick_resize = ct_tick_timer.clone();
        ui.on_request_window_resize(move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.get_expanded() {
                let (w, h) = current_window_size(&ui);
                place(&ui, w, h);
                // tick stopwatch/countdown hanya perlu jalan saat tab Timer terlihat
                if ui.get_active_tab() == 2 {
                    start_stopwatch_tick(&ui, &sw_resize, &sw_tick_resize);
                    start_countdown_tick(&ui, &ct_resize, &ct_tick_resize);
                } else {
                    sw_tick_resize.stop();
                    ct_tick_resize.stop();
                }
            }
        });
    }

    // fokus keyboard untuk kolom input Tasks: jendela overlay tidak pernah aktif
    // (WS_EX_NOACTIVATE), jadi pengetikan butuh fokus OS dipindah ke jendela ini
    {
        let weak = ui.as_weak();
        ui.on_tasks_input_focused(move || {
            if let Some(ui) = weak.upgrade() {
                focus_overlay(&ui);
            }
        });
    }

    bindings::calendar::register(&ui);

    // --- timer & stopwatch callbacks (Langkah 15) ---

    // Timer Start
    {
        let weak = ui.as_weak();
        let ct = countdown_timer.clone();
        let at = alarm_timer.clone();
        let ctt = ct_tick_timer.clone();
        let hidden_start = user_hidden.clone();
        let fs_start = fs_suppressed.clone();
        ui.on_timer_start(move || {
            let Some(ui) = weak.upgrade() else { return };
            ct.borrow_mut().start();
            apply_timer(&ui, &ct.borrow());
            start_countdown_tick(&ui, &ct, &ctt);

            // Pasang single-shot alarm saat target waktu tercapai (0% CPU idle, tanpa polling)
            let rem = ct.borrow().remaining();
            let wake_weak = weak.clone();
            let wake_ct = ct.clone();
            let wake_hidden = hidden_start.clone();
            let wake_fs = fs_start.clone();
            at.start(TimerMode::SingleShot, rem, move || {
                if let Some(ui) = wake_weak.upgrade() {
                    wake_ct.borrow_mut().check_finished();
                    apply_timer(&ui, &wake_ct.borrow());
                    // Bangunkan overlay dan tampilkan alarm di tab Timer, tetapi
                    // hormati sembunyikan manual dan fullscreen (tidak saling menimpa).
                    if !wake_hidden.get() && !wake_fs.get() && ui.window().is_visible() {
                        let prev_tab = ui.get_active_tab();
                        ui.set_expanded(true);
                        ui.set_active_tab(2);
                        let (w, h) = current_window_size(&ui);
                        place(&ui, w, h);

                        // Pulihkan tab terakhir beberapa detik kemudian, kecuali
                        // pengguna sudah berpindah tab atau mereset timer.
                        let restore_weak = wake_weak.clone();
                        Timer::single_shot(Duration::from_secs(10), move || {
                            if let Some(ui) = restore_weak.upgrade() {
                                if ui.get_active_tab() == 2 && ui.get_timer_finished() {
                                    ui.set_active_tab(prev_tab);
                                    ui.set_expanded(false);
                                    let (w, h) = current_window_size(&ui);
                                    place(&ui, w, h);
                                }
                            }
                        });
                    }
                }
            });
        });
    }

    // Timer Pause
    {
        let weak = ui.as_weak();
        let ct = countdown_timer.clone();
        let at = alarm_timer.clone();
        let ctt = ct_tick_timer.clone();
        ui.on_timer_pause(move || {
            let Some(ui) = weak.upgrade() else { return };
            ct.borrow_mut().pause();
            at.stop(); // hentikan alarm single-shot saat dijeda
            ctt.stop(); // tampilan tidak berubah saat dijeda
            apply_timer(&ui, &ct.borrow());
        });
    }

    // Timer Reset
    {
        let weak = ui.as_weak();
        let ct = countdown_timer.clone();
        let at = alarm_timer.clone();
        let ctt = ct_tick_timer.clone();
        ui.on_timer_reset(move || {
            let Some(ui) = weak.upgrade() else { return };
            ct.borrow_mut().reset();
            at.stop();
            ctt.stop();
            apply_timer(&ui, &ct.borrow());
        });
    }

    // Timer Set Duration (Preset menit)
    {
        let weak = ui.as_weak();
        let ct = countdown_timer.clone();
        let at = alarm_timer.clone();
        let ctt = ct_tick_timer.clone();
        ui.on_timer_set_min(move |mins| {
            let Some(ui) = weak.upgrade() else { return };
            at.stop();
            ctt.stop();
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
        let ctt = ct_tick_timer.clone();
        ui.on_timer_adjust_min(move |delta| {
            let Some(ui) = weak.upgrade() else { return };
            at.stop();
            ctt.stop();
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
            start_stopwatch_tick(&ui, &sw, &sw_tick);
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

    bindings::tasks::register(&ui);

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

    bindings::notif::register_privacy(&ui, &tray);

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
    use crate::core::window::{clamp_window_size, MAX_EXPANDED_H, MAX_EXPANDED_W};

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
