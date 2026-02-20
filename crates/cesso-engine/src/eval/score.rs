//! Packed middlegame/endgame score type used throughout evaluation.

use std::fmt;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// Packed middlegame/endgame evaluation score.
///
/// Encodes two `i16` values into a single `i32`:
/// - Middlegame (mg) in the upper 16 bits.
/// - Endgame (eg) in the lower 16 bits.
///
/// Encoding: `((mg as i32) << 16) + (eg as i32)`.
/// Extraction:
/// - `eg = self.0 as i16`
/// - `mg = ((self.0 + 0x8000) >> 16) as i16`
///
/// The `+0x8000` in the mg extraction compensates for sign contamination
/// from the eg field when eg is negative (its sign bit bleeds into the
/// upper half via ordinary arithmetic).
///
/// Addition and subtraction operate directly on the packed `i32`, which
/// is correct because the encoding is additive. Multiplication must
/// unpack, scale each component, and repack.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Score(i32);

impl Score {
    /// Zero score (mg=0, eg=0).
    pub const ZERO: Score = Score(0);

    /// Construct a `Score` from separate middlegame and endgame values.
    #[inline]
    pub const fn new(mg: i16, eg: i16) -> Score {
        // wrapping_add is required to handle the full i16 range: when mg is
        // i16::MIN the shift produces i32::MIN, and adding a negative eg
        // value would overflow with ordinary addition.
        Score(((mg as i32) << 16).wrapping_add(eg as i32))
    }

    /// Extract the middlegame component.
    #[inline]
    pub fn mg(self) -> i16 {
        // wrapping_add avoids overflow when self.0 is i32::MIN (eg = i16::MIN,
        // mg = i16::MIN): the +0x8000 compensates for eg sign contamination.
        (self.0.wrapping_add(0x8000) >> 16) as i16
    }

    /// Extract the endgame component.
    #[inline]
    pub fn eg(self) -> i16 {
        self.0 as i16
    }
}

/// Shorthand constructor for a packed [`Score`].
///
/// `S(mg, eg)` is equivalent to `Score::new(mg, eg)`. The uppercase name is a
/// deliberate domain convention shared across HCE chess engine literature.
#[allow(non_snake_case)]
#[inline]
pub const fn S(mg: i16, eg: i16) -> Score {
    Score::new(mg, eg)
}

impl Add for Score {
    type Output = Score;

    #[inline]
    fn add(self, rhs: Score) -> Score {
        Score(self.0 + rhs.0)
    }
}

impl AddAssign for Score {
    #[inline]
    fn add_assign(&mut self, rhs: Score) {
        self.0 += rhs.0;
    }
}

impl Sub for Score {
    type Output = Score;

    #[inline]
    fn sub(self, rhs: Score) -> Score {
        Score(self.0 - rhs.0)
    }
}

impl SubAssign for Score {
    #[inline]
    fn sub_assign(&mut self, rhs: Score) {
        self.0 -= rhs.0;
    }
}

impl Neg for Score {
    type Output = Score;

    #[inline]
    fn neg(self) -> Score {
        // Unpacking and repacking is required: negating the raw i32 would
        // corrupt the eg field when it is non-zero, because the encoding
        // mixes the sign bit of eg into the mg half.
        Score::new(-self.mg(), -self.eg())
    }
}

impl Mul<i16> for Score {
    type Output = Score;

    /// Multiply both components by a scalar.
    ///
    /// Unpacks mg/eg, scales each separately, then repacks. You cannot
    /// multiply the raw `i32` because cross-term contamination would
    /// corrupt both components.
    #[inline]
    fn mul(self, rhs: i16) -> Score {
        Score::new(self.mg() * rhs, self.eg() * rhs)
    }
}

impl fmt::Debug for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S({}, {})", self.mg(), self.eg())
    }
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S({}, {})", self.mg(), self.eg())
    }
}

#[cfg(test)]
mod tests {
    use super::{Score, S};

    #[test]
    fn roundtrip_positive() {
        let s = S(100, 200);
        assert_eq!(s.mg(), 100);
        assert_eq!(s.eg(), 200);
    }

    #[test]
    fn roundtrip_negative() {
        let s = S(-50, -30);
        assert_eq!(s.mg(), -50);
        assert_eq!(s.eg(), -30);
    }

    #[test]
    fn mixed_signs_pos_mg_neg_eg() {
        let s = S(100, -50);
        assert_eq!(s.mg(), 100);
        assert_eq!(s.eg(), -50);
    }

    #[test]
    fn mixed_signs_neg_mg_pos_eg() {
        let s = S(-100, 50);
        assert_eq!(s.mg(), -100);
        assert_eq!(s.eg(), 50);
    }

    #[test]
    fn addition() {
        assert_eq!(S(10, 20) + S(30, 40), S(40, 60));
    }

    #[test]
    fn subtraction() {
        assert_eq!(S(50, 60) - S(10, 20), S(40, 40));
    }

    #[test]
    fn negation() {
        assert_eq!(-S(10, -20), S(-10, 20));
    }

    #[test]
    fn multiply_positive_scalar() {
        assert_eq!(S(10, 20) * 3, S(30, 60));
    }

    #[test]
    fn multiply_negative_scalar() {
        assert_eq!(S(10, 20) * -2, S(-20, -40));
    }

    #[test]
    fn zero_constant() {
        assert_eq!(Score::ZERO.mg(), 0);
        assert_eq!(Score::ZERO.eg(), 0);
    }

    #[test]
    fn boundary_max() {
        let s = S(i16::MAX, i16::MAX);
        assert_eq!(s.mg(), i16::MAX);
        assert_eq!(s.eg(), i16::MAX);
    }

    #[test]
    fn boundary_min() {
        let s = S(i16::MIN, i16::MIN);
        assert_eq!(s.mg(), i16::MIN);
        assert_eq!(s.eg(), i16::MIN);
    }

    #[test]
    fn boundary_mixed() {
        let s = S(i16::MAX, i16::MIN);
        assert_eq!(s.mg(), i16::MAX);
        assert_eq!(s.eg(), i16::MIN);
    }

    #[test]
    fn add_assign() {
        let mut s = S(1, 2);
        s += S(3, 4);
        assert_eq!(s, S(4, 6));
    }
}
