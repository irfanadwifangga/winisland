//! Pengingat (Jadwal) satu kali yang disimpan lokal di `%APPDATA%\winisland\schedule.json`.
//!
//! Versi pertama sengaja sederhana: tanpa pengulangan, tanpa sinkronisasi, tanpa
//! dependensi baru. Waktu target disimpan sebagai detik Unix sehingga aman untuk
//! "besok" dan tetap benar walau aplikasi dimulai ulang. Penulisan berkas memakai
//! pola yang sama dengan Tasks: tulis ke berkas sementara lalu ganti nama, dan
//! berkas yang tidak terbaca dipindah ke `.bak` alih-alih ditimpa.
//!
//! Pengingat tidak memakai polling: pemanggil memasang satu timer single-shot
//! Slint untuk jadwal terdekat (lihat `next_duration` dan `take_due`).

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crate::plugins::tasks::{json_escape, json_unescape};

/// Satu pengingat terjadwal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleItem {
    pub id: u64,
    /// Waktu target dalam detik Unix.
    pub at: u64,
    pub label: String,
}

/// Pengelola daftar pengingat lokal.
#[derive(Debug)]
pub struct ScheduleManager {
    items: Vec<ScheduleItem>,
    next_id: u64,
    file_path: PathBuf,
}

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

/// Detik sejak tengah malam lokal saat ini; 0 bila jam sistem tidak terbaca.
fn local_secs_of_day() -> u64 {
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
    (st.w_hour as u64) * 3600 + (st.w_minute as u64) * 60 + st.w_second as u64
}

/// Detik sejak epoch Unix; 0 bila jam sistem lebih awal dari 1970.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Awal hari lokal (tengah malam) dalam detik Unix. Dipakai juga penghubung UI
/// untuk memformat jam tampilan.
pub fn local_midnight_unix_public() -> u64 {
    local_midnight_unix()
}

/// Awal hari lokal (tengah malam) dalam detik Unix.
fn local_midnight_unix() -> u64 {
    unix_now().saturating_sub(local_secs_of_day())
}

/// Menghitung waktu target detik Unix untuk jam:menit hari ini (atau besok bila
/// waktu itu sudah lewat).
fn target_unix(hour: u32, minute: u32) -> u64 {
    let target_of_day = (hour as u64) * 3600 + (minute as u64) * 60;
    let now_of_day = local_secs_of_day();
    let mut at = local_midnight_unix() + target_of_day;
    if target_of_day <= now_of_day {
        at += 24 * 3600;
    }
    at
}

/// Memformat waktu target sebagai "HH:MM" lokal.
/// `midnight` adalah awal hari lokal dalam detik Unix (lihat `local_midnight_unix`).
pub fn format_hhmm(at: u64, midnight: u64) -> String {
    let secs_of_day = at.saturating_sub(midnight) % (24 * 3600);
    format!("{:02}:{:02}", secs_of_day / 3600, (secs_of_day % 3600) / 60)
}

/// Memecah input pengguna "HH:MM label" menjadi (jam, menit, label).
/// Label boleh kosong. Mengembalikan `None` bila format waktunya salah.
pub fn parse_input(input: &str) -> Option<(u32, u32, String)> {
    let trimmed = input.trim();
    let (time_part, label) = match trimmed.split_once(char::is_whitespace) {
        Some((t, rest)) => (t, rest.trim().to_string()),
        None => (trimmed, String::new()),
    };
    let (h, m) = time_part.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    let hour = h.parse::<u32>().ok()?;
    let minute = m.parse::<u32>().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour, minute, label))
}

impl Default for ScheduleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduleManager {
    /// Membuat pengelola dengan lokasi berkas bawaan di AppData.
    pub fn new() -> Self {
        Self::with_path(default_schedule_path())
    }

    /// Membuat pengelola dengan lokasi berkas tertentu (untuk pengujian unit).
    pub fn with_path(file_path: PathBuf) -> Self {
        let mut mgr = Self {
            items: Vec::new(),
            next_id: 1,
            file_path,
        };
        mgr.load();
        mgr
    }

    /// Mengembalikan referensi slice seluruh pengingat (urut berdasarkan waktu).
    pub fn items(&self) -> &[ScheduleItem] {
        &self.items
    }

    /// Menambahkan pengingat baru untuk jam:menit (hari ini, atau besok bila lewat).
    /// Mengembalikan ID baru, atau `None` bila label kosong tanpa waktu.
    pub fn add(&mut self, hour: u32, minute: u32, label: &str) -> Option<u64> {
        let label = label.trim();
        if label.is_empty() {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(ScheduleItem {
            id,
            at: target_unix(hour, minute),
            label: label.to_string(),
        });
        self.items.sort_by_key(|it| it.at);
        if let Err(e) = self.save() {
            eprintln!("[schedule] FAILED to save: {e}");
        }
        Some(id)
    }

    /// Menghapus pengingat dengan ID tertentu.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|it| it.id != id);
        let removed = self.items.len() != before;
        if removed {
            if let Err(e) = self.save() {
                eprintln!("[schedule] FAILED to save: {e}");
            }
        }
        removed
    }

