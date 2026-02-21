//! Time management — convert clock parameters to search limits.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use cesso_core::Color;

use crate::search::control::SearchControl;

/// Compute soft and hard time limits from remaining time and increment.
///
/// The formula differentiates between no-increment and increment games:
///
/// | Parameter | No increment | With increment |
/// |---|---|---|
/// | `moves_to_go` default | 30 | 25 |
/// | Hard cap (% remaining) | 12% | 25% |
/// | Hard/soft ratio cap | 2.5x | 3.0x |
/// | Increment contribution | n/a | `base + inc * 0.75` |
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
    let inc_ms = increment.as_millis() as f64;
    let has_increment = inc_ms > 0.0;

    let mtg = moves_to_go
        .unwrap_or(if has_increment { 25 } else { 30 })
        .max(1) as f64;

    let base = usable / mtg;

    let soft = if has_increment {
        base + inc_ms * 0.75
    } else {
        base
    };

    let hard_cap_pct = if has_increment { 0.25 } else { 0.12 };
    let hard_ratio_cap = if has_increment { 3.0 } else { 2.5 };

    let hard = (usable * hard_cap_pct).min(soft * hard_ratio_cap);

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
    fn compute_limits_with_increment() {
        let (soft, hard) = compute_limits(
            Duration::from_secs(300),
            Duration::from_secs(2),
            None,
        );
        // With increment: mtg=25, base = (300000-10)/25 ~ 11999, soft = 11999 + 1500 ~ 13499
        assert!(soft.as_millis() > 10_000, "soft={:?}", soft);
        assert!(soft.as_millis() < 20_000, "soft={:?}", soft);
        assert!(hard > soft, "hard={:?} should be > soft={:?}", hard, soft);
        // Hard cap: min(usable*0.25, soft*3.0) = min(74997, 40498) = 40498
        assert!(hard.as_millis() < 50_000, "hard={:?}", hard);
    }

    #[test]
    fn compute_limits_no_increment_conservative() {
        let (soft, hard) = compute_limits(
            Duration::from_secs(300),
            Duration::ZERO,
            None,
        );
        // No increment: mtg=30, base = (300000-10)/30 ~ 9999, soft ~ 9999
        assert!(soft.as_millis() > 8_000, "soft={:?}", soft);
        assert!(soft.as_millis() < 12_000, "soft={:?}", soft);
        // Hard cap: min(usable*0.12, soft*2.5) = min(35998, 24999) = 24999
        assert!(hard.as_millis() < 30_000, "hard should be tight without increment, hard={:?}", hard);
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
    fn compute_limits_no_increment_hard_cap_tight() {
        let (_soft, hard) = compute_limits(
            Duration::from_secs(60),
            Duration::ZERO,
            None,
        );
        // No increment: hard capped at 12% of remaining
        // 12% of 59990 = 7198.8
        assert!(hard.as_millis() <= 7_200, "no-increment hard cap should be tight, hard={:?}", hard);
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
