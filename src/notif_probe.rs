//! Probe sementara (hanya build debug), langkah 10b: apakah `UserNotificationListener`
//! bisa membaca isi notifikasi dan menembakkan `NotificationChanged` pada aplikasi
//! unpackaged.
//!
//! Hasil 10a (21 Okt 2026): `RequestAccessAsync` mengembalikan status 1 (`Allowed`).
//! Status itu belum membuktikan isi notifikasi bisa dibaca maupun event-nya menembak.
//!
//! Probe ini mencetak isi notifikasi ke konsol, bukan ke overlay.

use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;

use windows::core::Result;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{
    KnownNotificationBindings, NotificationKinds, UserNotification,
    UserNotificationChangedEventArgs, UserNotificationChangedKind,
};

/// Jeda polling pembanding saat menunggu event, dalam detik. Polling ini hanya alat
/// diagnosis: bila jumlah notifikasi berubah tanpa ada event, berarti event tidak menembak.
const POLL_SECS: u64 = 10;

/// Membaca teks (judul dan isi) dari satu notifikasi toast lewat binding `ToastGeneric`.
fn read_texts(n: &UserNotification) -> Result<Vec<String>> {
    let binding = n
        .Notification()?
        .Visual()?
        .GetBinding(&KnownNotificationBindings::ToastGeneric()?)?;
    let elements = binding.GetTextElements()?;
    let mut out = Vec::new();
    for i in 0..elements.Size()? {
        out.push(elements.GetAt(i)?.Text()?.to_string());
    }
    Ok(out)
}

/// Jumlah notifikasi toast yang sedang aktif.
fn count(listener: &UserNotificationListener) -> Result<u32> {
    let list = listener
        .GetNotificationsAsync(NotificationKinds::Toast)?
        .join()?;
    list.Size()
}

/// Mencetak semua notifikasi toast aktif: id, nama aplikasi, dan teks.
/// Bagian yang gagal dibaca dicetak sebagai `Err(...)` supaya terlihat apa yang gagal.
fn dump(listener: &UserNotificationListener, label: &str) -> Result<u32> {
    let list = listener
        .GetNotificationsAsync(NotificationKinds::Toast)?
        .join()?;
    let total = list.Size()?;
    eprintln!("[notif] {label}: {total} notification(s)");
    for i in 0..total {
        let n = list.GetAt(i)?;
        let id = n.Id().unwrap_or(0);
        let app = n
            .AppInfo()
            .and_then(|a| a.DisplayInfo())
            .and_then(|d| d.DisplayName())
            .map(|s| s.to_string());
        eprintln!("[notif]   #{id} app={app:?} texts={:?}", read_texts(&n));
    }
    Ok(total)
}

/// Meminta akses, membaca daftar awal, lalu menunggu event `NotificationChanged`.
/// Memblokir thread pemanggil selamanya, jadi jalankan dari thread latar.
pub fn probe() -> Result<()> {
    let listener = UserNotificationListener::Current()?;
    let status = listener.RequestAccessAsync()?.join()?;
    eprintln!("[notif] access status: {status:?}");
    if status != UserNotificationListenerAccessStatus::Allowed {
        return Ok(());
    }

    let mut last_count = dump(&listener, "initial")?;

    // handler hanya mengirim sinyal; pembacaan dilakukan di thread ini (pola yang sama dengan media.rs)
    let (tx, rx) = channel::<(bool, u32)>();
    let token = listener.NotificationChanged(&TypedEventHandler::<
        UserNotificationListener,
        UserNotificationChangedEventArgs,
    >::new(move |_, args| {
        if let Some(args) = args.as_ref() {
            let added = args.ChangeKind()? == UserNotificationChangedKind::Added;
            let _ = tx.send((added, args.UserNotificationId()?));
        }
        Ok(())
    }))?;
    eprintln!("[notif] NotificationChanged registered (token {token}), waiting for events");

    loop {
        match rx.recv_timeout(Duration::from_secs(POLL_SECS)) {
            Ok((added, id)) => {
                let kind = if added { "added" } else { "removed" };
                eprintln!("[notif] EVENT: {kind} id={id}");
                last_count = dump(&listener, "after event")?;
            }
            Err(RecvTimeoutError::Timeout) => {
                // pembanding: jumlah berubah tanpa event berarti event tidak menembak
                let now = count(&listener)?;
                if now != last_count {
                    eprintln!("[notif] NO EVENT, but count changed {last_count} -> {now}");
                    last_count = dump(&listener, "poll")?;
                }
            }
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}
