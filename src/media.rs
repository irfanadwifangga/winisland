//! Membaca info media yang sedang diputar lewat SMTC (Windows.Media.Control).
//!
//! Pembaruan berbasis event: handler WinRT hanya mengirim sinyal ke satu worker thread,
//! dan worker itulah yang membaca datanya. Dengan begitu pembacaan selalu berurutan
//! (tidak ada race antar-event) dan event beruntun bisa digabung (debounce).

use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use windows::core::Result;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    MediaPropertiesChangedEventArgs,
};

/// Jeda penggabungan event beruntun, dalam milidetik. SMTC sering mengirim beberapa
/// event untuk satu pergantian lagu (kadang dengan judul kosong dulu).
const DEBOUNCE_MS: u64 = 150;

/// Judul dan artis dari sesi media yang sedang aktif.
#[derive(Clone, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
}

/// Membaca judul dan artis dari satu sesi. Memblokir (`join()`), jadi hanya
/// boleh dipanggil dari worker thread, bukan thread UI.
fn read_session(session: &Session) -> Result<NowPlaying> {
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    Ok(NowPlaying {
        title: props.Title()?.to_string(),
        artist: props.Artist()?.to_string(),
    })
}

/// Menjalankan worker yang memantau perubahan media. `on_change` dipanggil dari
/// worker thread setiap kali hasil baca berbeda dari yang terakhir dikirim
/// (`None` berarti tidak ada sesi aktif).
///
/// Mengembalikan `Sender` untuk meminta worker membaca ulang: kirim `true` untuk
/// membaca ulang sekaligus memasang ulang listener (jaring pengaman bila event terlewat).
pub fn spawn_watcher(on_change: impl FnMut(Option<NowPlaying>) + Send + 'static) -> Sender<bool> {
    let (tx, rx) = channel::<bool>();
    let tx_out = tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = run(tx, rx, on_change) {
            eprintln!("[media] watcher FAILED: {e}");
        }
    });
    tx_out
}

/// Isi worker. Sinyal bernilai `true` = sesi aktif berganti (pasang ulang listener),
/// `false` = properti media berubah.
fn run(
    tx: Sender<bool>,
    rx: Receiver<bool>,
    mut on_change: impl FnMut(Option<NowPlaying>),
) -> Result<()> {
    // `manager` harus tetap hidup selama loop; bila di-drop, handler ikut berhenti
    let manager = SessionManager::RequestAsync()?.join()?;

    let tx_session = tx.clone();
    manager.CurrentSessionChanged(&TypedEventHandler::<
        SessionManager,
        CurrentSessionChangedEventArgs,
    >::new(move |_, _| {
        let _ = tx_session.send(true);
        Ok(())
    }))?;

    // sesi yang sedang dipantau beserta token handler-nya (untuk dilepas saat sesi berganti)
    let mut attached: Option<(Session, i64)> = None;
    // hasil terakhir yang dikirim ke UI, untuk menghindari kiriman duplikat
    let mut last: Option<Option<NowPlaying>> = None;
    let mut reattach = true;

    loop {
        if reattach {
            if let Some((old, token)) = attached.take() {
                // error diabaikan: sesi lama bisa saja sudah tidak ada
                let _ = old.RemoveMediaPropertiesChanged(token);
            }
            // GetCurrentSession mengembalikan Err bila tidak ada sesi aktif
            if let Ok(session) = manager.GetCurrentSession() {
                let tx_props = tx.clone();
                let handler = TypedEventHandler::<Session, MediaPropertiesChangedEventArgs>::new(
                    move |_, _| {
                        let _ = tx_props.send(false);
                        Ok(())
                    },
                );
                match session.MediaPropertiesChanged(&handler) {
                    Ok(token) => attached = Some((session, token)),
                    Err(e) => eprintln!("[media] attach FAILED: {e}"),
                }
            }
        }

        let now = match &attached {
            Some((session, _)) => match read_session(session) {
                Ok(np) => Some(np),
                Err(e) => {
                    eprintln!("[media] read FAILED: {e}");
                    None
                }
            },
            None => None,
        };
        if last.as_ref() != Some(&now) {
            on_change(now.clone());
            last = Some(now);
        }

        // tunggu sinyal berikutnya (blocking, tanpa CPU), lalu gabungkan sinyal susulan
        let Ok(first) = rx.recv() else { return Ok(()) };
        let mut signal = first;
        while let Ok(more) = rx.recv_timeout(Duration::from_millis(DEBOUNCE_MS)) {
            signal |= more;
        }
        reattach = signal;
    }
}
