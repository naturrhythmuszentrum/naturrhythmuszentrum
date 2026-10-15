// SPDX-License-Identifier: AGPL-3.0-only
//! Unavailable numeric capability, reserved for its owning lane.

/// This leaf has no checker or producer until its supplied-certificate tests land.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Unavailable
}

#[cfg(test)]
mod tests {
    /// The placeholder cannot advertise an executable capability.
    /// ´test:unit:numeric-unavailable´
    #[test]
    fn numeric_unavailable() {
        assert_eq!(
            super::availability(),
            crate::solving::Availability::Unavailable
        );
    }
}
