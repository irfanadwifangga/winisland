//! Penghubung state dan callback kalender dengan UI Slint.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::plugins::calendar;
use crate::{CalendarCell, MainWindow};

/// Memperbarui properti kalender di UI dari CalendarState.
fn apply_calendar(ui: &MainWindow, state: &calendar::CalendarState) {
    ui.set_cal_title(calendar::format_month_title(state.view_year, state.view_month).into());
    let grid = state.generate_grid();
    let cells: Vec<CalendarCell> = grid
        .into_iter()
        .map(|cell| CalendarCell {
            text: cell.text.into(),
            is_current_month: cell.is_current_month,
            is_today: cell.is_today,
        })
        .collect();
    ui.set_cal_cells(ModelRc::new(VecModel::from(cells)));
}

pub(crate) fn register(ui: &MainWindow) {
    // --- kalender (Langkah 14) ---
    let cal_state = Rc::new(RefCell::new(calendar::CalendarState::new()));
    apply_calendar(&ui, &cal_state.borrow());
    {
        let weak = ui.as_weak();
        let cal = cal_state.clone();
        ui.on_cal_prev(move || {
            let Some(ui) = weak.upgrade() else { return };
            cal.borrow_mut().prev_month();
            apply_calendar(&ui, &cal.borrow());
        });
    }
    {
        let weak = ui.as_weak();
        let cal = cal_state.clone();
        ui.on_cal_next(move || {
            let Some(ui) = weak.upgrade() else { return };
            cal.borrow_mut().next_month();
            apply_calendar(&ui, &cal.borrow());
        });
    }
    {
        let weak = ui.as_weak();
        let cal = cal_state.clone();
        ui.on_cal_today(move || {
            let Some(ui) = weak.upgrade() else { return };
            cal.borrow_mut().reset_to_today();
            apply_calendar(&ui, &cal.borrow());
        });
    }
}
