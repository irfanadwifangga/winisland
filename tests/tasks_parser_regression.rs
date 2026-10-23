#[allow(dead_code)]
#[path = "../src/plugins/tasks.rs"]
mod tasks;

use tasks::{parse_tasks_json, serialize_tasks_json, TaskItem};

#[test]
fn task_text_braces_survive_json_roundtrip() {
    for text in [
        "Review {draft}",
        "{",
        "}",
        "{} {{nested}}",
        "Review \"{draft}\"",
        "Review \\\"{draft}\\\"",
        "C:\\notes\\{draft}\\",
        "Review } before { with \"quotes\" and \\slashes\\",
        "Catatan {日本語} café\nbaris kedua",
    ] {
        let expected = vec![
            TaskItem {
                id: 1,
                text: text.to_string(),
                done: false,
            },
            TaskItem {
                id: 2,
                text: "Keep the next task".to_string(),
                done: true,
            },
        ];

        let json = serialize_tasks_json(&expected);
        assert_eq!(parse_tasks_json(&json), expected, "teks tugas: {text:?}");
    }
}
