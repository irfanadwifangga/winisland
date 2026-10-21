//! Membaca info media yang sedang diputar lewat SMTC (Windows.Media.Control).

use windows::core::Result;
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as SessionManager;

pub struct NowPlaying {
    pub title: String,
    pub artist: String,
}

/// Memanggil API async secara blocking. JANGAN panggil dari thread UI.
/// Mengembalikan Ok(None) bila tidak ada sesi media aktif.
pub fn read_now_playing() -> Result<Option<NowPlaying>> {
    let manager = SessionManager::RequestAsync()?.join()?;

    let session = match manager.GetCurrentSession() {
        Ok(s) => s,
        Err(e) => {
            // belum pasti apakah "tidak ada sesi" muncul sebagai Err atau nilai kosong
            eprintln!("[media] tidak ada sesi aktif: {e}");
            return Ok(None);
        }
    };

    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    Ok(Some(NowPlaying {
        title: props.Title()?.to_string(),
        artist: props.Artist()?.to_string(),
    }))
}