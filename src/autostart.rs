//! Pengelolaan autostart lewat kunci registry HKCU\...\Run (tanpa hak admin).

use std::io;

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "winisland";

/// Perintah autostart: path exe saat ini dalam tanda kutip (aman untuk path berspasi).
fn current_command() -> io::Result<String> {
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\"", exe.display()))
}

fn open_run_key() -> io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)
}

/// True jika entri ada dan menunjuk ke exe yang sedang berjalan.
pub fn is_enabled() -> bool {
    let Ok(key) = open_run_key() else {
        eprintln!("[autostart] gagal membuka kunci Run");
        return false;
    };
    let value = match key.get_value::<String, _>(VALUE_NAME) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[autostart] nilai registry tidak terbaca: {e}");
            return false;
        }
    };
    let Ok(command) = current_command() else {
        eprintln!("[autostart] gagal membaca path exe");
        return false;
    };
    let cocok = value.eq_ignore_ascii_case(&command);
    eprintln!("[autostart] registry={value} | exe={command} | cocok={cocok}");
    cocok
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