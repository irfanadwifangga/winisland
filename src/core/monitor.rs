//! Penanganan multi-monitor dan penentuan batas layar menggunakan Win32 GDI.

use std::mem;

use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

/// Batas koordinat fisik satu display monitor dalam desktop virtual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorBounds {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub is_primary: bool,
}

unsafe extern "system" fn enum_monitors_cb(
    h_monitor: HMONITOR,
    _hdc: HDC,
    _prc: *mut RECT,
    dw_data: LPARAM,
) -> BOOL {
    let mut info: MONITORINFO = mem::zeroed();
    info.cbSize = mem::size_of::<MONITORINFO>() as u32;

    if GetMonitorInfoW(h_monitor, &mut info).as_bool() {
        let monitors = &mut *(dw_data.0 as *mut Vec<MonitorBounds>);
        let is_primary = (info.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY
        let left = info.rcMonitor.left;
        let top = info.rcMonitor.top;
        let width = info.rcMonitor.right - info.rcMonitor.left;
        let height = info.rcMonitor.bottom - info.rcMonitor.top;

        monitors.push(MonitorBounds {
            left,
            top,
            width,
            height,
            is_primary,
        });
    }
    TRUE
}

/// Mengambil daftar seluruh monitor fisik yang sedang terhubung.
/// Monitor utama (primary) selalu ditempatkan di indeks 0.
pub fn list_monitors() -> Vec<MonitorBounds> {
    let mut monitors: Vec<MonitorBounds> = Vec::new();
    unsafe {
        let state_ptr = LPARAM(&mut monitors as *mut Vec<MonitorBounds> as isize);
        let _ = EnumDisplayMonitors(None, None, Some(enum_monitors_cb), state_ptr);
    }

    if monitors.is_empty() {
        // Fallback bila enumerasi gagal: gunakan metrik monitor utama standar
        let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        return vec![MonitorBounds {
            left: 0,
            top: 0,
            width: w,
            height: h,
            is_primary: true,
        }];
    }

    // Pastikan monitor primary selalu berada di posisi pertama (indeks 0),
    // diikuti monitor sekunder yang diurutkan dari koordinat left terkecil.
    monitors.sort_by(|a, b| match (a.is_primary, b.is_primary) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.left.cmp(&b.left),
    });

    monitors
}

/// Mengambil batas monitor berdasarkan indeks pilihan.
/// Bila indeks melebihi jumlah monitor yang tersedia, otomatis kembali ke monitor utama (indeks 0).
pub fn get_monitor(index: usize) -> MonitorBounds {
    let list = list_monitors();
    if index < list.len() {
        list[index]
    } else {
        list[0]
    }
}

/// Label yang ditampilkan pada menu tray untuk indeks monitor tertentu.
pub fn monitor_label(index: usize) -> String {
    let monitors = list_monitors();
    let count = monitors.len();
    if count <= 1 {
        return "Display: Primary".to_string();
    }
    let actual_idx = if index < count { index } else { 0 };
    if monitors[actual_idx].is_primary {
        format!("Display: {} (Primary)", actual_idx + 1)
    } else {
        format!("Display: {}", actual_idx + 1)
    }
}

/// Indeks monitor berikutnya saat pengguna mengklik menu rotasi display.
pub fn next_monitor_index(current: usize) -> usize {
    let count = list_monitors().len();
    if count <= 1 {
        0
    } else {
        (current + 1) % count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_monitors_not_empty() {
        let list = list_monitors();
        assert!(!list.is_empty(), "harus mendeteksi minimal 1 monitor");
        assert!(
            list[0].is_primary,
            "monitor pertama pada list harus selalu primary"
        );
        assert!(list[0].width > 0);
        assert!(list[0].height > 0);
    }

    #[test]
    fn test_get_monitor_fallback() {
        let mon = get_monitor(9999);
        assert!(
            mon.is_primary,
            "indeks tidak valid harus fallback ke primary"
        );
    }

    #[test]
    fn test_next_monitor_index() {
        let count = list_monitors().len();
        if count <= 1 {
            assert_eq!(next_monitor_index(0), 0);
            assert_eq!(next_monitor_index(5), 0);
        } else {
            assert_eq!(next_monitor_index(0), 1);
            assert_eq!(next_monitor_index(count - 1), 0);
        }
    }
}
