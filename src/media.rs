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
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs,
};

/// Jeda penggabungan event beruntun, dalam milidetik. SMTC sering mengirim beberapa
/// event untuk satu pergantian lagu (kadang dengan judul kosong dulu).
const DEBOUNCE_MS: u64 = 150;

/// Judul, artis, dan status pemutaran dari sesi media yang sedang aktif.
#[derive(Clone, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    /// `true` bila sedang diputar, `false` bila dijeda, berhenti, atau statusnya tidak terbaca.
    pub playing: bool,
}

/// Membaca judul, artis, dan status pemutaran dari satu sesi. Memblokir (`join()`),
/// jadi hanya boleh dipanggil dari worker thread, bukan thread UI.
fn read_session(session: &Session) -> Result<NowPlaying> {
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    // status yang gagal dibaca dianggap tidak sedang diputar
    let playing = session
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .map(|status| status == PlaybackStatus::Playing)
        .unwrap_or(false);
    Ok(NowPlaying {
        title: props.Title()?.to_string(),
        artist: props.Artist()?.to_string(),
        playing,
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
/// `false` = properti media atau status pemutaran berubah.
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

    // sesi yang sedang dipantau beserta token handler-nya (untuk dilepas saat sesi berganti):
    // (sesi, token MediaPropertiesChanged, token PlaybackInfoChanged)
    let mut attached: Option<(Session, i64, i64)> = None;
    // hasil terakhir yang dikirim ke UI, untuk menghindari kiriman duplikat
    let mut last: Option<Option<NowPlaying>> = None;
    let mut reattach = true;

    loop {
        if reattach {
            if let Some((old, props_token, playback_token)) = attached.take() {
                // error diabaikan: sesi lama bisa saja sudah tidak ada
                let _ = old.RemoveMediaPropertiesChanged(props_token);
                let _ = old.RemovePlaybackInfoChanged(playback_token);
            }
            // GetCurrentSession mengembalikan Err bila tidak ada sesi aktif
            if let Ok(session) = manager.GetCurrentSession() {
                let tx_props = tx.clone();
                let props_handler =
                    TypedEventHandler::<Session, MediaPropertiesChangedEventArgs>::new(
                        move |_, _| {
                            let _ = tx_props.send(false);
                            Ok(())
                        },
                    );
                let tx_playback = tx.clone();
                let playback_handler =
                    TypedEventHandler::<Session, PlaybackInfoChangedEventArgs>::new(move |_, _| {
                        let _ = tx_playback.send(false);
                        Ok(())
                    });
                match (
                    session.MediaPropertiesChanged(&props_handler),
                    session.PlaybackInfoChanged(&playback_handler),
                ) {
                    (Ok(props_token), Ok(playback_token)) => {
                        attached = Some((session, props_token, playback_token));
                    }
                    (props, playback) => {
                        // bila salah satu gagal, lepas yang berhasil supaya tidak bocor
                        if let Ok(token) = props {
                            let _ = session.RemoveMediaPropertiesChanged(token);
                        }
                        if let Ok(token) = playback {
                            let _ = session.RemovePlaybackInfoChanged(token);
                        }
                        eprintln!("[media] attach FAILED");
                    }
                }
            }
        }

        let now = match &attached {
            Some((session, _, _)) => match read_session(session) {
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
