//! Modul pembacaan statistik sistem (CPU, RAM, Baterai) dan tanggal lokal.
//!
//! Dirancang untuk efisiensi tinggi tanpa dependensi eksternal tambahan:
//! - CPU: `GetSystemTimes` (menghitung selisih waktu kernel, user, dan idle).
//! - RAM: `GlobalMemoryStatusEx` (`dwMemoryLoad`, persentase beban memori fisik sistem).
//! - Baterai: `GetSystemPowerStatus` (persentase baterai dan status pengisian daya; None jika desktop).
//! - Tanggal: `GetLocalTime` (hari dan tanggal dalam bahasa Inggris).
//!
//! Seluruh polling hanya dijalankan saat overlay dalam keadaan expanded (0% CPU saat idle).

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

/// Informasi status baterai perangkat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryInfo {
    /// Persentase daya baterai (0-100).
    pub percent: u8,
    /// True jika sedang tersambung ke adaptor / mengisi daya.
    pub charging: bool,
}

/// Mengonversi struct `FILETIME` Win32 menjadi integer 64-bit.
fn filetime_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

/// Menghitung persentase penggunaan CPU dari selisih waktu idle, kernel, dan user.
///
/// Pada arsitektur Windows NT, waktu kernel (`kernel_diff`) sudah mencakup waktu idle (`idle_diff`).
/// Total waktu sistem adalah `kernel_diff + user_diff`.
pub fn calculate_cpu_usage(idle_diff: u64, kernel_diff: u64, user_diff: u64) -> u8 {
    let total_sys = kernel_diff.saturating_add(user_diff);
    if total_sys == 0 {
        return 0;
    }
    let busy = total_sys.saturating_sub(idle_diff);
    let pct = (busy as f64 / total_sys as f64 * 100.0).round();
    pct.clamp(0.0, 100.0) as u8
}

/// Pelacak pemakaian CPU sistem berbasis delta waktu.
#[derive(Debug, Clone)]
pub struct CpuTracker {
    prev_idle: u64,
    prev_kernel: u64,
    prev_user: u64,
    has_prev: bool,
    last_val: u8,
}

impl Default for CpuTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuTracker {
    pub fn new() -> Self {
        Self {
            prev_idle: 0,
            prev_kernel: 0,
            prev_user: 0,
            has_prev: false,
            last_val: 0,
        }
    }

    /// Mereset histori pengukuran delta.
    pub fn reset(&mut self) {
        self.has_prev = false;
        self.last_val = 0;
    }

    /// Mengambil sampel waktu CPU terkini dan mengembalikan persentase penggunaan.
    pub fn sample(&mut self) -> u8 {
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();

        let ok = unsafe {
            GetSystemTimes(
                Some(&mut idle as *mut FILETIME),
                Some(&mut kernel as *mut FILETIME),
                Some(&mut user as *mut FILETIME),
            )
        };

        if ok.is_err() {
            return self.last_val;
        }

        let idle_u64 = filetime_to_u64(&idle);
        let kernel_u64 = filetime_to_u64(&kernel);
        let user_u64 = filetime_to_u64(&user);

        if self.has_prev {
            let idle_diff = idle_u64.saturating_sub(self.prev_idle);
            let kernel_diff = kernel_u64.saturating_sub(self.prev_kernel);
            let user_diff = user_u64.saturating_sub(self.prev_user);

            let usage = calculate_cpu_usage(idle_diff, kernel_diff, user_diff);
            self.last_val = usage;
        }

        self.prev_idle = idle_u64;
        self.prev_kernel = kernel_u64;
        self.prev_user = user_u64;
        self.has_prev = true;

        self.last_val
    }
}

/// Mengambil persentase beban RAM fisik sistem (0-100%).
pub fn get_ram_load() -> u8 {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };

    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok.is_ok() {
        status.dwMemoryLoad.min(100) as u8
    } else {
        0
    }
}

/// Memparsing struktur status daya sistem untuk mendeteksi ketersediaan baterai.
pub fn parse_battery_status(status: &SYSTEM_POWER_STATUS) -> Option<BatteryInfo> {
    // BatteryFlag 128 = tidak ada baterai sistem (mis. PC desktop).
    // BatteryLifePercent 255 = status tidak diketahui / tidak ada baterai.
    if status.BatteryFlag == 128 || status.BatteryLifePercent == 255 {
        return None;
    }

    let percent = status.BatteryLifePercent.min(100);
    // ACLineStatus 1 = tersambung listrik. BatteryFlag bit 3 (nilai 8) = sedang mengisi daya.
    let charging = status.ACLineStatus == 1 || (status.BatteryFlag & 8) != 0;

    Some(BatteryInfo { percent, charging })
}

/// Mengambil status baterai sistem terkini.
pub fn get_battery_info() -> Option<BatteryInfo> {
    let mut status = SYSTEM_POWER_STATUS::default();
    let ok = unsafe { GetSystemPowerStatus(&mut status) };
    if ok.is_ok() {
        parse_battery_status(&status)
    } else {
        None
    }
}

