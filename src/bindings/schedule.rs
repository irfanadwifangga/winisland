//! Penghubung daftar pengingat (Jadwal) dan callback dengan UI Slint.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::plugins::schedule;
use crate::{MainWindow, ScheduleRow};

/// Memperbarui daftar pengingat di UI, urut berdasarkan waktu target.
pub(crate) fn apply_schedules(ui: &MainWindow, mgr: &schedule::ScheduleManager) {
    let midnight = schedule::local_midnight_unix_public();
    let rows: Vec<ScheduleRow> = mgr
        .items()
        .iter()
        .map(|it| ScheduleRow {
            id: it.id as i32,
            time: schedule::format_hhmm(it.at, midnight).into(),
            label: it.label.as_str().into(),
        })
        .collect();
    ui.set_schedules(ModelRc::new(VecModel::from(rows)));
}

/// Memasang state dan callback pengingat. `on_change` dipanggil setelah daftar
/// berubah supaya pemanggil bisa memasang ulang timer pengingat terdekat.
pub(crate) fn register(
    ui: &MainWindow,
    mgr: Rc<RefCell<schedule::ScheduleManager>>,
    on_change: Rc<dyn Fn()>,
) {
    apply_schedules(ui, &mgr.borrow());

    {
        let weak = ui.as_weak();
        let m = mgr.clone();
        let on_change = on_change.clone();
        ui.on_schedule_add(move |input| {
            let Some(ui) = weak.upgrade() else { return };
            // format input: "HH:MM label"
            if let Some((h, min, label)) = schedule::parse_input(&input) {
                m.borrow_mut().add(h, min, &label);
                apply_schedules(&ui, &m.borrow());
                on_change();
            }
        });
    }

    {
        let weak = ui.as_weak();
        let m = mgr.clone();
        let on_change = on_change.clone();
        ui.on_schedule_delete(move |id| {
            let Some(ui) = weak.upgrade() else { return };
            m.borrow_mut().remove(id as u64);
            apply_schedules(&ui, &m.borrow());
            on_change();
        });
    }
}
