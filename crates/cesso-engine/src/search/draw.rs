//! Draw offer/accept decision logic.

/// Decision after evaluating a draw situation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawDecision {
    /// Accept the opponent's draw offer.
    Accept,
    /// Offer a draw to the opponent.
    Offer,
    /// Continue playing.
    PlayOn,
}

/// Decide whether to accept, offer, or decline a draw.
///
/// # Arguments
///
/// * `score` — search score in centipawns from the engine's perspective.
/// * `contempt` — contempt factor in centipawns (positive = prefer playing on).
/// * `phase` — game phase (0 = endgame, 24 = full middlegame).
/// * `opponent_offered` — whether the opponent has offered a draw.
///
/// # Decision rules
///
/// * **Accept**: opponent offered AND `score <= -contempt`.
/// * **Offer**: opponent did NOT offer AND `contempt <= 0` AND `phase <= 6` AND `score.abs() <= 10`.
/// * **PlayOn**: everything else.
pub fn decide_draw(score: i32, contempt: i32, phase: i32, opponent_offered: bool) -> DrawDecision {
    // Accept: only when opponent offered and we're doing poorly enough
    if opponent_offered && score <= -contempt {
        return DrawDecision::Accept;
    }

    // Offer: only in endgames with no contempt and near-equal score
    if !opponent_offered && contempt <= 0 && phase <= 6 && score.abs() <= 10 {
        return DrawDecision::Offer;
    }

    DrawDecision::PlayOn
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Accept tests ---

    #[test]
    fn accept_when_losing_and_offered() {
        assert_eq!(decide_draw(-100, 0, 12, true), DrawDecision::Accept);
    }

    #[test]
    fn accept_when_equal_and_zero_contempt() {
        assert_eq!(decide_draw(0, 0, 12, true), DrawDecision::Accept);
    }

    #[test]
    fn accept_with_positive_contempt_only_when_losing_enough() {
        // contempt=50: accept only when score <= -50
        assert_eq!(decide_draw(-50, 50, 12, true), DrawDecision::Accept);
        assert_eq!(decide_draw(-100, 50, 12, true), DrawDecision::Accept);
    }

    #[test]
    fn decline_when_winning_despite_offer() {
        // contempt=50: score=0 > -50, so play on
        assert_eq!(decide_draw(0, 50, 12, true), DrawDecision::PlayOn);
        assert_eq!(decide_draw(100, 0, 12, true), DrawDecision::PlayOn);
    }

    #[test]
    fn accept_with_negative_contempt_generous() {
        // contempt=-50: accept when score <= 50
        assert_eq!(decide_draw(50, -50, 12, true), DrawDecision::Accept);
        assert_eq!(decide_draw(0, -50, 12, true), DrawDecision::Accept);
        assert_eq!(decide_draw(-100, -50, 12, true), DrawDecision::Accept);
    }

    #[test]
    fn decline_with_negative_contempt_too_much_advantage() {
        // contempt=-50: score=60 > 50, play on
        assert_eq!(decide_draw(60, -50, 12, true), DrawDecision::PlayOn);
    }

    // --- Offer tests ---

    #[test]
    fn offer_in_endgame_near_equal_zero_contempt() {
        assert_eq!(decide_draw(0, 0, 6, false), DrawDecision::Offer);
        assert_eq!(decide_draw(5, 0, 4, false), DrawDecision::Offer);
        assert_eq!(decide_draw(-10, 0, 0, false), DrawDecision::Offer);
    }

    #[test]
    fn offer_with_negative_contempt() {
        assert_eq!(decide_draw(0, -50, 6, false), DrawDecision::Offer);
    }

    #[test]
    fn no_offer_with_positive_contempt() {
        assert_eq!(decide_draw(0, 1, 6, false), DrawDecision::PlayOn);
    }

    #[test]
    fn no_offer_in_middlegame() {
        assert_eq!(decide_draw(0, 0, 7, false), DrawDecision::PlayOn);
        assert_eq!(decide_draw(0, 0, 24, false), DrawDecision::PlayOn);
    }

    #[test]
    fn no_offer_when_not_near_equal() {
        assert_eq!(decide_draw(11, 0, 6, false), DrawDecision::PlayOn);
        assert_eq!(decide_draw(-11, 0, 6, false), DrawDecision::PlayOn);
    }

    #[test]
    fn no_offer_when_opponent_already_offered() {
        // If opponent offered, we go through accept logic, not offer
        assert_eq!(decide_draw(0, 0, 6, true), DrawDecision::Accept);
    }

    // --- PlayOn tests ---

    #[test]
    fn play_on_when_winning_no_offer() {
        assert_eq!(decide_draw(200, 0, 12, false), DrawDecision::PlayOn);
    }

    #[test]
    fn play_on_default() {
        assert_eq!(decide_draw(50, 50, 12, false), DrawDecision::PlayOn);
    }
}