    /// Mengambil semua pengingat yang waktunya sudah tercapai (termasuk yang
    /// terlewat saat aplikasi mati atau komputer tidur), lalu menghapusnya.
    pub fn take_due(&mut self) -> Vec<ScheduleItem> {
        let now = unix_now();
        let mut due: Vec<ScheduleItem> = self
            .items
            .iter()
            .filter(|it| it.at <= now)
            .cloned()
            .collect();
        if due.is_empty() {
            return due;
        }
        let due_ids: Vec<u64> = due.iter().map(|it| it.id).collect();
        self.items.retain(|it| !due_ids.contains(&it.id));
        if let Err(e) = self.save() {
            eprintln!("[schedule] FAILED to save: {e}");
        }
        due.sort_by_key(|it| it.at);
        due
    }

    /// Durasi dari sekarang sampai pengingat terdekat yang belum tercapai.
    pub fn next_duration(&self) -> Option<Duration> {
        let now = unix_now();
        self.items
            .iter()
            .filter(|it| it.at > now)
            .map(|it| Duration::from_secs(it.at - now))
            .min()
    }

    /// Memuat daftar pengingat dari berkas. Berkas yang tidak terbaca atau tidak
    /// menghasilkan satu pun item dipindah ke `.bak` (pola yang sama dengan Tasks).
    pub fn load(&mut self) {
        if !self.file_path.exists() {
            self.items.clear();
            self.next_id = 1;
            return;
        }
        match fs::read_to_string(&self.file_path) {
            Ok(content) => {
                let parsed = parse_schedule_json(&content);
                if parsed.is_empty() && content.trim() != "[]" {
                    let bak = self.file_path.with_extension("json.bak");
                    if let Err(re) = fs::rename(&self.file_path, &bak) {
                        eprintln!("[schedule] FAILED to back up unreadable file: {re}");
                    } else {
                        eprintln!("[schedule] unreadable file moved to {bak:?}");
                    }
                    self.items.clear();
                    self.next_id = 1;
                } else {
                    self.items = parsed;
                    self.items.sort_by_key(|it| it.at);
                    let max_id = self.items.iter().map(|it| it.id).max().unwrap_or(0);
                    self.next_id = max_id.saturating_add(1).min(i32::MAX as u64);
                }
            }
            Err(e) => {
                let bak = self.file_path.with_extension("json.bak");
                if let Err(re) = fs::rename(&self.file_path, &bak) {
                    eprintln!("[schedule] FAILED to back up unreadable file: {re}");
                } else {
                    eprintln!("[schedule] unreadable file moved to {bak:?} ({e})");
                }
                self.items.clear();
                self.next_id = 1;
            }
        }
    }

    /// Menyimpan daftar ke berkas secara atomik (sementara lalu ganti nama).
    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.file_path.with_extension("json.tmp");
        fs::write(&tmp, serialize_schedule_json(&self.items))?;
        fs::rename(&tmp, &self.file_path)
    }
}

/// Lokasi penyimpanan bawaan `%APPDATA%\winisland\schedule.json`.
fn default_schedule_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata)
            .join("winisland")
            .join("schedule.json")
    } else {
        PathBuf::from("schedule.json")
    }
}

/// Menserialisasi daftar pengingat ke JSON satu-field-per-baris (format yang sama
/// dengan serializer Tasks; parser hanya mendukung bentuk ini).
pub fn serialize_schedule_json(items: &[ScheduleItem]) -> String {
    let mut json = String::from("[\n");
    for (i, it) in items.iter().enumerate() {
        json.push_str("  {\n");
        json.push_str(&format!("    \"id\": {},\n", it.id));
        json.push_str(&format!("    \"at\": {},\n", it.at));
        json.push_str(&format!("    \"label\": \"{}\"\n", json_escape(&it.label)));
        if i + 1 < items.len() {
            json.push_str("  },\n");
        } else {
            json.push_str("  }\n");
        }
    }
    json.push(']');
    json
}

