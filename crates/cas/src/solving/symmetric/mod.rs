// SPDX-License-Identifier: AGPL-3.0-only
//! Unavailable symmetric capability, reserved for its owning lane.

/// This leaf has no checker or producer until its supplied-certificate tests land.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Unavailable
}

#[cfg(test)]
mod tests {
    /// The placeholder cannot advertise an executable capability.
    /// ´test:unit:symmetric-unavailable´
    #[test]
    fn symmetric_unavailable() {
        assert_eq!(
            super::availability(),
            crate::solving::Availability::Unavailable
        );
    }
}
