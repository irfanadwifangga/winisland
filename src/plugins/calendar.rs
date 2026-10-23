//! Modul perhitungan dan penyusunan grid kalender bulanan lokal.
//!
//! Menghitung jumlah hari, tahun kabisat, dan hari dalam seminggu secara murni matematis
//! tanpa dependensi eksternal, sehingga binary tetap kecil dan efisien.

/// Representasi satu sel tanggal pada grid kalender (6 baris × 7 kolom = 42 sel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarCell {
    /// Nomor tanggal (1..=31).
    pub day: u16,
    /// Teks tanggal untuk ditampilkan pada UI (mis. "22" atau "" jika di luar bulan aktif).
    pub text: String,
    /// True jika tanggal ini berada di dalam bulan yang sedang dilihat.
    pub is_current_month: bool,
    /// True jika tanggal ini adalah hari ini menurut jam sistem.
    pub is_today: bool,
}

/// Menghitung jumlah hari dalam suatu bulan pada tahun tertentu.
pub fn days_in_month(year: u16, month: u16) -> u16 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 31,
    }
}

/// Mengecek apakah suatu tahun merupakan tahun kabisat (Gregorian).
pub fn is_leap_year(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

/// Menghitung hari dalam seminggu untuk tanggal tertentu (0 = Sunday, 1 = Monday, ..., 6 = Saturday).
///
/// Menggunakan algoritma Zeller/Tomohiko Sakamoto yang efisien.
pub fn weekday(year: u16, month: u16, day: u16) -> u16 {
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = year as i32;
    if month < 3 {
        y -= 1;
    }
    let m_idx = (month.clamp(1, 12) - 1) as usize;
    let val = y + y / 4 - y / 100 + y / 400 + OFFSETS[m_idx] + (day as i32);
    val.rem_euclid(7) as u16
}

/// Nama-nama bulan lengkap dalam bahasa Inggris.
const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Menghasilkan judul bulan dan tahun, mis. "October 2026".
pub fn format_month_title(year: u16, month: u16) -> String {
    let m_idx = (month.clamp(1, 12) - 1) as usize;
    format!("{} {}", MONTH_NAMES[m_idx], year)
}

/// Mengambil tanggal lokal saat ini dari Windows API `GetLocalTime`.
pub fn current_system_date() -> (u16, u16, u16) {
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

    (st.w_year, st.w_month, st.w_day)
}

/// Status kalender yang bisa dinavigasi (bulan/tahun saat ini atau bulan yang sedang dilihat).
#[derive(Debug, Clone)]
pub struct CalendarState {
    pub view_year: u16,
    pub view_month: u16,
    pub today_year: u16,
    pub today_month: u16,
    pub today_day: u16,
}

impl CalendarState {
    /// Membuat status kalender baru berdasarkan tanggal sistem saat ini.
    pub fn new() -> Self {
        let (y, m, d) = current_system_date();
        Self {
            view_year: y,
            view_month: m,
            today_year: y,
            today_month: m,
            today_day: d,
        }
    }

    /// Memperbarui tanggal "hari ini" dari jam sistem.
    pub fn refresh_today(&mut self) {
        let (y, m, d) = current_system_date();
        self.today_year = y;
        self.today_month = m;
        self.today_day = d;
    }

    /// Berpindah ke bulan sebelumnya.
    pub fn prev_month(&mut self) {
        if self.view_month <= 1 {
            self.view_month = 12;
            self.view_year = self.view_year.saturating_sub(1);
        } else {
            self.view_month -= 1;
        }
    }

    /// Berpindah ke bulan berikutnya.
    pub fn next_month(&mut self) {
        if self.view_month >= 12 {
            self.view_month = 1;
            self.view_year = self.view_year.saturating_add(1);
        } else {
            self.view_month += 1;
        }
    }

    /// Mengembalikan tampilan ke bulan saat ini (hari ini).
    pub fn reset_to_today(&mut self) {
        self.refresh_today();
        self.view_year = self.today_year;
        self.view_month = self.today_month;
    }

    /// Menghasilkan 42 sel grid kalender untuk bulan yang sedang dilihat.
    pub fn generate_grid(&self) -> Vec<CalendarCell> {
        let total_days = days_in_month(self.view_year, self.view_month);
        let first_weekday = weekday(self.view_year, self.view_month, 1); // 0 = Sunday

        // Bulan sebelumnya untuk padding awal
        let (prev_year, prev_month) = if self.view_month == 1 {
            (self.view_year.saturating_sub(1), 12)
        } else {
            (self.view_year, self.view_month - 1)
        };
        let prev_month_days = days_in_month(prev_year, prev_month);

        let mut cells = Vec::with_capacity(42);

        // 1. Sel padding bulan sebelumnya
        let lead_days = first_weekday;
        for i in 0..lead_days {
            let day_num = prev_month_days - lead_days + i + 1;
            cells.push(CalendarCell {
                day: day_num,
                text: day_num.to_string(),
                is_current_month: false,
                is_today: false,
            });
        }

        // 2. Sel tanggal bulan aktif
        for d in 1..=total_days {
            let is_today = self.view_year == self.today_year
                && self.view_month == self.today_month
                && d == self.today_day;

            cells.push(CalendarCell {
                day: d,
                text: d.to_string(),
                is_current_month: true,
                is_today,
            });
        }

        // 3. Sel padding bulan berikutnya hingga genap 42 sel (6 baris × 7 kolom)
        let mut next_day = 1;
        while cells.len() < 42 {
            cells.push(CalendarCell {
                day: next_day,
                text: next_day.to_string(),
                is_current_month: false,
                is_today: false,
            });
            next_day += 1;
        }

        cells
    }
}

impl Default for CalendarState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_leap_year() {
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(2025));
        assert!(!is_leap_year(2026));
        assert!(!is_leap_year(1900));
    }

    #[test]
    fn test_days_in_month() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29); // kabisat
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 10), 31);
    }

    #[test]
    fn test_weekday_calculation() {
        // 22 Oktober 2026 adalah hari Kamis (4, 0=Sun)
        assert_eq!(weekday(2026, 10, 22), 4);
        // 1 Januari 2026 adalah hari Kamis (4)
        assert_eq!(weekday(2026, 1, 1), 4);
        // 1 Maret 2026 adalah hari Minggu (0)
        assert_eq!(weekday(2026, 3, 1), 0);
        // 29 Februari 2024 adalah hari Kamis (4)
        assert_eq!(weekday(2024, 2, 29), 4);
    }

    #[test]
    fn test_format_month_title() {
        assert_eq!(format_month_title(2026, 10), "October 2026");
        assert_eq!(format_month_title(2027, 1), "January 2027");
    }

    #[test]
    fn test_calendar_state_navigation() {
        let mut state = CalendarState {
            view_year: 2026,
            view_month: 10,
            today_year: 2026,
            today_month: 10,
            today_day: 22,
        };

        state.next_month();
        assert_eq!(state.view_month, 11);
        assert_eq!(state.view_year, 2026);

        state.prev_month();
        assert_eq!(state.view_month, 10);
        assert_eq!(state.view_year, 2026);

        // pergantian tahun mundur
        state.view_month = 1;
        state.prev_month();
        assert_eq!(state.view_month, 12);
        assert_eq!(state.view_year, 2025);

        // pergantian tahun maju
        state.next_month();
        assert_eq!(state.view_month, 1);
        assert_eq!(state.view_year, 2026);
    }

    #[test]
    fn test_generate_grid_length_and_today() {
        let state = CalendarState {
            view_year: 2026,
            view_month: 10,
            today_year: 2026,
            today_month: 10,
            today_day: 22,
        };

        let grid = state.generate_grid();
        assert_eq!(grid.len(), 42); // selalu 42 sel

        // 1 Okt 2026 adalah hari Kamis (offset 4). Jadi 4 sel pertama adalah padding bulan Sept.
        assert!(!grid[0].is_current_month);
        assert!(!grid[3].is_current_month);
        assert!(grid[4].is_current_month);
        assert_eq!(grid[4].day, 1);

        // 22 Oktober 2026 harus bertanda is_today = true
        let today_cell = grid
            .iter()
            .find(|c| c.is_today)
            .expect("harus ada is_today");
        assert_eq!(today_cell.day, 22);
        assert!(today_cell.is_current_month);
    }
}