/// Memarsing daftar pengingat dari string JSON; objek rusak dilewati.
pub fn parse_schedule_json(json: &str) -> Vec<ScheduleItem> {
    let mut items = Vec::new();
    let mut in_object = false;
    let mut in_string = false;
    let mut escaped = false;
    let mut current_obj = String::new();

    for c in json.chars() {
        if in_object && in_string {
            current_obj.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '{' {
            in_object = true;
            current_obj.clear();
        } else if c == '}' && in_object {
            in_object = false;
            if let Some(item) = parse_single_item(&current_obj) {
                items.push(item);
            }
        } else if in_object {
            if c == '"' {
                in_string = true;
            }
            current_obj.push(c);
        }
    }
    items
}

/// Memarsing satu objek pengingat; mengembalikan `None` bila field tidak lengkap.
fn parse_single_item(obj_str: &str) -> Option<ScheduleItem> {
    let mut id = None;
    let mut at = None;
    let mut label = None;

    for line in obj_str.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if let Some(rest) = trimmed.strip_prefix("\"id\":") {
            if let Ok(v) = rest.trim().parse::<u64>() {
                id = Some(v);
            }
        } else if let Some(rest) = trimmed.strip_prefix("\"at\":") {
            if let Ok(v) = rest.trim().parse::<u64>() {
                at = Some(v);
            }
        } else if let Some(rest) = trimmed.strip_prefix("\"label\":") {
            let val = rest.trim();
            if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                label = Some(json_unescape(&val[1..val.len() - 1]));
            }
        }
    }

    match (id, at, label) {
        (Some(id), Some(at), Some(label)) => Some(ScheduleItem { id, at, label }),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_parse_input_formats() {
        assert_eq!(
            parse_input("14:30 buy milk"),
            Some((14, 30, "buy milk".to_string()))
        );
        assert_eq!(parse_input("  09:05  "), Some((9, 5, String::new())));
        assert_eq!(parse_input("0:00 x"), None, "jam harus dua digit");
        assert_eq!(parse_input("24:00 x"), None, "jam maksimum 23");
        assert_eq!(parse_input("12:60 x"), None, "menit maksimum 59");
        assert_eq!(parse_input("abc x"), None);
        assert_eq!(parse_input(""), None);
    }

    #[test]
    fn test_format_hhmm_wraps_within_day() {
        let midnight = 1_000_000_000u64;
        assert_eq!(
            format_hhmm(midnight + 5 * 3600 + 30 * 60, midnight),
            "05:30"
        );
        // melewati tengah malam tetap menampilkan jam yang benar
        assert_eq!(
            format_hhmm(midnight + 26 * 3600, midnight),
            "02:00",
            "26 jam setelah tengah malam = 02:00 hari berikutnya"
        );
        assert_eq!(format_hhmm(midnight.saturating_sub(1), midnight), "00:00");
    }

    #[test]
    fn test_serialize_parse_roundtrip_with_special_chars() {
        let items = vec![
            ScheduleItem {
                id: 1,
                at: 1_792_000_000,
                label: "Review {draft} \"quote\" \\slash\\\nline 2".to_string(),
            },
            ScheduleItem {
                id: 2,
                at: 1_792_086_400,
                label: "Catatan {日本語}".to_string(),
            },
        ];
        let json = serialize_schedule_json(&items);
        assert_eq!(parse_schedule_json(&json), items);
    }

    #[test]
    fn test_parse_schedule_json_skips_broken_objects() {
        let json = "[\n  {\n    \"id\": 1\n  },\n  {\n    \"id\": 2,\n    \"at\": 100,\n    \"label\": \"ok\"\n  }\n]";
        let items = parse_schedule_json(json);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 2);
    }

    #[test]
    fn test_manager_crud_take_due_and_next() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_sched_{}.json", std::process::id()));
        let _ = fs::remove_file(&file_path);

        let mut mgr = ScheduleManager::with_path(file_path.clone());
        assert_eq!(mgr.items().len(), 0);
        assert!(mgr.add(0, 0, "").is_none(), "label kosong ditolak");

        // suntikkan langsung dua item lampau dan satu item masa depan
        let now = unix_now();
        mgr.items.push(ScheduleItem {
            id: 100,
            at: now.saturating_sub(20),
            label: "past one".to_string(),
        });
        mgr.items.push(ScheduleItem {
            id: 101,
            at: now.saturating_sub(10),
            label: "past two".to_string(),
        });
        mgr.items.push(ScheduleItem {
            id: 102,
            at: now + 3600,
            label: "future".to_string(),
        });
        assert_eq!(mgr.items().len(), 3);

        // hanya item lampau yang dianggap due, terurut paling lama dulu
        let due = mgr.take_due();
        assert_eq!(
            due.iter().map(|it| it.id).collect::<Vec<_>>(),
            vec![100, 101]
        );
        assert_eq!(mgr.items().len(), 1);

        // pengingat terdekat yang tersisa sekitar 1 jam dari sekarang
        let d = mgr.next_duration().expect("should have next");
        assert!(d >= Duration::from_secs(3600 - 5) && d <= Duration::from_secs(3600));

        // item terhapus tak bisa dihapus lagi
        assert!(!mgr.remove(100));
        assert!(mgr.remove(102));

        // persisten: muat ulang tetap kosong setelah take_due
        let reloaded = ScheduleManager::with_path(file_path.clone());
        assert_eq!(reloaded.items().len(), 0);

        let _ = fs::remove_file(&file_path);
    }

    #[test]
    fn test_load_corrupt_file_backs_up() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_sched_corrupt_{}.json", std::process::id()));
        let bak = file_path.with_extension("json.bak");
        let _ = fs::remove_file(&file_path);
        let _ = fs::remove_file(&bak);

        fs::write(&file_path, "[{\"id\": 1, \"label\": \"tengah").unwrap();

        let mut mgr = ScheduleManager::with_path(file_path.clone());
        assert_eq!(mgr.items().len(), 0);
        assert!(bak.exists(), "berkas lama harus dipindah ke .bak");

        // simpan berikutnya menulis berkas valid kembali
        mgr.add(1, 23, "setelah pemulihan").expect("should add");
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("setelah pemulihan"));

        let _ = fs::remove_file(&file_path);
        let _ = fs::remove_file(&bak);
    }
}
