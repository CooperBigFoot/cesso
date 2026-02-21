//! Search control — stop flag and time management.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Controls when a search should stop.
///
/// Checked periodically by the search (every 2048 nodes) to decide
/// whether to abort. Supports three modes:
/// - **Infinite**: no time pressure, only responds to external stop flag
/// - **Timed**: clock starts immediately (normal `go wtime/btime`)
/// - **Ponder**: clock inactive until [`activate()`](SearchControl::activate) is called (`go ponder` -> `ponderhit`)
pub struct SearchControl {
    stopped: Arc<AtomicBool>,
    clock_active: AtomicBool,
    start: Mutex<Option<Instant>>,
    soft_limit: Option<Duration>,
    hard_limit: Option<Duration>,
}

impl SearchControl {
    /// Create control for `go infinite` or `go ponder` without time limits.
    pub fn new_infinite(stopped: Arc<AtomicBool>) -> Self {
        Self {
            stopped,
            clock_active: AtomicBool::new(false),
            start: Mutex::new(None),
            soft_limit: None,
            hard_limit: None,
        }
    }

    /// Create control with time limits; clock starts immediately.
    pub fn new_timed(stopped: Arc<AtomicBool>, soft: Duration, hard: Duration) -> Self {
        Self {
            stopped,
            clock_active: AtomicBool::new(true),
            start: Mutex::new(Some(Instant::now())),
            soft_limit: Some(soft),
            hard_limit: Some(hard),
        }
    }

    /// Create control for pondering — time limits exist but clock is inactive.
    ///
    /// Call [`activate()`](Self::activate) on `ponderhit` to start the clock.
    pub fn new_ponder(stopped: Arc<AtomicBool>, soft: Duration, hard: Duration) -> Self {
        Self {
            stopped,
            clock_active: AtomicBool::new(false),
            start: Mutex::new(None),
            soft_limit: Some(soft),
            hard_limit: Some(hard),
        }
    }

    /// Activate the clock (called on `ponderhit`).
    ///
    /// Records [`Instant::now()`] as the start time and enables time checks.
    pub fn activate(&self) {
        *self.start.lock().expect("start mutex poisoned") = Some(Instant::now());
        self.clock_active.store(true, Ordering::Release);
    }

    /// Check whether the search should abort immediately.
    ///
    /// Returns `true` if:
    /// - The external stop flag was set, OR
    /// - The clock is active and the hard limit has been exceeded
    ///   (checked only every 2048 nodes for performance)
    ///
    /// When the hard limit fires, the stop flag is set so subsequent
    /// calls return immediately without re-checking the clock.
    pub fn should_stop(&self, nodes: u64) -> bool {
        if self.stopped.load(Ordering::Relaxed) {
            return true;
        }

        // Only check the clock every 2048 nodes
        if nodes & 2047 != 0 {
            return false;
        }

        if !self.clock_active.load(Ordering::Acquire) {
            return false;
        }

        if let Some(hard) = self.hard_limit
            && self.elapsed() >= hard
        {
            self.stopped.store(true, Ordering::Release);
            return true;
        }

        false
    }

    /// Check whether iterative deepening should start a new iteration.
    ///
    /// Called between ID iterations. Returns `true` if the soft limit
    /// has been exceeded (meaning we likely don't have time for another
    /// full iteration).
    pub fn should_stop_iterating(&self) -> bool {
        if self.stopped.load(Ordering::Relaxed) {
            return true;
        }

        if !self.clock_active.load(Ordering::Acquire) {
            return false;
        }

        if let Some(soft) = self.soft_limit {
            return self.elapsed() >= soft;
        }

        false
    }

    /// Elapsed time since the clock was activated.
    ///
    /// Returns [`Duration::ZERO`] if the clock has not been activated.
    pub fn elapsed(&self) -> Duration {
        self.start
            .lock()
            .expect("start mutex poisoned")
            .map_or(Duration::ZERO, |s| s.elapsed())
    }

    /// Reference to the shared stop flag.
    pub fn stop_flag(&self) -> &Arc<AtomicBool> {
        &self.stopped
    }
}
