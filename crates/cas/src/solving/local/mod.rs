// SPDX-License-Identifier: AGPL-3.0-only
//! Unavailable local capability, reserved for its owning lane.

/// This leaf has no checker or producer until its supplied-certificate tests land.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Unavailable
}

/// Per-subtype capability permits independently green checker slices in this leaf.
pub(crate) const fn schema_availability(_tag: u8) -> super::Availability {
    super::Availability::Unavailable
}

/// Reject every application until this lane supplies its independent typed checker.
pub(crate) fn check(
    _cas: &crate::graph::Cas,
    _draft: &crate::proof_model::ApplicationDraft,
    _premises: &[crate::proof_model::CheckedApplication],
    budget: &mut crate::budget::Budget,
) -> Result<crate::proof_model::ScopedConditions, crate::error::Fault> {
    budget.work(
        1,
        crate::error::Location::new("unavailable solving checker"),
    )?;
    Err(crate::error::Fault::invalid(
        crate::error::InvalidCode::UnsupportedFragment,
        crate::error::Location::new("unavailable solving checker"),
    ))
}

/// No solve transition or executor is advertised before the owning producer is implemented.
pub(crate) const fn executor() -> Option<super::SolveExecutor> {
    None
}

#[cfg(test)]
mod tests {
    /// The placeholder cannot advertise an executable capability.
    /// ´test:unit:local-unavailable´
    #[test]
    fn local_unavailable() {
        assert_eq!(
            super::availability(),
            crate::solving::Availability::Unavailable
        );
        assert!(super::executor().is_none());
        for tag in 0..18 {
            assert_eq!(
                super::schema_availability(tag),
                crate::solving::Availability::Unavailable
            );
        }
    }
}
