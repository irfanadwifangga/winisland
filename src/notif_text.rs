//! Logika murni untuk notifikasi: pembersihan teks, pengurutan, dan pengelompokan.
//!
//! Tidak memakai API Windows sama sekali, jadi seluruh isinya bisa diuji dengan
//! `cargo test`. Pembacaan dari Windows ada di `notif.rs`.

/// Panjang maksimum nama aplikasi setelah dibersihkan, dalam karakter.
const APP_MAX: usize = 40;
/// Panjang maksimum judul setelah dibersihkan, dalam karakter.
const TITLE_MAX: usize = 80;
/// Panjang maksimum isi setelah dibersihkan, dalam karakter.
const BODY_MAX: usize = 160;

/// Satuan waktu `DateTime.UniversalTime`: 100 ns, jadi 10 juta per detik.
const TICKS_PER_SEC: i64 = 10_000_000;
/// Selisih 1601-01-01 (awal hitungan Windows) ke 1970-01-01 (epoch Unix), dalam satuan 100 ns.
const UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;

/// Satu notifikasi mentah hasil pembacaan, belum dibersihkan.
pub struct RawNotif {
    pub app: String,
    pub title: String,
    pub body: String,
    /// `DateTime.UniversalTime`: satuan 100 ns sejak 1601-01-01 UTC.
    pub time: i64,
}

/// Satu baris hasil pengelompokan notifikasi yang sama.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifItem {
    pub app: String,
    pub title: String,
    /// Kosong bila isi disembunyikan (pengaturan privasi bawaan).
    pub body: String,
    /// Jumlah notifikasi identik yang digabung ke baris ini.
    pub count: u32,
    /// Waktu notifikasi terbaru di kelompok ini (`UniversalTime`).
    pub time: i64,
}

/// Hasil satu kali pembacaan, siap ditampilkan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Kelompok terbaru lebih dulu, dibatasi `max_items`.
    pub items: Vec<NotifItem>,
    /// Jumlah notifikasi mentah di Action Center (sebelum pengelompokan).
    pub total: u32,
    /// Jumlah kelompok unik (sebelum dibatasi `max_items`).
    pub groups: usize,
}

/// Karakter tak terlihat yang dibuang. Sengaja TIDAK membuang U+200C dan U+200D
/// (ZWNJ dan ZWJ): keduanya dipakai di aksara tertentu dan di rangkaian emoji,
/// membuangnya mengubah arti teks.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'                 // soft hyphen
        | '\u{061C}'               // Arabic letter mark
        | '\u{200B}'               // zero width space
        | '\u{200E}' | '\u{200F}'  // tanda arah kiri/kanan (dipakai WhatsApp)
        | '\u{202A}'..='\u{202E}'  // kontrol bidi (embedding/override)
        | '\u{2060}'..='\u{2064}'  // word joiner dan operator tak terlihat
        | '\u{2066}'..='\u{2069}'  // isolat bidi
        | '\u{FEFF}'               // BOM / zero width no-break space
    )
}

/// Membuat teks aman untuk satu baris: membuang karakter tak terlihat, mengganti
/// newline/tab dan spasi beruntun dengan satu spasi, lalu memotong ke `max_chars`
/// karakter (diakhiri `…` bila terpotong).
pub fn clean(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    // batasi pekerjaan untuk masukan yang sangat panjang
    for c in s.chars().take(max_chars.saturating_mul(4)) {
        if is_invisible(c) {
            continue;
        }
        if c.is_whitespace() || c.is_control() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(c);
    }
    if out.chars().count() > max_chars {
        let keep = max_chars.saturating_sub(1);
        let mut cut: String = out.chars().take(keep).collect();
        while cut.ends_with(' ') {
            cut.pop();
        }
        cut.push('…');
        return cut;
    }
    out
}

