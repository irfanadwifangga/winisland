//! Modul Timer (Countdown) dan Stopwatch lokal dengan presisi berbasis `Instant`.
//!
//! Menggunakan kalkulasi selisih `std::time::Instant` tanpa drift dan tanpa polling
//! saat overlay dalam keadaan collapsed (menjaga CPU idle 0%).

use std::time::{Duration, Instant};

/// Status mesin waktu untuk Stopwatch (Count-up).
#[derive(Debug, Clone)]
pub enum StopwatchState {
    /// Berhenti di 0.
    Stopped,
    /// Sedang berjalan sejak `start_time` dengan akumulasi waktu sebelumnya.
    Running {
        start_time: Instant,
        accumulated: Duration,
    },
    /// Dijeda pada akumulasi waktu tertentu.
    Paused { accumulated: Duration },
}

/// Pengelola stopwatch lokal.
#[derive(Debug, Clone)]
pub struct Stopwatch {
    state: StopwatchState,
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}

impl Stopwatch {
    pub fn new() -> Self {
        Self {
            state: StopwatchState::Stopped,
        }
    }

    /// True jika stopwatch sedang berjalan aktif.
    pub fn is_running(&self) -> bool {
        matches!(self.state, StopwatchState::Running { .. })
    }

    /// True jika stopwatch dalam keadaan dijeda (memiliki waktu > 0).
    pub fn is_paused(&self) -> bool {
        matches!(self.state, StopwatchState::Paused { .. })
    }

    /// Memulai atau melanjutkan stopwatch.
    pub fn start(&mut self) {
        match self.state {
            StopwatchState::Stopped => {
                self.state = StopwatchState::Running {
                    start_time: Instant::now(),
                    accumulated: Duration::ZERO,
                };
            }
            StopwatchState::Paused { accumulated } => {
                self.state = StopwatchState::Running {
                    start_time: Instant::now(),
                    accumulated,
                };
            }
            StopwatchState::Running { .. } => {}
        }
    }

    /// Menjeda stopwatch.
    pub fn pause(&mut self) {
        if let StopwatchState::Running {
            start_time,
            accumulated,
        } = self.state
        {
            let elapsed = accumulated + start_time.elapsed();
            self.state = StopwatchState::Paused {
                accumulated: elapsed,
            };
        }
    }

    /// Mereset stopwatch kembali ke 0.
    pub fn reset(&mut self) {
        self.state = StopwatchState::Stopped;
    }

    /// Menghitung total durasi waktu yang telah berjalan.
    pub fn elapsed(&self) -> Duration {
        match self.state {
            StopwatchState::Stopped => Duration::ZERO,
            StopwatchState::Paused { accumulated } => accumulated,
            StopwatchState::Running {
                start_time,
                accumulated,
            } => accumulated + start_time.elapsed(),
        }
    }

    /// Memformat durasi ke teks "MM:SS.cs" (menit, detik, seperseratus detik).
    pub fn format_time(&self) -> String {
        format_stopwatch_duration(self.elapsed())
    }
}

/// Format durasi stopwatch menjadi format "MM:SS.cs".
pub fn format_stopwatch_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    let centis = d.subsec_millis() / 10;
    format!("{:02}:{:02}.{:02}", mins, secs, centis)
}

/// Status mesin waktu untuk Timer (Countdown).
#[derive(Debug, Clone)]
pub enum TimerState {
    /// Diam menunggu dimulai, durasi telah disetel.
    Idle { duration: Duration },
    /// Sedang menghitung mundur menuju `target_time`.
    Running {
        target_time: Instant,
        total_duration: Duration,
    },
    /// Dijeda dengan sisa durasi tertentu.
    Paused {
        remaining: Duration,
        total_duration: Duration,
    },
    /// Waktu habis (mode alarm).
    Finished { total_duration: Duration },
}

/// Pengelola timer countdown lokal.
#[derive(Debug, Clone)]
pub struct CountdownTimer {
    state: TimerState,
    default_duration: Duration,
}

impl Default for CountdownTimer {
    fn default() -> Self {
        Self::new(Duration::from_secs(300)) // Bawaan: 5 menit
    }
}

impl CountdownTimer {
    pub fn new(initial_duration: Duration) -> Self {
        Self {
            state: TimerState::Idle {
                duration: initial_duration,
            },
            default_duration: initial_duration,
        }
    }

    /// True jika timer sedang menghitung mundur.
    pub fn is_running(&self) -> bool {
        matches!(self.state, TimerState::Running { .. })
    }

    /// True jika timer sedang dijeda.
    pub fn is_paused(&self) -> bool {
        matches!(self.state, TimerState::Paused { .. })
    }

    /// True jika hitungan mundur sudah selesai (alarm).
    pub fn is_finished(&self) -> bool {
        matches!(self.state, TimerState::Finished { .. })
    }

    /// Mengatur durasi timer baru (hanya jika sedang tidak berjalan aktif).
    pub fn set_duration(&mut self, duration: Duration) {
        if !self.is_running() {
            let clamped = duration.clamp(Duration::from_secs(10), Duration::from_secs(99 * 60));
            self.default_duration = clamped;
            self.state = TimerState::Idle { duration: clamped };
        }
    }

    /// Menambah atau mengurangi menit pada timer.
    pub fn adjust_minutes(&mut self, delta_minutes: i32) {
        if !self.is_running() {
            let current_secs = self.default_duration.as_secs() as i64;
            let new_secs = (current_secs + (delta_minutes as i64 * 60)).clamp(60, 99 * 60) as u64;
            self.set_duration(Duration::from_secs(new_secs));
        }
    }