/// Nama-nama hari singkatan dalam bahasa Inggris (0 = Sunday).
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// Nama-nama bulan singkatan dalam bahasa Inggris (1 = Jan).
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Memformat nama hari dan tanggal dari komponen waktu sistem.
pub fn format_date_parts(day_of_week: usize, day: u16, month: usize) -> String {
    let day_name = if day_of_week < DAYS.len() {
        DAYS[day_of_week]
    } else {
        "???"
    };

    let month_name = if (1..=12).contains(&month) {
        MONTHS[month - 1]
    } else {
        "???"
    };

    format!("{}, {} {}", day_name, day, month_name)
}

/// Mengambil tanggal lokal sistem dalam format singkatan bahasa Inggris (mis. "Thu, 22 Oct").
pub fn local_date_string() -> String {
    #[repr(C)]
    struct SystemTimeWin32 {
        w_year: u16,
        w_month: u16,
        w_day_of_week: u16,
        w_day: u16,
        w_hour: u16,
        w_minute: u16,
        w_second: u16,
        w_milliseconds: u16,
    }

    extern "system" {
        fn GetLocalTime(lp_system_time: *mut SystemTimeWin32);
    }

    let mut st = SystemTimeWin32 {
        w_year: 0,
        w_month: 0,
        w_day_of_week: 0,
        w_day: 0,
        w_hour: 0,
        w_minute: 0,
        w_second: 0,
        w_milliseconds: 0,
    };

    unsafe {
        GetLocalTime(&mut st);
    }

    format_date_parts(st.w_day_of_week as usize, st.w_day, st.w_month as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cpu_usage_zero_diff() {
        assert_eq!(calculate_cpu_usage(0, 0, 0), 0);
    }

    #[test]
    fn test_calculate_cpu_usage_full_idle() {
        // kernel = 100 (termasuk idle 100), user = 0 -> busy = 0%
        assert_eq!(calculate_cpu_usage(100, 100, 0), 0);
    }

    #[test]
    fn test_calculate_cpu_usage_half_busy() {
        // total = 100 kernel + 100 user = 200. idle = 100. busy = 100/200 = 50%
        assert_eq!(calculate_cpu_usage(100, 100, 100), 50);
    }

    #[test]
    fn test_calculate_cpu_usage_full_busy() {
        // total = 50 kernel + 50 user = 100. idle = 0. busy = 100%
        assert_eq!(calculate_cpu_usage(0, 50, 50), 100);
    }

    #[test]
    fn test_parse_battery_status_no_battery_flag() {
        let status = SYSTEM_POWER_STATUS {
            ACLineStatus: 1,
            BatteryFlag: 128,
            BatteryLifePercent: 100,
            SystemStatusFlag: 0,
            BatteryLifeTime: 0,
            BatteryFullLifeTime: 0,
        };
        assert_eq!(parse_battery_status(&status), None);
    }

    #[test]
    fn test_parse_battery_status_unknown_percent() {
        let status = SYSTEM_POWER_STATUS {
            ACLineStatus: 1,
            BatteryFlag: 0,
            BatteryLifePercent: 255,
            SystemStatusFlag: 0,
            BatteryLifeTime: 0,
            BatteryFullLifeTime: 0,
        };
        assert_eq!(parse_battery_status(&status), None);
    }

    #[test]
    fn test_parse_battery_status_on_battery() {
        let status = SYSTEM_POWER_STATUS {
            ACLineStatus: 0,
            BatteryFlag: 0,
            BatteryLifePercent: 82,
            SystemStatusFlag: 0,
            BatteryLifeTime: 3600,
            BatteryFullLifeTime: 7200,
        };
        assert_eq!(
            parse_battery_status(&status),
            Some(BatteryInfo {
                percent: 82,
                charging: false
            })
        );
    }

    #[test]
    fn test_parse_battery_status_charging() {
        let status = SYSTEM_POWER_STATUS {
            ACLineStatus: 1,
            BatteryFlag: 8,
            BatteryLifePercent: 95,
            SystemStatusFlag: 0,
            BatteryLifeTime: 0,
            BatteryFullLifeTime: 0,
        };
        assert_eq!(
            parse_battery_status(&status),
            Some(BatteryInfo {
                percent: 95,
                charging: true
            })
        );
    }

    #[test]
    fn test_format_date_parts() {
        // 4 = Thursday, 22, 10 = October
        assert_eq!(format_date_parts(4, 22, 10), "Thu, 22 Oct");
        // 0 = Sunday, 1, 1 = January
        assert_eq!(format_date_parts(0, 1, 1), "Sun, 1 Jan");
        // out of bounds fallback
        assert_eq!(format_date_parts(9, 15, 14), "???, 15 ???");
    }
}
