//! Preferensi pengguna yang disimpan di registry `HKCU\Software\winisland`
//! (tanpa hak admin dan tanpa berkas konfigurasi terpisah, seperti autostart).
//!
//! - `ShowNotificationBody`: apakah isi notifikasi boleh dibaca dan ditampilkan (bawaan: false).
//! - `MonitorIndex`: indeks monitor tempat overlay ditampilkan (0 = Primary, 1 = Display 2, dst).

use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

/// Kunci registry per pengguna milik aplikasi ini.
const APP_KEY: &str = r"Software\winisland";
/// Nilai DWORD: 1 = isi notifikasi ditampilkan, selain itu (atau tidak ada) = disembunyikan.
const SHOW_BODY_VALUE: &str = "ShowNotificationBody";
/// Nilai DWORD: indeks monitor pilihan (0 = Primary, 1 = Display 2, dst).
const MONITOR_INDEX_VALUE: &str = "MonitorIndex";

/// Keadaan saat ini di memori. Bernilai `false` (tersembunyi) sampai `load()` dipanggil.
static SHOW_BODY: AtomicBool = AtomicBool::new(false);
/// Indeks monitor saat ini di memori (0 = Primary).
static MONITOR_INDEX: AtomicUsize = AtomicUsize::new(0);

/// Membaca preferensi dari registry ke memori. Panggil sekali saat startup.
/// Nilai belum ada (pemakaian pertama) tidak dicatat sebagai error.
pub fn load() {
    let show = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(APP_KEY, KEY_READ)
        .and_then(|k| k.get_value::<u32, _>(SHOW_BODY_VALUE))
        .map(|v| v == 1)
        .unwrap_or(false);
    SHOW_BODY.store(show, Ordering::Relaxed);

    let monitor_idx = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(APP_KEY, KEY_READ)
        .and_then(|k| k.get_value::<u32, _>(MONITOR_INDEX_VALUE))
        .map(|v| v as usize)
        .unwrap_or(0);
    MONITOR_INDEX.store(monitor_idx, Ordering::Relaxed);
}

/// Apakah isi notifikasi boleh dibaca dan ditampilkan. Murah dan aman dipanggil dari thread mana pun.
pub fn show_body() -> bool {
    SHOW_BODY.load(Ordering::Relaxed)
}

/// Mengubah preferensi: memori diubah dulu (berlaku seketika), lalu disimpan ke registry.
/// Bila penyimpanan gagal, perubahan tetap berlaku sampai aplikasi ditutup.
pub fn set_show_body(show: bool) -> io::Result<()> {
    SHOW_BODY.store(show, Ordering::Relaxed);
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(APP_KEY)?;
    key.set_value(SHOW_BODY_VALUE, &u32::from(show))
}

/// Indeks monitor yang sedang dipilih (0 = Primary).
pub fn monitor_index() -> usize {
    MONITOR_INDEX.load(Ordering::Relaxed)
}

/// Mengubah indeks monitor yang dipilih dan menyimpannya ke registry.
pub fn set_monitor_index(index: usize) -> io::Result<()> {
    MONITOR_INDEX.store(index, Ordering::Relaxed);
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(APP_KEY)?;
    key.set_value(MONITOR_INDEX_VALUE, &(index as u32))
}