    /// Memulai countdown.
    pub fn start(&mut self) {
        match self.state {
            TimerState::Idle { duration }
            | TimerState::Finished {
                total_duration: duration,
            } => {
                self.state = TimerState::Running {
                    target_time: Instant::now() + duration,
                    total_duration: duration,
                };
            }
            TimerState::Paused {
                remaining,
                total_duration,
            } => {
                self.state = TimerState::Running {
                    target_time: Instant::now() + remaining,
                    total_duration,
                };
            }
            TimerState::Running { .. } => {}
        }
    }

    /// Menjeda countdown.
    pub fn pause(&mut self) {
        if let TimerState::Running {
            target_time,
            total_duration,
        } = self.state
        {
            let now = Instant::now();
            if now >= target_time {
                self.state = TimerState::Finished { total_duration };
            } else {
                let remaining = target_time.saturating_duration_since(now);
                self.state = TimerState::Paused {
                    remaining,
                    total_duration,
                };
            }
        }
    }

    /// Mereset timer kembali ke durasi awal.
    pub fn reset(&mut self) {
        self.state = TimerState::Idle {
            duration: self.default_duration,
        };
    }

    /// Memperbarui status selesai bila target waktu telah tercapai.
    pub fn check_finished(&mut self) -> bool {
        if let TimerState::Running {
            target_time,
            total_duration,
        } = self.state
        {
            if Instant::now() >= target_time {
                self.state = TimerState::Finished { total_duration };
                return true;
            }
        }
        self.is_finished()
    }

    /// Mengambil sisa waktu countdown saat ini.
    pub fn remaining(&self) -> Duration {
        match self.state {
            TimerState::Idle { duration } => duration,
            TimerState::Running { target_time, .. } => {
                target_time.saturating_duration_since(Instant::now())
            }
            TimerState::Paused { remaining, .. } => remaining,
            TimerState::Finished { .. } => Duration::ZERO,
        }
    }

    /// Menghitung persentase sisa waktu (0.0 sampai 1.0) untuk progress bar.
    pub fn progress(&self) -> f32 {
        match self.state {
            TimerState::Idle { .. } => 1.0,
            TimerState::Finished { .. } => 0.0,
            TimerState::Running {
                target_time,
                total_duration,
            } => {
                if total_duration.is_zero() {
                    return 0.0;
                }
                let rem = target_time
                    .saturating_duration_since(Instant::now())
                    .as_secs_f32();
                (rem / total_duration.as_secs_f32()).clamp(0.0, 1.0)
            }
            TimerState::Paused {
                remaining,
                total_duration,
            } => {
                if total_duration.is_zero() {
                    return 0.0;
                }
                (remaining.as_secs_f32() / total_duration.as_secs_f32()).clamp(0.0, 1.0)
            }
        }
    }

    /// Format sisa durasi menjadi "MM:SS".
    pub fn format_time(&self) -> String {
        format_countdown_duration(self.remaining())
    }
}

/// Format sisa durasi countdown menjadi format "MM:SS".
pub fn format_countdown_duration(d: Duration) -> String {
    let total_secs = d.as_secs() + if d.subsec_nanos() > 0 { 1 } else { 0 };
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_stopwatch_duration() {
        let d = Duration::from_millis(65_430); // 1 min 5 sec 430 ms -> 01:05.43
        assert_eq!(format_stopwatch_duration(d), "01:05.43");

        let d0 = Duration::ZERO;
        assert_eq!(format_stopwatch_duration(d0), "00:00.00");
    }

    #[test]
    fn test_format_countdown_duration() {
        let d = Duration::from_secs(300); // 5 menit
        assert_eq!(format_countdown_duration(d), "05:00");

        let d2 = Duration::from_secs(59);
        assert_eq!(format_countdown_duration(d2), "00:59");
    }

    #[test]
    fn test_stopwatch_flow() {
        let mut sw = Stopwatch::new();
        assert!(!sw.is_running());
        assert!(!sw.is_paused());
        assert_eq!(sw.elapsed(), Duration::ZERO);

        sw.start();
        assert!(sw.is_running());

        sw.pause();
        assert!(!sw.is_running());
        assert!(sw.is_paused());

        sw.reset();
        assert!(!sw.is_running());
        assert!(!sw.is_paused());
        assert_eq!(sw.elapsed(), Duration::ZERO);
    }

    #[test]
    fn test_timer_presets_and_adjustments() {
        let mut timer = CountdownTimer::new(Duration::from_secs(60));
        assert_eq!(timer.remaining(), Duration::from_secs(60));

        timer.adjust_minutes(4); // +4 menit -> 5 menit (300 detik)
        assert_eq!(timer.remaining(), Duration::from_secs(300));

        timer.adjust_minutes(-2); // -2 menit -> 3 menit (180 detik)
        assert_eq!(timer.remaining(), Duration::from_secs(180));

        timer.set_duration(Duration::from_secs(1500)); // 25 menit (Pomodoro)
        assert_eq!(timer.remaining(), Duration::from_secs(1500));
    }

    #[test]
    fn test_timer_flow_and_pause() {
        let mut timer = CountdownTimer::new(Duration::from_secs(120));
        timer.start();
        assert!(timer.is_running());

        timer.pause();
        assert!(timer.is_paused());
        assert!(!timer.is_running());

        timer.reset();
        assert!(!timer.is_running());
        assert!(!timer.is_paused());
        assert_eq!(timer.remaining(), Duration::from_secs(120));
    }
}
