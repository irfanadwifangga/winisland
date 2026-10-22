//! Modul pemantauan volume audio master Windows (Volume OSD).
//!
//! Menggunakan Windows Core Audio `IAudioEndpointVolume` dan `IAudioEndpointVolumeCallback`.
//! Seluruh pembaruan bersifat event-driven tanpa polling (0% CPU idle).
//! Ketika pengguna mengubah volume via tombol keyboard, slider taskbar, atau tombol headset,
//! Windows memicu `OnNotify` secara seketika.

use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};

/// Informasi status volume master perangkat audio default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeState {
    /// Tingkat volume master (0-100%).
    pub level: u8,
    /// True jika audio dalam keadaan mute / bisu.
    pub muted: bool,
}

/// Membaca volume master audio sistem terkini.
pub fn get_master_volume() -> windows::core::Result<VolumeState> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;

        let level_scalar = endpoint.GetMasterVolumeLevelScalar()?;
        let muted_bool = endpoint.GetMute()?;

        let level = (level_scalar * 100.0).round().clamp(0.0, 100.0) as u8;
        let muted = muted_bool.as_bool();

        Ok(VolumeState { level, muted })
    }
}

type VolumeHandler = Box<dyn Fn(VolumeState) + Send + Sync + 'static>;

/// Callback COM untuk menerima event perubahan volume dari Windows.
#[windows::core::implement(IAudioEndpointVolumeCallback)]
struct AudioVolumeCallback {
    handler: VolumeHandler,
}

impl IAudioEndpointVolumeCallback_Impl for AudioVolumeCallback_Impl {
    fn OnNotify(&self, pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        if !pnotify.is_null() {
            unsafe {
                let data = &*pnotify;
                let level = (data.fMasterVolume * 100.0).round().clamp(0.0, 100.0) as u8;
                let muted = data.bMuted.as_bool();
                (self.handler)(VolumeState { level, muted });
            }
        }
        Ok(())
    }
}

/// Handle watcher volume untuk menjaga COM callback tetap aktif.
pub struct VolumeWatcher {
    endpoint: Option<IAudioEndpointVolume>,
    callback: Option<IAudioEndpointVolumeCallback>,
}

impl VolumeWatcher {
    /// Memulai pemantauan event volume master Windows.
    ///
    /// Memanggil `handler` setiap kali volume berubah (level atau mute).
    pub fn start<F>(handler: F) -> windows::core::Result<Self>
    where
        F: Fn(VolumeState) + Send + Sync + 'static,
    {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
            let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;

            let callback_impl = AudioVolumeCallback {
                handler: Box::new(handler),
            };
            let callback: IAudioEndpointVolumeCallback = callback_impl.into();

            endpoint.RegisterControlChangeNotify(&callback)?;

            Ok(Self {
                endpoint: Some(endpoint),
                callback: Some(callback),
            })
        }
    }
}

impl Drop for VolumeWatcher {
    fn drop(&mut self) {
        if let (Some(endpoint), Some(callback)) = (self.endpoint.take(), self.callback.take()) {
            unsafe {
                let _ = endpoint.UnregisterControlChangeNotify(&callback);
            }
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
        let watcher = VolumeWatcher::start(|_state| {});
        if let Ok(watcher) = watcher {
            assert!(watcher.endpoint.is_some());
            assert!(watcher.callback.is_some());
            // drop watcher safely unregisters
            drop(watcher);
        }
    }
}
