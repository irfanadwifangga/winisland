//! Modul pelacakan penggunaan GPU (Graphics Processing Unit) sistem Windows.
//!
//! Menggunakan Performance Data Helper (PDH) Win32 counter `\GPU Engine(*)\Utilization Percentage`.
//! Pendekatan ini lintas vendor (bekerja pada NVIDIA, AMD, dan Intel) tanpa memerlukan SDK eksternal
//! khusus vendor (seperti NVML).
//!
//! Seluruh pembacaan hanya dijalankan saat overlay dalam keadaan expanded (0% CPU idle saat idle/peek).

use windows::core::w;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};

/// UTF-16 byte sequence untuk substring "engtype_3D".
const NEEDLE_3D: [u16; 10] = [
    'e' as u16, 'n' as u16, 'g' as u16, 't' as u16, 'y' as u16, 'p' as u16, 'e' as u16, '_' as u16,
    '3' as u16, 'D' as u16,
];

/// Memeriksa apakah slice karakter UTF-16 mengandung substring `engtype_3D`.
pub fn is_3d_engine(name_utf16: &[u16]) -> bool {
    name_utf16.windows(NEEDLE_3D.len()).any(|w| w == NEEDLE_3D)
}

/// Menghitung persentase beban GPU dari total pemakaian engine 3D dan aktivitas engine lainnya.
pub fn calculate_gpu_usage(sum_3d: f64, max_other_engine: f64) -> u8 {
    let dominant = sum_3d.max(max_other_engine);
    (dominant.round() as u32).min(100) as u8
}

/// Pelacak pemakaian GPU sistem berbasis Windows PDH counter.
pub struct GpuTracker {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
    raw_buffer: Vec<u8>,
    available: bool,
    has_sample: bool,
    last_val: u8,
}

impl Default for GpuTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuTracker {
    /// Membuat instance `GpuTracker` baru dan menginisialisasi query PDH.
    ///
    /// Jika query PDH tidak didukung atau gagal diinisialisasi (mis. pada mesin tanpa counter GPU),
    /// instance akan berada dalam status `available = false` dan mengembalikan 0 tanpa panic.
    pub fn new() -> Self {
        unsafe {
            let mut query = PDH_HQUERY::default();
            if PdhOpenQueryW(None, 0, &mut query) != 0 {
                return Self::unavailable();
            }

            let mut counter = PDH_HCOUNTER::default();
            let counter_path = w!("\\GPU Engine(*)\\Utilization Percentage");
            if PdhAddEnglishCounterW(query, counter_path, 0, &mut counter) != 0 {
                let _ = PdhCloseQuery(query);
                return Self::unavailable();
            }

            // Pengambilan data sampel awal (baseline PDH untuk counter bertipe rate)
            let _ = PdhCollectQueryData(query);

            Self {
                query,
                counter,
                raw_buffer: Vec::with_capacity(32 * 1024),
                available: true,
                has_sample: false,
                last_val: 0,
            }
        }
    }

    /// Membuat instance nonaktif jika sistem tidak mendukung counter GPU.
    fn unavailable() -> Self {
        Self {
            query: PDH_HQUERY::default(),
            counter: PDH_HCOUNTER::default(),
            raw_buffer: Vec::new(),
            available: false,
            has_sample: false,
            last_val: 0,
        }
    }

    /// Mengembalikan true bila pelacakan GPU didukung dan siap digunakan pada mesin ini.
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Mereset status sampel saat overlay ditutup.
    pub fn reset(&mut self) {
        if self.available {
            unsafe {
                let _ = PdhCollectQueryData(self.query);
            }
        }
        self.has_sample = false;
        self.last_val = 0;
    }

