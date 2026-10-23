//! Penghubung daftar tugas lokal dan callback dengan UI Slint.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::plugins::tasks;
use crate::{MainWindow, TaskRow};

/// Memperbarui daftar tugas dan ringkasan jumlah tugas di UI.
fn apply_tasks(ui: &MainWindow, mgr: &tasks::TaskManager) {
    let rows: Vec<TaskRow> = mgr
        .tasks()
        .iter()
        .map(|t| TaskRow {
            id: t.id as i32,
            text: t.text.as_str().into(),
            done: t.done,
        })
        .collect();
    ui.set_tasks(ModelRc::new(VecModel::from(rows)));
    ui.set_tasks_remaining(mgr.remaining_count() as i32);
    ui.set_tasks_completed(mgr.completed_count() as i32);
}

pub(crate) fn register(ui: &MainWindow) {
    // --- tasks (Langkah 17) ---
    let task_mgr = Rc::new(RefCell::new(tasks::TaskManager::new()));
    apply_tasks(&ui, &task_mgr.borrow());

    {
        let weak = ui.as_weak();
        let tm = task_mgr.clone();
        ui.on_task_add(move |text| {
            let Some(ui) = weak.upgrade() else { return };
            tm.borrow_mut().add_task(&text);
            apply_tasks(&ui, &tm.borrow());
        });
    }

    {
        let weak = ui.as_weak();
        let tm = task_mgr.clone();
        ui.on_task_toggle(move |id| {
            let Some(ui) = weak.upgrade() else { return };
            tm.borrow_mut().toggle_task(id as u64);
            apply_tasks(&ui, &tm.borrow());
        });
    }

    {
        let weak = ui.as_weak();
        let tm = task_mgr.clone();
        ui.on_task_delete(move |id| {
            let Some(ui) = weak.upgrade() else { return };
            tm.borrow_mut().remove_task(id as u64);
            apply_tasks(&ui, &tm.borrow());
        });
    }

    {
        let weak = ui.as_weak();
        let tm = task_mgr.clone();
        ui.on_task_clear_completed(move || {
            let Some(ui) = weak.upgrade() else { return };
            tm.borrow_mut().clear_completed();
            apply_tasks(&ui, &tm.borrow());
        });
    }
}