/// Mengurutkan (terbaru dulu), membersihkan, dan menggabungkan notifikasi yang
/// tampil sama.
///
/// Judul yang sama dengan nama aplikasi (tanpa membedakan huruf besar/kecil ASCII)
/// tidak menambah informasi, jadi dikosongkan SEBELUM penggabungan. Tanpa aturan ini
/// tampilan menjadi "Antigravity | Antigravity".
///
/// Kunci penggabungan adalah apa yang benar-benar ditampilkan: (aplikasi, judul)
/// bila isi disembunyikan, (aplikasi, judul, isi) bila isi ditampilkan. Dengan isi
/// disembunyikan, beberapa pesan dari pengirim yang sama menjadi satu baris
/// dengan jumlahnya.
pub fn group(mut raw: Vec<RawNotif>, include_body: bool, max_items: usize) -> Snapshot {
    let total = raw.len() as u32;
    // sort_by stabil: urutan asli dipertahankan untuk waktu yang sama
    raw.sort_by(|a, b| b.time.cmp(&a.time));

    let mut items: Vec<NotifItem> = Vec::new();
    for r in raw {
        let mut app = clean(&r.app, APP_MAX);
        if app.is_empty() {
            app = "Unknown app".to_string();
        }
        let mut title = clean(&r.title, TITLE_MAX);
        // judul yang sama dengan nama aplikasi tidak menambah informasi: kosongkan
        if title.eq_ignore_ascii_case(&app) {
            title.clear();
        }
        let body = if include_body {
            clean(&r.body, BODY_MAX)
        } else {
            String::new()
        };
        if let Some(it) = items
            .iter_mut()
            .find(|i| i.app == app && i.title == title && i.body == body)
        {
            // entri lebih lama dari yang sudah ada: hanya menambah hitungan
            it.count += 1;
        } else {
            items.push(NotifItem {
                app,
                title,
                body,
                count: 1,
                time: r.time,
            });
        }
    }

    let groups = items.len();
    items.truncate(max_items);
    Snapshot {
        items,
        total,
        groups,
    }
}

/// Mengubah detik Unix menjadi satuan `UniversalTime` (100 ns sejak 1601).
pub fn ticks_from_unix_secs(secs: i64) -> i64 {
    secs.saturating_mul(TICKS_PER_SEC)
        .saturating_add(UNIX_EPOCH_TICKS)
}

/// Umur notifikasi dalam detik; tidak pernah negatif.
pub fn age_secs(time: i64, now_ticks: i64) -> i64 {
    (now_ticks.saturating_sub(time) / TICKS_PER_SEC).max(0)
}

