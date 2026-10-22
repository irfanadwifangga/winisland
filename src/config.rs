//! Preferensi pengguna yang disimpan di registry `HKCU\Software\winisland`
//! (tanpa hak admin dan tanpa berkas konfigurasi terpisah, seperti autostart).
//!
//! Saat ini hanya satu preferensi: apakah isi (body) notifikasi boleh dibaca dan
//! ditampilkan. Bawaannya TERSEMBUNYI; setiap kegagalan membaca juga berarti tersembunyi.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

/// Kunci registry per pengguna milik aplikasi ini.
const APP_KEY: &str = r"Software\winisland";
/// Nilai DWORD: 1 = isi notifikasi ditampilkan, selain itu (atau tidak ada) = disembunyikan.
const SHOW_BODY_VALUE: &str = "ShowNotificationBody";

/// Keadaan saat ini di memori. Bernilai `false` (tersembunyi) sampai `load()` dipanggil.
static SHOW_BODY: AtomicBool = AtomicBool::new(false);

/// Membaca preferensi dari registry ke memori. Panggil sekali saat startup.
/// Nilai belum ada (pemakaian pertama) tidak dicatat sebagai error.
pub fn load() {
    let stored = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(APP_KEY, KEY_READ)
        .and_then(|key| key.get_value::<u32, _>(SHOW_BODY_VALUE));
    let show = match stored {
        Ok(value) => value == 1,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => {
            eprintln!("[config] failed to read the notification body setting: {e}");
            false
        }
    };
    SHOW_BODY.store(show, Ordering::Relaxed);
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
