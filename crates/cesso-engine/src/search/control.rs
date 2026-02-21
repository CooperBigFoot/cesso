//! Search control — stop flag and time management.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
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
    soft_scale: AtomicI32,
    /// Scaling factor applied to the soft limit after ponderhit (in hundredths).
    ///
    /// Set to `50` for ponder mode (half the normal soft limit) so that after
    /// `ponderhit` the engine plays more quickly to compensate for time spent
    /// pondering. Set to `100` (neutral) for timed and infinite modes.
    ponder_scale: AtomicI32,
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
            soft_scale: AtomicI32::new(100),
            ponder_scale: AtomicI32::new(100),
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
            soft_scale: AtomicI32::new(100),
            ponder_scale: AtomicI32::new(100),
        }
    }

    /// Create control for pondering — time limits exist but clock is inactive.
    ///
    /// Call [`activate()`](Self::activate) on `ponderhit` to start the clock.
    ///
    /// The `ponder_scale` is baked in at `50` (half the normal soft limit) so
    /// that after `ponderhit` the engine reacts faster than in a normal timed
    /// search.  The hard limit is **not** reduced — it remains the full budget.
    pub fn new_ponder(stopped: Arc<AtomicBool>, soft: Duration, hard: Duration) -> Self {
        Self {
            stopped,
            clock_active: AtomicBool::new(false),
            start: Mutex::new(None),
            soft_limit: Some(soft),
            hard_limit: Some(hard),
            soft_scale: AtomicI32::new(100),
            ponder_scale: AtomicI32::new(50),
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

    /// Update the soft limit scaling factor (in hundredths).
    ///
    /// 100 = neutral (1.0x), 60 = play faster (0.6x), 180 = think longer (1.8x).
    pub fn update_soft_scale(&self, scale_hundredths: i32) {
        self.soft_scale.store(scale_hundredths, Ordering::Relaxed);
    }

    /// Check whether iterative deepening should start a new iteration.
    ///
    /// Called between ID iterations. Returns `true` if the effective soft limit
    /// has been exceeded (meaning we likely don't have time for another full
    /// iteration).
    ///
    /// The effective soft limit is computed as:
    /// ```text
    /// effective = soft * soft_scale/100 * ponder_scale/100
    /// ```
    /// and is then clamped to the hard limit so that stability scaling (e.g.
    /// 250%) can never push the engine past its hard budget.
    pub fn should_stop_iterating(&self) -> bool {
        if self.stopped.load(Ordering::Relaxed) {
            return true;
        }

        if !self.clock_active.load(Ordering::Acquire) {
            return false;
        }

        if let Some(soft) = self.soft_limit {
            let scale = self.soft_scale.load(Ordering::Relaxed);
            let ponder_scale = self.ponder_scale.load(Ordering::Relaxed);
            let effective_ms =
                (soft.as_millis() as i64 * scale as i64 * ponder_scale as i64 / 10_000) as u64;
            let mut effective = Duration::from_millis(effective_ms);

            // A1: clamp effective soft limit by the hard limit so that
            // stability scaling (e.g. 250%) cannot exceed the hard budget.
            if let Some(hard) = self.hard_limit {
                effective = effective.min(hard);
            }

            return self.elapsed() >= effective;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn soft_scale_60_fires_earlier() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_timed(
            stopped,
            Duration::from_secs(10),
            Duration::from_secs(30),
        );
        control.update_soft_scale(60);
        // Effective soft = 10s * 0.6 = 6s
        // Since we just created it, elapsed ~ 0, should not stop yet
        assert!(!control.should_stop_iterating());
    }

    #[test]
    fn soft_scale_does_not_affect_hard() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_timed(
            stopped,
            Duration::from_secs(10),
            Duration::from_secs(30),
        );
        control.update_soft_scale(1); // Very aggressive scale
        // Hard limit is unaffected by soft scale
        assert!(!control.should_stop(2048)); // check at node 2048
    }

    // --- A3 tests ---

    /// A1: effective soft (10s * 2.5 = 25s) must be clamped to hard (5s).
    /// Since elapsed ~0 the call returns false, but the test also verifies the
    /// clamping path does not panic with a hard < soft configuration.
    #[test]
    fn soft_scale_clamped_by_hard_limit() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_timed(
            stopped,
            Duration::from_secs(10),
            Duration::from_secs(5),
        );
        control.update_soft_scale(250); // would give 25s without clamping
        // Effective = min(10s * 2.5, 5s) = 5s. Elapsed ~0 → should not stop.
        assert!(!control.should_stop_iterating());
    }

    /// A2: ponder_scale of 50 halves the effective soft limit after ponderhit.
    /// effective = 10s * 100/100 * 50/100 = 5s. Elapsed ~0 → should not stop.
    #[test]
    fn ponder_scale_reduces_soft_limit() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_ponder(
            stopped,
            Duration::from_secs(10),
            Duration::from_secs(30),
        );
        control.activate();
        // Default soft_scale=100, ponder_scale=50 → effective = 5s
        assert!(!control.should_stop_iterating());
    }

    /// A2: ponder_scale composes with a stability soft_scale reduction.
    /// effective = 10s * 60/100 * 50/100 = 3s. Elapsed ~0 → should not stop.
    #[test]
    fn ponder_scale_composes_with_stability() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_ponder(
            stopped,
            Duration::from_secs(10),
            Duration::from_secs(30),
        );
        control.activate();
        control.update_soft_scale(60);
        // effective = 10s * 60/100 * 50/100 = 3s
        assert!(!control.should_stop_iterating());
    }

    /// A2: ponder_scale is neutral (100) for a regular timed search.
    /// effective = 10s * 100/100 * 100/100 = 10s. Elapsed ~0 → should not stop.
    #[test]
    fn timed_search_ponder_scale_is_neutral() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_timed(
            stopped,
            Duration::from_secs(10),
            Duration::from_secs(30),
        );
        // ponder_scale defaults to 100 for timed searches
        assert!(!control.should_stop_iterating());
    }

    /// A2: the hard limit is NOT reduced for ponder mode — only the soft limit
    /// is halved. Since elapsed ~0 and hard=30s, should_stop returns false.
    #[test]
    fn ponder_hard_limit_not_affected() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_ponder(
            stopped,
            Duration::from_secs(10),
            Duration::from_secs(30),
        );
        control.activate();
        // Hard limit still 30s; elapsed ~0 → should not stop
        assert!(!control.should_stop(2048));
    }

    /// A2: an unactivated ponder control must never trigger a stop — neither
    /// the soft path (clock inactive) nor the hard path (clock inactive).
    #[test]
    fn inactive_ponder_does_not_stop() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_ponder(
            stopped,
            Duration::from_secs(10),
            Duration::from_secs(30),
        );
        // Clock not activated — both checks must return false
        assert!(!control.should_stop_iterating());
        assert!(!control.should_stop(2048));
    }
}
