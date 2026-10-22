//! Membaca notifikasi toast yang sedang aktif lewat `UserNotificationListener`.
//!
//! Hanya membaca daftar (tanpa langganan event): registrasi `NotificationChanged`
//! gagal pada aplikasi unpackaged (`0x80070490`, lihat rencana, langkah 10b).
//! Panggilan `fetch` memblokir, jadi JANGAN dipanggil dari thread UI.
//!
//! Logika pembersihan dan pengelompokan ada di `notif_text.rs` (bisa diuji).
//!
//! Privasi: bila `include_body` salah, isi pesan tidak dibaca sama sekali
//! (hanya elemen teks pertama, yaitu judul).

use windows::core::Result;
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};

use super::notif_text::{group, RawNotif, Snapshot};

/// Jumlah kelompok maksimum yang dikembalikan ke UI.
const MAX_ITEMS: usize = 5;

/// Hasil `fetch`.
pub enum Fetch {
    /// Daftar berhasil dibaca (bisa kosong).
    Ready(Snapshot),
    /// Akses notifikasi ditolak atau dicabut di Pengaturan Windows. Dalam keadaan
    /// ini Windows mengembalikan daftar kosong tanpa error, jadi dicek terpisah.
    NoAccess,
}

/// Membaca paling banyak `limit` elemen teks dari satu notifikasi toast
/// (binding `ToastGeneric`). Elemen pertama adalah judul, sisanya isi.
fn read_texts(n: &UserNotification, limit: u32) -> Result<Vec<String>> {
    let binding = n
        .Notification()?
        .Visual()?
        .GetBinding(&KnownNotificationBindings::ToastGeneric()?)?;
    let elements = binding.GetTextElements()?;
    let mut out = Vec::new();
    for i in 0..elements.Size()?.min(limit) {
        out.push(elements.GetAt(i)?.Text()?.to_string());
    }
    Ok(out)
}

/// Mengubah satu `UserNotification` menjadi data mentah. Bagian yang gagal dibaca
/// menjadi kosong; notifikasi tetap dihitung.
fn to_raw(n: &UserNotification, include_body: bool) -> RawNotif {
    let app = n
        .AppInfo()
        .and_then(|a| a.DisplayInfo())
        .and_then(|d| d.DisplayName())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let limit = if include_body { u32::MAX } else { 1 };
    let mut texts = read_texts(n, limit).unwrap_or_default().into_iter();
    let title = texts.next().unwrap_or_default();
    let body = texts.collect::<Vec<_>>().join(" ");
    let time = n.CreationTime().map(|t| t.UniversalTime).unwrap_or(0);
    RawNotif {
        app,
        title,
        body,
        time,
    }
}

/// Membaca daftar notifikasi toast aktif: terbaru dulu, dibersihkan, dan notifikasi
/// yang tampil sama digabung (lihat `notif_text::group`).
pub fn fetch(include_body: bool) -> Result<Fetch> {
    let listener = UserNotificationListener::Current()?;

    // status dicek setiap kali: pengguna bisa mencabut akses kapan saja
    let mut status = listener.GetAccessStatus()?;
    if status == UserNotificationListenerAccessStatus::Unspecified {
        status = listener.RequestAccessAsync()?.join()?;
    }
    if status != UserNotificationListenerAccessStatus::Allowed {
        return Ok(Fetch::NoAccess);
    }

    let list = listener
        .GetNotificationsAsync(NotificationKinds::Toast)?
        .join()?;
    let mut raw = Vec::new();
    for i in 0..list.Size()? {
        if let Ok(n) = list.GetAt(i) {
            raw.push(to_raw(&n, include_body));
        }
    }
    Ok(Fetch::Ready(group(raw, include_body, MAX_ITEMS)))
}

/// Uji sementara untuk langkah 10c (hanya build debug): membaca tiga kali dan
/// mencetak ringkasan serta lama tiap pembacaan, supaya biaya pembacaan terlihat.
/// Isi pesan hanya dibaca bila sakelar privasi aktif (`config::show_body`, dari menu tray).
#[cfg(debug_assertions)]
pub fn debug_run() {
    use super::notif_text::{age_secs, ticks_from_unix_secs};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    let include_body = crate::core::config::show_body();
    eprintln!("[notif] include_body={include_body}");

    for round in 1..=3 {
        let started = Instant::now();
        let result = fetch(include_body);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        match result {
            Ok(Fetch::Ready(s)) => {
                eprintln!(
                    "[notif] read #{round}: {} raw, {} groups, showing {} ({ms:.1} ms)",
                    s.total,
                    s.groups,
                    s.items.len()
                );
                let secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let now = ticks_from_unix_secs(secs);
                for it in &s.items {
                    let age = age_secs(it.time, now);
                    if include_body {
                        eprintln!(
                            "[notif]   {} | {} | {} | x{} | {age}s ago",
                            it.app, it.title, it.body, it.count
                        );
                    } else {
                        eprintln!(
                            "[notif]   {} | {} | x{} | {age}s ago",
                            it.app, it.title, it.count
                        );
                    }
                }
            }
            Ok(Fetch::NoAccess) => {
                eprintln!("[notif] read #{round}: access not granted ({ms:.1} ms)")
            }
            Err(e) => eprintln!("[notif] read #{round} FAILED: {e}"),
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
