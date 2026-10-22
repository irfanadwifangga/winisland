//! Modul pengelolaan daftar tugas ringkas (Quick Tasks / To-Do List) lokal.
//!
//! Seluruh data disimpan secara persisten dalam format JSON di `%APPDATA%\winisland\tasks.json`.
//! Dibuat secara mandiri tanpa dependensi crate eksternal tambahan (zero dependencies),
//! menjamin pemakaian memori minimal, kompilasi cepat, dan 0% CPU idle.

use std::fs;
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

        self.save();
        Some(id)
    }

    /// Mengubah status selesai/belum selesai dari tugas dengan ID tertentu.
    pub fn toggle_task(&mut self, id: u64) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.done = !task.done;
            self.save();
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
            self.save();
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
            self.save();
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

        if let Ok(content) = fs::read_to_string(&self.file_path) {
            let parsed = parse_tasks_json(&content);
            let max_id = parsed.iter().map(|t| t.id).max().unwrap_or(0);
            self.next_id = max_id + 1;
            self.tasks = parsed;
        }
    }

    /// Menyimpan daftar tugas ke file JSON di disk.
    pub fn save(&self) {
        if let Some(parent) = self.file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serialize_tasks_json(&self.tasks);
        let _ = fs::write(&self.file_path, json);
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

/// Melakukan escape karakter khusus untuk string JSON.
fn json_escape(s: &str) -> String {
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

/// Mengubah escape sequence JSON kembali ke string biasa.
fn json_unescape(s: &str) -> String {
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
    // Cari setiap blok objek { ... }
    let mut in_object = false;
    let mut current_obj = String::new();

    for c in json.chars() {
        if c == '{' {
            in_object = true;
            current_obj.clear();
        } else if c == '}' && in_object {
            in_object = false;
            if let Some(item) = parse_single_task_object(&current_obj) {
                tasks.push(item);
            }
        } else if in_object {
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
}
