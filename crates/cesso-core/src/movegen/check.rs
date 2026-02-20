//! Compile-time check state dispatch for move generation.

/// Marker trait for compile-time check state dispatch.
pub(crate) trait CheckType {
    const IN_CHECK: bool;
}

/// Zero-sized type indicating the king is in check.
pub(crate) struct InCheck;
impl CheckType for InCheck {
    const IN_CHECK: bool = true;
}

/// Zero-sized type indicating the king is not in check.
pub(crate) struct NotInCheck;
impl CheckType for NotInCheck {
    const IN_CHECK: bool = false;
}