/// Umur dalam teks singkat untuk UI: "just now", "5 min ago", "3 h ago", "2 d ago".
/// Nilai negatif diperlakukan sebagai 0.
pub fn format_age(secs: i64) -> String {
    match secs.max(0) {
        0..=59 => "just now".to_string(),
        s @ 60..=3599 => format!("{} min ago", s / 60),
        s @ 3600..=86_399 => format!("{} h ago", s / 3600),
        s => format!("{} d ago", s / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(app: &str, title: &str, body: &str, time: i64) -> RawNotif {
        RawNotif {
            app: app.into(),
            title: title.into(),
            body: body.into(),
            time,
        }
    }

    #[test]
    fn clean_collapses_whitespace_and_newlines() {
        assert_eq!(clean("  a \n\n b\t\tc  ", 80), "a b c");
    }

    #[test]
    fn clean_removes_invisible_marks_but_keeps_zwj() {
        assert_eq!(clean("\u{200E}Ima\u{200B}: halo\u{202E}", 80), "Ima: halo");
        // ZWJ dipertahankan supaya rangkaian emoji tidak terpecah
        assert_eq!(clean("a\u{200D}b", 80), "a\u{200D}b");
    }

    #[test]
    fn clean_truncates_with_ellipsis_by_chars() {
        assert_eq!(clean("abcdefghij", 5), "abcd…");
        // karakter multibyte dihitung per karakter, bukan per byte
        assert_eq!(clean("ありがとうございます", 4), "ありが…");
        assert_eq!(clean("abc", 5), "abc");
    }

    #[test]
    fn clean_does_not_leave_space_before_ellipsis() {
        assert_eq!(clean("abcd efgh", 5), "abcd…");
    }

    #[test]
    fn clean_empty_and_only_invisible() {
        assert_eq!(clean("", 10), "");
        assert_eq!(clean(" \n\u{200E} ", 10), "");
    }

    #[test]
    fn group_sorts_newest_first_and_counts_raw_total() {
        let s = group(
            vec![
                raw("A", "old", "", 10),
                raw("B", "new", "", 30),
                raw("C", "mid", "", 20),
            ],
            false,
            5,
        );
        let titles: Vec<&str> = s.items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["new", "mid", "old"]);
        assert_eq!(s.total, 3);
        assert_eq!(s.groups, 3);
    }

    #[test]
    fn group_merges_duplicates_and_keeps_newest_time() {
        let s = group(
            vec![
                raw("NVIDIA App", "Reward", "", 10),
                raw("NVIDIA App", "Reward", "", 50),
                raw("NVIDIA App", "Reward", "", 30),
            ],
            false,
            5,
        );
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0].count, 3);
        assert_eq!(s.items[0].time, 50);
        assert_eq!(s.total, 3);
    }

    #[test]
    fn group_hides_body_and_merges_same_title() {
        let rows = || {
            vec![
                raw("WhatsApp", "Budi", "pesan satu", 10),
                raw("WhatsApp", "Budi", "pesan dua", 20),
            ]
        };
        let hidden = group(rows(), false, 5);
        assert_eq!(hidden.items.len(), 1);
        assert_eq!(hidden.items[0].count, 2);
        assert_eq!(hidden.items[0].body, "");

        let shown = group(rows(), true, 5);
        assert_eq!(shown.items.len(), 2);
        assert_eq!(shown.items[0].body, "pesan dua"); // terbaru dulu
    }

    #[test]
    fn group_limits_items_but_reports_all_groups() {
        let rows = (0..8).map(|i| raw("A", &format!("t{i}"), "", i)).collect();
        let s = group(rows, false, 3);
        assert_eq!(s.items.len(), 3);
        assert_eq!(s.groups, 8);
        assert_eq!(s.total, 8);
        assert_eq!(s.items[0].title, "t7");
    }

    #[test]
    fn group_falls_back_when_app_name_is_empty() {
        let s = group(vec![raw("", "x", "", 1)], false, 5);
        assert_eq!(s.items[0].app, "Unknown app");
    }

    #[test]
    fn group_empty_input() {
        let s = group(Vec::new(), false, 5);
        assert!(s.items.is_empty());
        assert_eq!((s.total, s.groups), (0, 0));
    }

    #[test]
    fn age_is_never_negative_and_converts_ticks() {
        let now = ticks_from_unix_secs(1_000_000);
        assert_eq!(age_secs(ticks_from_unix_secs(999_940), now), 60);
        assert_eq!(age_secs(now + 5 * TICKS_PER_SEC, now), 0);
    }

    #[test]
    fn ticks_match_known_unix_epoch_offset() {
        assert_eq!(ticks_from_unix_secs(0), 116_444_736_000_000_000);
    }

    #[test]
    fn group_blanks_title_equal_to_app_name() {
        let s = group(vec![raw("Antigravity", "Antigravity", "isi", 1)], false, 5);
        assert_eq!(s.items[0].app, "Antigravity");
        assert_eq!(s.items[0].title, "");
    }

    #[test]
    fn group_blanks_title_ignoring_ascii_case_and_merges_after_blanking() {
        let s = group(
            vec![
                raw("Antigravity", "Antigravity", "", 10),
                raw("Antigravity", "antigravity", "", 20),
                raw("Antigravity", "  ANTIGRAVITY \n", "", 30),
            ],
            false,
            5,
        );
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0].count, 3);
        assert_eq!(s.items[0].title, "");
    }

    #[test]
    fn group_keeps_title_when_it_differs_from_app_name() {
        let s = group(vec![raw("WhatsApp", "Budi", "", 1)], false, 5);
        assert_eq!(s.items[0].title, "Budi");
    }

    #[test]
    fn format_age_picks_the_right_unit() {
        assert_eq!(format_age(-5), "just now");
        assert_eq!(format_age(0), "just now");
        assert_eq!(format_age(59), "just now");
        assert_eq!(format_age(60), "1 min ago");
        assert_eq!(format_age(3_599), "59 min ago");
        assert_eq!(format_age(3_600), "1 h ago");
        assert_eq!(format_age(86_399), "23 h ago");
        assert_eq!(format_age(86_400), "1 d ago");
        assert_eq!(format_age(3 * 86_400 + 5), "3 d ago");
    }
}
