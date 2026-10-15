// SPDX-License-Identifier: AGPL-3.0-only
//! Unavailable coefficients capability, reserved for its owning lane.

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

/// The coefficient lane supplies normalized multivariate coprimality checking here.
pub(crate) fn canonical_fraction(
    _state: &crate::graph::State,
    _numerator: &crate::graph::Handle,
    _denominator: &crate::graph::Handle,
    budget: &mut crate::budget::Budget,
) -> Result<(), crate::error::Fault> {
    let at = crate::error::Location::new("unavailable fraction normalization");
    budget.work(1, at)?;
    Err(crate::error::Fault::invalid(
        crate::error::InvalidCode::UnsupportedFragment,
        at,
    ))
}

#[cfg(test)]
mod tests {
    /// The placeholder cannot advertise an executable capability.
    /// ´test:unit:coefficients-unavailable´
    #[test]
    fn coefficients_unavailable() {
        assert_eq!(
            super::availability(),
            crate::solving::Availability::Unavailable
        );
        for tag in 0..18 {
            assert_eq!(
                super::schema_availability(tag),
                crate::solving::Availability::Unavailable
            );
        }
    }
}
