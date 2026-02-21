//! Time management — convert clock parameters to search limits.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use cesso_core::Color;

use crate::search::control::SearchControl;

/// Compute soft and hard time limits from remaining time and increment.
///
/// Formula:
/// - `base = remaining / moves_to_go` (default 25 if not specified)
/// - `soft = base + increment * 0.75`
/// - `hard = min(remaining * 0.3, soft * 3.0)`
/// - Both clamped to `remaining - 10ms` overhead
///
/// Edge cases: very low time (< 10ms) or zero remaining yield 1ms limits.
pub fn compute_limits(
    remaining: Duration,
    increment: Duration,
    moves_to_go: Option<u32>,
) -> (Duration, Duration) {
    let remaining_ms = remaining.as_millis() as f64;

    if remaining_ms < 10.0 {
        let one_ms = Duration::from_millis(1);
        return (one_ms, one_ms);
    }

    let overhead = 10.0;
    let usable = (remaining_ms - overhead).max(1.0);
    let mtg = moves_to_go.unwrap_or(25).max(1) as f64;
    let inc_ms = increment.as_millis() as f64;

    let base = usable / mtg;
    let soft = base + inc_ms * 0.75;
    let hard = (usable * 0.3).min(soft * 3.0);

    let soft = soft.min(usable).max(1.0);
    let hard = hard.min(usable).max(1.0);

    (
        Duration::from_millis(soft as u64),
        Duration::from_millis(hard as u64),
    )
}

/// Build a [`SearchControl`] from UCI `go` parameters and the side to move.
///
/// Priority order:
/// 1. `ponder: true` with time -> `SearchControl::new_ponder`
/// 2. `infinite: true` -> `SearchControl::new_infinite`
/// 3. `movetime: Some(d)` -> `SearchControl::new_timed(d, d)`
/// 4. `wtime/btime` present -> `compute_limits()` then `SearchControl::new_timed`
/// 5. `depth` only / bare `go` -> `SearchControl::new_infinite`
#[allow(clippy::too_many_arguments)]
pub fn limits_from_go(
    wtime: Option<Duration>,
    btime: Option<Duration>,
    winc: Option<Duration>,
    binc: Option<Duration>,
    movestogo: Option<u32>,
    movetime: Option<Duration>,
    infinite: bool,
    ponder: bool,
    side: Color,
    stopped: Arc<AtomicBool>,
) -> SearchControl {
    // Pick the time/increment for the side to move
    let (remaining, increment) = match side {
        Color::White => (wtime, winc),
        Color::Black => (btime, binc),
    };

    if infinite && !ponder {
        return SearchControl::new_infinite(stopped);
    }

    if let Some(mt) = movetime {
        if ponder {
            return SearchControl::new_ponder(stopped, mt, mt);
        }
        return SearchControl::new_timed(stopped, mt, mt);
    }

    if let Some(rem) = remaining {
        let inc = increment.unwrap_or(Duration::ZERO);
        let (soft, hard) = compute_limits(rem, inc, movestogo);

        if ponder {
            return SearchControl::new_ponder(stopped, soft, hard);
        }
        return SearchControl::new_timed(stopped, soft, hard);
    }

    // depth-only or bare `go` — no time limits
    if ponder {
        // Ponder with no time info — just infinite pondering
        return SearchControl::new_infinite(stopped);
    }
    SearchControl::new_infinite(stopped)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Duration;

    use cesso_core::Color;

    use crate::time::compute_limits;
    use crate::time::limits_from_go;

    #[test]
    fn compute_limits_standard_game() {
        let (soft, hard) = compute_limits(
            Duration::from_secs(300),
            Duration::from_secs(2),
            None,
        );
        // base = (300000 - 10) / 25 ~ 11999.6, soft = 11999.6 + 1500 ~ 13499
        assert!(soft.as_millis() > 10_000, "soft={:?}", soft);
        assert!(soft.as_millis() < 20_000, "soft={:?}", soft);
        assert!(hard > soft, "hard={:?} should be > soft={:?}", hard, soft);
    }

    #[test]
    fn compute_limits_very_low_time() {
        let (soft, hard) = compute_limits(
            Duration::from_millis(5),
            Duration::ZERO,
            None,
        );
        assert_eq!(soft, Duration::from_millis(1));
        assert_eq!(hard, Duration::from_millis(1));
    }

    #[test]
    fn compute_limits_zero_remaining() {
        let (soft, hard) = compute_limits(
            Duration::ZERO,
            Duration::ZERO,
            None,
        );
        assert_eq!(soft, Duration::from_millis(1));
        assert_eq!(hard, Duration::from_millis(1));
    }

    #[test]
    fn compute_limits_with_movestogo() {
        let (soft, _hard) = compute_limits(
            Duration::from_secs(60),
            Duration::ZERO,
            Some(10),
        );
        // base = (60000 - 10) / 10 ~ 5999, soft ~ 5999 (no increment)
        assert!(soft.as_millis() > 4_000, "soft={:?}", soft);
        assert!(soft.as_millis() < 8_000, "soft={:?}", soft);
    }

    #[test]
    fn limits_from_go_infinite() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = limits_from_go(
            None, None, None, None, None, None,
            true, false, Color::White, stopped,
        );
        // Infinite should not stop on its own
        assert!(!control.should_stop(10000));
        assert!(!control.should_stop_iterating());
    }

    #[test]
    fn limits_from_go_movetime() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = limits_from_go(
            None, None, None, None, None,
            Some(Duration::from_secs(5)),
            false, false, Color::White, stopped,
        );
        // Should not stop immediately
        assert!(!control.should_stop_iterating());
    }

    #[test]
    fn limits_from_go_with_clock() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = limits_from_go(
            Some(Duration::from_secs(300)),
            Some(Duration::from_secs(300)),
            Some(Duration::from_secs(2)),
            Some(Duration::from_secs(2)),
            None, None,
            false, false, Color::White, stopped,
        );
        // Should not stop immediately with 5 minutes
        assert!(!control.should_stop_iterating());
    }

    #[test]
    fn limits_from_go_depth_only() {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = limits_from_go(
            None, None, None, None, None, None,
            false, false, Color::White, stopped,
        );
        // Should behave like infinite
        assert!(!control.should_stop(10000));
    }
}
