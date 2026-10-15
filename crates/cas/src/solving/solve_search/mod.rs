// SPDX-License-Identifier: AGPL-3.0-only
//! Unavailable solve search capability, reserved for its owning lane.

/// This leaf has no checker or producer until its supplied-certificate tests land.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Unavailable
}

/// No solve transition or executor is advertised before the owning producer is implemented.
pub(crate) const fn executor() -> Option<super::SolveExecutor> {
    None
}

#[cfg(test)]
mod tests {
    /// The placeholder cannot advertise an executable capability.
    /// ´test:unit:solve-search-unavailable´
    #[test]
    fn solve_search_unavailable() {
        assert_eq!(
            super::availability(),
            crate::solving::Availability::Unavailable
        );
        assert!(super::executor().is_none());
    }
}
