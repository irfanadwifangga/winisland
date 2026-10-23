//! Modul pengelolaan daftar tugas ringkas (Quick Tasks / To-Do List) lokal.
//!
//! Seluruh data disimpan secara persisten dalam format JSON di `%APPDATA%\winisland\tasks.json`.
//! Dibuat secara mandiri tanpa dependensi crate eksternal tambahan (zero dependencies),
//! menjamin pemakaian memori minimal, kompilasi cepat, dan 0% CPU idle.

use std::fs;
use std::io;
use std::path::PathBuf;

/// Satu butir tugas dalam daftar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskItem {
    pub id: u64,
    pub text: String,
    pub done: bool,
}

/// Pengelola daftar tugas lokal.
#[derive(Debug)]
pub struct TaskManager {
    tasks: Vec<TaskItem>,
    next_id: u64,
    file_path: PathBuf,
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskManager {
    /// Membuat `TaskManager` baru dengan lokasi file default di AppData.
    pub fn new() -> Self {
        let path = default_tasks_path();
        Self::with_path(path)
    }

    /// Membuat `TaskManager` dengan path file tertentu (berguna untuk pengujian unit).
    pub fn with_path(file_path: PathBuf) -> Self {
        let mut mgr = Self {
            tasks: Vec::new(),
            next_id: 1,
            file_path,
        };
        mgr.load();
        mgr
    }

    /// Mengembalikan referensi slice seluruh tugas.
    pub fn tasks(&self) -> &[TaskItem] {
        &self.tasks
    }

    /// Menghitung jumlah tugas yang belum selesai.
    pub fn remaining_count(&self) -> usize {
        self.tasks.iter().filter(|t| !t.done).count()
    }

    /// Menghitung jumlah tugas yang sudah selesai.
    pub fn completed_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.done).count()
    }

    /// Menambahkan tugas baru. Mengembalikan ID tugas baru jika berhasil, atau None jika teks kosong.
    pub fn add_task(&mut self, text: &str) -> Option<u64> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }

        let id = self.next_id;
        self.next_id += 1;

        self.tasks.push(TaskItem {
            id,
            text: trimmed.to_string(),
            done: false,
        });

        if let Err(e) = self.save() {
            eprintln!("[tasks] FAILED to save: {e}");
        }
        Some(id)
    }

    /// Mengubah status selesai/belum selesai dari tugas dengan ID tertentu.
    pub fn toggle_task(&mut self, id: u64) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.done = !task.done;
            if let Err(e) = self.save() {
                eprintln!("[tasks] FAILED to save: {e}");
            }
            true
        } else {
            false
        }
    }

    /// Menghapus tugas dengan ID tertentu.
    pub fn remove_task(&mut self, id: u64) -> bool {
        let before_len = self.tasks.len();
        self.tasks.retain(|t| t.id != id);
        if self.tasks.len() != before_len {
            if let Err(e) = self.save() {
                eprintln!("[tasks] FAILED to save: {e}");
            }
            true
        } else {
            false
        }
    }

    /// Menghapus semua tugas yang berstatus sudah selesai.
    pub fn clear_completed(&mut self) -> usize {
        let before_len = self.tasks.len();
        self.tasks.retain(|t| !t.done);
        let removed = before_len - self.tasks.len();
        if removed > 0 {
            if let Err(e) = self.save() {
                eprintln!("[tasks] FAILED to save: {e}");
            }
        }
        removed
    }

    /// Memuat daftar tugas dari file JSON di disk.
    pub fn load(&mut self) {
        if !self.file_path.exists() {
            // Jika file belum ada, mulai dengan daftar kosong
            self.tasks.clear();
            self.next_id = 1;
            return;
        }

        match fs::read_to_string(&self.file_path) {
            Ok(content) => {
                let parsed = parse_tasks_json(&content);
                if parsed.is_empty() && content.trim() != "[]" {
                    // Berkas terbaca tetapi tidak menghasilkan satu pun tugas:
                    // hampir pasti korup atau terpotong. Pindah ke .bak supaya
                    // simpanan berikutnya tidak menimpa data lama.
                    let bak = self.file_path.with_extension("json.bak");
                    if let Err(re) = fs::rename(&self.file_path, &bak) {
                        eprintln!("[tasks] FAILED to back up unreadable file: {re}");
                    } else {
                        eprintln!("[tasks] unreadable file moved to {bak:?}");
                    }
                    self.tasks.clear();
                    self.next_id = 1;
                } else {
                    // ID di UI Slint bertipe i32: butir dengan ID di luar rentang itu
                    // dilewatkan agar pemetaan ID UI tetap valid dan next_id tidak meluber.
                    self.tasks = parsed
                        .into_iter()
                        .filter(|t| t.id <= i32::MAX as u64)
                        .collect();
                    let max_id = self.tasks.iter().map(|t| t.id).max().unwrap_or(0);
                    self.next_id = max_id.saturating_add(1).min(i32::MAX as u64);
                }
            }
            Err(e) => {
                // Berkas rusak atau tidak terbaca: jangan ditimpa diam-diam.
                // Pindah ke .bak supaya data lama masih bisa dipulihkan manual.
                let bak = self.file_path.with_extension("json.bak");
                if let Err(re) = fs::rename(&self.file_path, &bak) {
                    eprintln!("[tasks] FAILED to back up unreadable file: {re}");
                } else {
                    eprintln!("[tasks] unreadable file moved to {bak:?} ({e})");
                }
                self.tasks.clear();
                self.next_id = 1;
            }
        }
    }

    /// Menyimpan daftar tugas ke file JSON di disk.
    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serialize_tasks_json(&self.tasks);
        // Tulis ke berkas sementara lalu ganti nama: proses mati di tengah menulis
        // tidak meninggalkan tasks.json yang terpotong. `fs::rename` menggantikan
        // berkas tujuan yang sudah ada, termasuk di Windows.
        let tmp = self.file_path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.file_path)
    }
}

