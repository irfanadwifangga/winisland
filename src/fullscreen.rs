//! Deteksi aplikasi atau game yang sedang berjalan dalam mode layar penuh (fullscreen).
//!
//! Saat jendela latar depan (foreground) menutupi seluruh area monitor tempat overlay
//! berada dan tidak memiliki caption bar (WS_CAPTION), overlay disembunyikan sementara
//! agar tidak menghalangi game atau video layar penuh.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetDesktopWindow, GetForegroundWindow, GetShellWindow, GetWindowLongW,
    GetWindowRect, IsIconic, IsWindowVisible, GWL_STYLE, WS_CAPTION, WS_CHILD,
};

use crate::monitor::{self, MonitorBounds};

/// Mengecek apakah rectangle jendela menutupi seluruh area monitor tertentu.
pub fn rect_covers_monitor(rect: &RECT, mon: &MonitorBounds) -> bool {
    rect.left <= mon.left
        && rect.top <= mon.top
        && rect.right >= (mon.left + mon.width)
        && rect.bottom >= (mon.top + mon.height)
}

/// Mengecek apakah style jendela menandakan mode fullscreen (tidak memiliki WS_CAPTION dan bukan child window).
pub fn is_fullscreen_style(style: u32) -> bool {
    let has_caption = (style & WS_CAPTION.0) == WS_CAPTION.0;
    let is_child = (style & WS_CHILD.0) != 0;
    !has_caption && !is_child
}

/// Memeriksa apakah nama kelas jendela termasuk yang harus diabaikan (shell / desktop / taskbar).
pub fn is_ignored_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "Windows.UI.Core.CoreWindow"
    )
}

/// Mengecek apakah geometri dan style jendela memenuhi syarat sebagai fullscreen di monitor target.
pub fn is_fullscreen_geometry(rect: &RECT, mon: &MonitorBounds, style: u32) -> bool {
    rect_covers_monitor(rect, mon) && is_fullscreen_style(style)
}

/// Memeriksa apakah jendela foreground saat ini sedang dalam mode fullscreen di monitor yang ditentukan.
pub fn is_foreground_fullscreen_on_monitor(our_hwnd: Option<HWND>, monitor_idx: usize) -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }

        // Jangan sembunyikan jika jendela foreground adalah overlay kita sendiri
        if let Some(our) = our_hwnd {
            if fg == our {
                return false;
            }
        }

        // Abaikan desktop dan shell
        let desktop = GetDesktopWindow();
        let shell = GetShellWindow();
        if fg == desktop || fg == shell {
            return false;
        }

        // Harus jendela yang terlihat dan tidak diminimalkan
        if !IsWindowVisible(fg).as_bool() || IsIconic(fg).as_bool() {
            return false;
        }

        // Cek nama kelas jendela (abaikan desktop worker, taskbar, dll)
        let mut class_buf = [0u16; 64];
        let len = GetClassNameW(fg, &mut class_buf);
        if len > 0 {
            let class_str = String::from_utf16_lossy(&class_buf[..len as usize]);
            if is_ignored_class(&class_str) {
                return false;
            }
        }

        let mut rect = RECT::default();
        if GetWindowRect(fg, &mut rect).is_err() {
            return false;
        }

        let style = GetWindowLongW(fg, GWL_STYLE) as u32;

        let mon = monitor::get_monitor(monitor_idx);
        is_fullscreen_geometry(&rect, &mon, style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mon() -> MonitorBounds {
        MonitorBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            is_primary: true,
        }
    }

    #[test]
    fn test_rect_covers_monitor_exact() {
        let mon = sample_mon();
        let rect = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        assert!(rect_covers_monitor(&rect, &mon));
    }

    #[test]
    fn test_rect_covers_monitor_with_overlap() {
        let mon = sample_mon();
        let rect = RECT {
            left: -2,
            top: -2,
            right: 1922,
            bottom: 1082,
        };
        assert!(rect_covers_monitor(&rect, &mon));
    }

    #[test]
    fn test_rect_smaller_than_monitor() {
        let mon = sample_mon();
        let rect = RECT {
            left: 100,
            top: 100,
            right: 1200,
            bottom: 800,
        };
        assert!(!rect_covers_monitor(&rect, &mon));
    }

    #[test]
    fn test_rect_maximized_window_with_taskbar() {
        // Jendela maximized biasa di Windows menyisakan taskbar di bawah (misal tinggi 48px)
        let mon = sample_mon();
        let rect = RECT {
            left: -8,
            top: -8,
            right: 1928,
            bottom: 1032, // < 1080
        };
        assert!(!rect_covers_monitor(&rect, &mon));
    }

    #[test]
    fn test_rect_different_monitor() {
        let mon1 = MonitorBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            is_primary: true,
        };
        let mon2 = MonitorBounds {
            left: 1920,
            top: 0,
            width: 1920,
            height: 1080,
            is_primary: false,
        };

        // Jendela fullscreen di monitor 2
        let rect_mon2 = RECT {
            left: 1920,
            top: 0,
            right: 3840,
            bottom: 1080,
        };

        assert!(!rect_covers_monitor(&rect_mon2, &mon1));
        assert!(rect_covers_monitor(&rect_mon2, &mon2));
    }

    #[test]
    fn test_is_fullscreen_style() {
        // Style jendela biasa dengan title bar (WS_CAPTION)
        let standard_window_style = WS_CAPTION.0 | 0x00040000;
        assert!(!is_fullscreen_style(standard_window_style));

        // Style fullscreen borderless (tanpa WS_CAPTION, bukan child)
        let borderless_fs_style = 0x80000000 | 0x10000000; // WS_POPUP | WS_VISIBLE
        assert!(is_fullscreen_style(borderless_fs_style));

        // Child window (WS_CHILD) tidak boleh dianggap fullscreen
        let child_style = WS_CHILD.0 | 0x10000000;
        assert!(!is_fullscreen_style(child_style));
    }

    #[test]
    fn test_is_ignored_class() {
        assert!(is_ignored_class("Progman"));
        assert!(is_ignored_class("WorkerW"));
        assert!(is_ignored_class("Shell_TrayWnd"));
        assert!(is_ignored_class("Shell_SecondaryTrayWnd"));
        assert!(is_ignored_class("Windows.UI.Core.CoreWindow"));
        assert!(!is_ignored_class("Chrome_WidgetWin_1"));
        assert!(!is_ignored_class("Notepad"));
        assert!(!is_ignored_class("UnrealWindow"));
    }
}
