//! Penghubung pembacaan notifikasi dan sakelar privasi dengan UI Slint.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::core::config;
use crate::plugins::notif::{self, Fetch};
use crate::plugins::notif_text::{age_secs, format_age, ticks_from_unix_secs, Snapshot};
use crate::{AppTray, MainWindow, NotifRow};

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
pub(crate) fn refresh(weak: slint::Weak<MainWindow>) {
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

/// Memasang sakelar privasi di tray; daftar lama dibuang sebelum pembacaan ulang.
pub(crate) fn register_privacy(ui: &MainWindow, tray: &AppTray) {
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
                    refresh(ui_weak.clone());
                }
            }
        });
    }
}