/// Menentukan lokasi penyimpanan default `%APPDATA%\winisland\tasks.json`.
fn default_tasks_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("winisland").join("tasks.json")
    } else {
        PathBuf::from("tasks.json")
    }
}

/// Melakukan escape karakter khusus untuk string JSON. Dipakai juga modul schedule.
pub(crate) fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Mengubah escape sequence JSON kembali ke string biasa. Dipakai juga modul schedule.
pub(crate) fn json_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Menserialisasi daftar tugas ke format JSON.
pub fn serialize_tasks_json(tasks: &[TaskItem]) -> String {
    let mut json = String::from("[\n");
    for (i, task) in tasks.iter().enumerate() {
        json.push_str("  {\n");
        json.push_str(&format!("    \"id\": {},\n", task.id));
        json.push_str(&format!("    \"text\": \"{}\",\n", json_escape(&task.text)));
        json.push_str(&format!("    \"done\": {}\n", task.done));
        if i + 1 < tasks.len() {
            json.push_str("  },\n");
        } else {
            json.push_str("  }\n");
        }
    }
    json.push(']');
    json
}

/// Memparsing daftar tugas dari string JSON sederhana.
pub fn parse_tasks_json(json: &str) -> Vec<TaskItem> {
    let mut tasks = Vec::new();
    // Kurung kurawal di dalam string bukan batas objek JSON.
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
            if let Some(item) = parse_single_task_object(&current_obj) {
                tasks.push(item);
            }
        } else if in_object {
            if c == '"' {
                in_string = true;
            }
            current_obj.push(c);
        }
    }

    tasks
}

