//! Pengelolaan autostart lewat kunci registry HKCU\...\Run (tanpa hak admin).

use std::io;

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

/// Kunci registry per pengguna tempat Windows membaca program yang dijalankan saat login.
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Nama nilai registry milik aplikasi ini.
const VALUE_NAME: &str = "winisland";

/// Perintah autostart: path exe saat ini dalam tanda kutip (aman untuk path berspasi).
fn current_command() -> io::Result<String> {
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\"", exe.display()))
}

/// Membuka kunci `Run` milik pengguna saat ini dengan izin baca dan tulis.
fn open_run_key() -> io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)
}

/// True jika entri ada dan menunjuk ke exe yang sedang berjalan.
pub fn is_enabled() -> bool {
    let Ok(key) = open_run_key() else {
        eprintln!("[autostart] failed to open the Run key");
        return false;
    };
    let value = match key.get_value::<String, _>(VALUE_NAME) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[autostart] registry value could not be read: {e}");
            return false;
        }
    };
    let Ok(command) = current_command() else {
        eprintln!("[autostart] failed to read the exe path");
        return false;
    };
    let matches = value.eq_ignore_ascii_case(&command);
    eprintln!("[autostart] registry={value} | exe={command} | matches={matches}");
    matches
}

/// Menulis atau menghapus entri. Menghapus entri yang memang tidak ada dianggap berhasil.
pub fn set_enabled(enabled: bool) -> io::Result<()> {
    let key = open_run_key()?;
    if enabled {
        key.set_value(VALUE_NAME, &current_command()?)
    } else {
        match key.delete_value(VALUE_NAME) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}