    /// Mengambil sampel penggunaan GPU terkini dan mengembalikan persentasenya (0-100%).
    ///
    /// Aman dipanggil berulang kali; hanya membutuhkan ~1-2 ms per pemanggilan dan
    /// hanya berjalan saat overlay dibuka (expanded).
    pub fn sample(&mut self) -> u8 {
        if !self.available {
            return 0;
        }

        unsafe {
            let res = PdhCollectQueryData(self.query);
            if res != 0 {
                return self.last_val;
            }

            if !self.has_sample {
                self.has_sample = true;
                return self.last_val;
            }

            let mut buffer_size = 0;
            let mut item_count = 0;

            // Panggilan pertama untuk mengetahui ukuran byte buffer yang diperlukan
            let _ = PdhGetFormattedCounterArrayW(
                self.counter,
                PDH_FMT_DOUBLE,
                &mut buffer_size,
                &mut item_count,
                None,
            );

            if buffer_size == 0 {
                return self.last_val;
            }

            // Pastikan kapasitas buffer mencukupi byte array PDH beserta data string szName
            if self.raw_buffer.len() < buffer_size as usize {
                self.raw_buffer.resize(buffer_size as usize, 0);
            }

            let status = PdhGetFormattedCounterArrayW(
                self.counter,
                PDH_FMT_DOUBLE,
                &mut buffer_size,
                &mut item_count,
                Some(self.raw_buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W),
            );

            if status == 0 && item_count > 0 {
                let items_ptr = self.raw_buffer.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_W;
                let items = std::slice::from_raw_parts(items_ptr, item_count as usize);

                let mut sum_3d = 0.0;
                let mut max_engine = 0.0;

                for item in items {
                    let val = item.FmtValue.Anonymous.doubleValue;
                    if val <= 0.01 {
                        continue;
                    }

                    if !item.szName.is_null() {
                        let len = (0..).take_while(|&i| *item.szName.0.offset(i) != 0).count();
                        let slice = std::slice::from_raw_parts(item.szName.0, len);
                        if is_3d_engine(slice) {
                            sum_3d += val;
                        }
                    }

                    if val > max_engine {
                        max_engine = val;
                    }
                }

                self.last_val = calculate_gpu_usage(sum_3d, max_engine);
            }

            self.last_val
        }
    }
}

impl Drop for GpuTracker {
    fn drop(&mut self) {
        if self.available && !self.query.0.is_null() {
            unsafe {
                let _ = PdhCloseQuery(self.query);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_is_3d_engine_detection() {
        let sample_3d: Vec<u16> = "pid_25508_luid_0x00000000_0x00012B7A_phys_0_eng_0_engtype_3D"
            .encode_utf16()
            .collect();
        assert!(is_3d_engine(&sample_3d));

        let sample_copy: Vec<u16> =
            "pid_25508_luid_0x00000000_0x00012B7A_phys_0_eng_4_engtype_Copy"
                .encode_utf16()
                .collect();
        assert!(!is_3d_engine(&sample_copy));

        let sample_video: Vec<u16> =
            "pid_4_luid_0x00000000_0x00012B7A_phys_0_eng_9_engtype_VideoDecode"
                .encode_utf16()
                .collect();
        assert!(!is_3d_engine(&sample_video));

        let sample_empty: Vec<u16> = vec![];
        assert!(!is_3d_engine(&sample_empty));
    }

    #[test]
    fn test_calculate_gpu_usage_bounds_and_clamping() {
        assert_eq!(calculate_gpu_usage(0.0, 0.0), 0);
        assert_eq!(calculate_gpu_usage(35.4, 10.0), 35);
        assert_eq!(calculate_gpu_usage(35.6, 10.0), 36);
        assert_eq!(calculate_gpu_usage(10.0, 75.2), 75);
        assert_eq!(calculate_gpu_usage(120.0, 50.0), 100);
    }

    #[test]
    fn test_gpu_tracker_unavailable_safe_fallback() {
        let mut tracker = GpuTracker::unavailable();
        assert!(!tracker.is_available());
        assert_eq!(tracker.sample(), 0);
        tracker.reset();
        assert_eq!(tracker.sample(), 0);
    }

    #[test]
    fn test_gpu_tracker_lifecycle() {
        let mut tracker = GpuTracker::new();
        // Pada Windows 10/11 sistem pengujian ini, tracker tersedia
        if tracker.is_available() {
            let val = tracker.sample();
            assert!(val <= 100);
            tracker.reset();
            let val_after = tracker.sample();
            assert!(val_after <= 100);
        }
    }
}