/// Memparsing nilai dari satu objek tugas JSON.
fn parse_single_task_object(obj_str: &str) -> Option<TaskItem> {
    let mut id = None;
    let mut text = None;
    let mut done = None;

    for line in obj_str.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if let Some(rest) = trimmed.strip_prefix("\"id\":") {
            if let Ok(parsed_id) = rest.trim().parse::<u64>() {
                id = Some(parsed_id);
            }
        } else if let Some(rest) = trimmed.strip_prefix("\"text\":") {
            let val = rest.trim();
            if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                let inner = &val[1..val.len() - 1];
                text = Some(json_unescape(inner));
            }
        } else if let Some(rest) = trimmed.strip_prefix("\"done\":") {
            let val = rest.trim();
            if val == "true" {
                done = Some(true);
            } else if val == "false" {
                done = Some(false);
            }
        }
    }

    match (id, text, done) {
        (Some(id), Some(text), Some(done)) => Some(TaskItem { id, text, done }),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_json_escape_and_unescape() {
        let original = "Hello \"World\" \\\n\r\t!";
        let escaped = json_escape(original);
        let unescaped = json_unescape(&escaped);
        assert_eq!(original, unescaped);
    }

    #[test]
    fn test_serialize_and_parse_roundtrip() {
        let tasks = vec![
            TaskItem {
                id: 1,
                text: "Buy milk & cookies".to_string(),
                done: false,
            },
            TaskItem {
                id: 2,
                text: "Finish Step 17 \"Tasks\" tab".to_string(),
                done: true,
            },
        ];

        let json = serialize_tasks_json(&tasks);
        let parsed = parse_tasks_json(&json);
        assert_eq!(tasks, parsed);
    }

    #[test]
    fn test_task_manager_crud_operations() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_tasks_{}.json", std::process::id()));

        // Pastikan bersih sebelum uji
        let _ = fs::remove_file(&file_path);

        let mut mgr = TaskManager::with_path(file_path.clone());
        assert_eq!(mgr.tasks().len(), 0);
        assert_eq!(mgr.remaining_count(), 0);

        // Tambah task
        let id1 = mgr.add_task("Review pull request").expect("should add");
        let id2 = mgr.add_task("Write unit tests").expect("should add");
        assert_eq!(mgr.tasks().len(), 2);
        assert_eq!(mgr.remaining_count(), 2);
        assert_eq!(mgr.completed_count(), 0);

        // Penolakan teks kosong
        assert!(mgr.add_task("   ").is_none());

        // Toggle task
        assert!(mgr.toggle_task(id1));
        assert_eq!(mgr.remaining_count(), 1);
        assert_eq!(mgr.completed_count(), 1);

        // Muat ulang dari disk untuk memverifikasi persistensi
        let mut mgr_reloaded = TaskManager::with_path(file_path.clone());
        assert_eq!(mgr_reloaded.tasks().len(), 2);
        assert!(
            mgr_reloaded
                .tasks()
                .iter()
                .find(|t| t.id == id1)
                .unwrap()
                .done
        );

        // Hapus task
        assert!(mgr_reloaded.remove_task(id2));
        assert_eq!(mgr_reloaded.tasks().len(), 1);

        // Clear completed
        let removed = mgr_reloaded.clear_completed();
        assert_eq!(removed, 1);
        assert_eq!(mgr_reloaded.tasks().len(), 0);

        // Bersihkan berkas uji
        let _ = fs::remove_file(&file_path);
    }

    #[test]
    fn test_load_corrupt_file_backs_up_instead_of_overwriting() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_tasks_corrupt_{}.json", std::process::id()));
        let bak = file_path.with_extension("json.bak");
        let _ = fs::remove_file(&file_path);
        let _ = fs::remove_file(&bak);

        // berkas sengaja tidak lengkap (proses mati saat menulis, misalnya)
        fs::write(&file_path, "[{\"id\": 1, \"text\": \"tengah").unwrap();

        let mut mgr = TaskManager::with_path(file_path.clone());
        assert_eq!(mgr.tasks().len(), 0);
        assert!(
            bak.exists(),
            "berkas lama harus dipindah ke .bak, bukan ditimpa"
        );

        // simpan berikutnya menulis tasks.json yang valid kembali
        let id = mgr.add_task("setelah pemulihan").expect("should add");
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("setelah pemulihan"));
        assert_eq!(id, 1);

        let _ = fs::remove_file(&file_path);
        let _ = fs::remove_file(&bak);
    }

    #[test]
    fn test_save_failure_is_reported_not_silent() {
        let temp_dir = std::env::temp_dir();
        // parent "folder" sengaja berupa berkas: create_dir_all pasti gagal
        let blocker = temp_dir.join(format!("test_tasks_blocker_{}.json", std::process::id()));
        let _ = fs::remove_file(&blocker);
        fs::write(&blocker, "penanda").unwrap();
        let file_path = blocker.join("tasks.json");

        let mut mgr = TaskManager::with_path(file_path);
        let id = mgr.add_task("tidak tersimpan");
        assert!(id.is_some(), "data tetap ada di memori walau simpan gagal");
        assert!(
            mgr.save().is_err(),
            "kegagalan penyimpanan harus terlihat sebagai error"
        );

        let _ = fs::remove_file(&blocker);
    }

    #[test]
    fn test_load_skips_ids_beyond_slint_i32_range() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_tasks_bigid_{}.json", std::process::id()));
        let _ = fs::remove_file(&file_path);

        fs::write(
            &file_path,
            // format satu-field-per-baris sesuai output serialize_tasks_json
            r#"[
  {
    "id": 99999999999,
    "text": "di luar rentang",
    "done": false
  },
  {
    "id": 5,
    "text": "masuk",
    "done": true
  }
]"#,
        )
        .unwrap();

        let mut mgr = TaskManager::with_path(file_path.clone());
        assert_eq!(mgr.tasks().len(), 1);
        assert_eq!(mgr.tasks()[0].id, 5);
        // ID baru lanjut dari butir yang valid, bukan dari ID raksasa
        let id = mgr.add_task("baru").expect("should add");
        assert_eq!(id, 6);

        let _ = fs::remove_file(&file_path);
    }
}
