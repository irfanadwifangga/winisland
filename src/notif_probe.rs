//! Probe sementara (hanya build debug): apakah aplikasi unpackaged boleh memakai
//! `UserNotificationListener`.
//!
//! Hasil (21 Okt 2026): `RequestAccessAsync` mengembalikan status 1 (`Allowed`) pada aplikasi
//! unpackaged. Status ini belum membuktikan bahwa isi notifikasi bisa dibaca.

use windows::core::Result;
use windows::UI::Notifications::Management::UserNotificationListener;

/// Meminta akses ke notifikasi pengguna dan mencetak statusnya ke konsol.
/// Memblokir thread pemanggil (`join()`), jadi jalankan dari thread latar.
pub fn probe() -> Result<()> {
    let listener = UserNotificationListener::Current()?;
    let status = listener.RequestAccessAsync()?.join()?;
    eprintln!("[notif] access status: {status:?}");
    Ok(())
}
