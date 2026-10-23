//! Penghubung worker SMTC dan tombol kontrol media dengan UI Slint.

use slint::ComponentHandle;

use crate::plugins::media::{self, MediaHandle};
use crate::MainWindow;

/// Memasang worker dan callback; handle tetap disimpan pemanggil untuk refresh.
pub(crate) fn register(ui: &MainWindow) -> MediaHandle {
    // --- media: langganan event SMTC lewat worker thread
    // `media_ctl` meminta worker membaca ulang (saat hover, sebagai jaring pengaman)
    // dan mengirim perintah tombol.
    let media_ctl = {
        let weak = ui.as_weak();
        media::spawn_watcher(move |info| {
            let (title, artist, playing, has_media) = match info {
                Some(m) => (m.title, m.artist, m.playing, true),
                None => ("Nothing playing".to_string(), String::new(), false, false),
            };
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_track_title(title.into());
                ui.set_track_artist(artist.into());
                ui.set_playing(playing);
                ui.set_has_media(has_media);
            });
        })
    };

    // --- tombol kontrol media: perintah dikirim ke worker, tidak memblokir UI
    {
        let ctl = media_ctl.clone();
        ui.on_play_pause(move || ctl.toggle_play_pause());
        let ctl = media_ctl.clone();
        ui.on_skip_next(move || ctl.skip_next());
        let ctl = media_ctl.clone();
        ui.on_skip_previous(move || ctl.skip_previous());
    }

    media_ctl
}
