//! Modul pemantauan volume audio master Windows (Volume OSD).
//!
//! Menggunakan Windows Core Audio `IAudioEndpointVolume` dan `IAudioEndpointVolumeCallback`.
//! Seluruh pembaruan bersifat event-driven tanpa polling (0% CPU idle) dan dijalankan di
//! background worker thread terisolasi agar tidak mengganggu COM apartment mode (STA) pada
//! thread UI winit (`RPC_E_CHANGED_MODE`).
//! Ketika pengguna mengubah volume via tombol keyboard, slider taskbar, atau tombol headset,
//! Windows memicu `OnNotify` secara seketika.

use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};

/// Informasi status volume master perangkat audio default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeState {
    /// Tingkat volume master (0-100%).
    pub level: u8,
    /// True jika audio dalam keadaan mute / bisu.
    pub muted: bool,
}

/// Membaca volume master audio sistem terkini di thread terisolasi.
/// Menjalankan COM di thread terpisah agar tidak mengganggu apartment mode (STA/MTA) pada thread pemanggil.
#[allow(dead_code)]
pub fn get_master_volume() -> windows::core::Result<VolumeState> {
    std::thread::spawn(|| unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;

        let level_scalar = endpoint.GetMasterVolumeLevelScalar()?;
        let muted_bool = endpoint.GetMute()?;

        let level = (level_scalar * 100.0).round().clamp(0.0, 100.0) as u8;
        let muted = muted_bool.as_bool();

        CoUninitialize();
        Ok(VolumeState { level, muted })
    })
    .join()
    .map_err(|_| windows::core::Error::from_hresult(E_FAIL))?
}

type VolumeHandler = Box<dyn Fn(VolumeState, bool) + Send + Sync + 'static>;

/// Callback COM untuk menerima event perubahan volume dari Windows.
#[windows::core::implement(IAudioEndpointVolumeCallback)]
struct AudioVolumeCallback {
    handler: Arc<VolumeHandler>,
}

impl IAudioEndpointVolumeCallback_Impl for AudioVolumeCallback_Impl {
    fn OnNotify(&self, pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        if !pnotify.is_null() {
            unsafe {
                let data = &*pnotify;
                let level = (data.fMasterVolume * 100.0).round().clamp(0.0, 100.0) as u8;
                let muted = data.bMuted.as_bool();
                (self.handler)(VolumeState { level, muted }, false);
            }
        }
        Ok(())
    }
}

/// Handle watcher volume untuk menjaga background worker thread dan COM callback tetap aktif.
pub struct VolumeWatcher {
    stop_tx: Option<Sender<()>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl VolumeWatcher {
    /// Memulai pemantauan event volume master Windows di background thread terisolasi.
    ///
    /// `handler` menerima `(VolumeState, is_initial: bool)`.
    /// `is_initial = true` dikirim sekali saat startup untuk inisialisasi state UI tanpa popup OSD.
    /// `is_initial = false` dikirim setiap kali volume sistem berubah via tombol keyboard / slider.
    pub fn start<F>(handler: F) -> windows::core::Result<Self>
    where
        F: Fn(VolumeState, bool) + Send + Sync + 'static,
    {
        let (stop_tx, stop_rx) = channel::<()>();
        let (init_tx, init_rx) = channel::<windows::core::Result<()>>();

        let handler = Arc::new(Box::new(handler) as VolumeHandler);

        let thread_handle = std::thread::spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let res: windows::core::Result<(
                IMMDeviceEnumerator,
                IAudioEndpointVolume,
                IAudioEndpointVolumeCallback,
            )> = (|| {
                let enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
                let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;

                // Baca status volume awal dan kirimkan ke handler sebagai is_initial = true
                if let (Ok(level_scalar), Ok(muted_bool)) =
                    (endpoint.GetMasterVolumeLevelScalar(), endpoint.GetMute())
                {
                    let level = (level_scalar * 100.0).round().clamp(0.0, 100.0) as u8;
                    let muted = muted_bool.as_bool();
                    handler(VolumeState { level, muted }, true);
                }

                let callback_impl = AudioVolumeCallback {
                    handler: handler.clone(),
                };
                let callback: IAudioEndpointVolumeCallback = callback_impl.into();
                endpoint.RegisterControlChangeNotify(&callback)?;

                Ok((enumerator, endpoint, callback))
            })();

            match res {
                Ok((_enumerator, endpoint, callback)) => {
                    let _ = init_tx.send(Ok(()));
                    // Tunggu sinyal stop saat VolumeWatcher di-drop (0% CPU idle)
                    let _ = stop_rx.recv();
                    let _ = endpoint.UnregisterControlChangeNotify(&callback);
                }
                Err(e) => {
                    let _ = init_tx.send(Err(e));
                }
            }

            CoUninitialize();
        });

        // Tunggu inisialisasi worker thread selesai
        match init_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stop_tx: Some(stop_tx),
                thread_handle: Some(thread_handle),
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(windows::core::Error::from_hresult(E_FAIL)),
        }
    }
}

impl Drop for VolumeWatcher {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(th) = self.thread_handle.take() {
            let _ = th.join();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_get_master_volume_returns_valid_bounds() {
        if let Ok(state) = get_master_volume() {
            assert!(state.level <= 100);
            println!(
                "Test master volume: {}% (muted: {})",
                state.level, state.muted
            );
        }
    }

    #[test]
    fn test_volume_watcher_creation_and_teardown() {
        let watcher = VolumeWatcher::start(|_state, _is_initial| {});
        if let Ok(watcher) = watcher {
            drop(watcher);
        }
    }
}
