// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh output declarations have an unrestricted context independent of physical input guards.

use super::{AT, LcSpectralLaw, SpectralLawError as Error};
use nrz_cas::{
    Cas, FaultKind, Handle, InvalidCode,
    domain::{AmbientDomain, HypothesisRef},
    limits::Budget,
    source::{RegionConstructor, SyntaxNode},
};

/// A closed producer initializes fresh output slots; its proof context is not a free input equation.
pub(super) fn check(
    cas: &Cas,
    law: &LcSpectralLaw,
    context: &Handle,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.work(3, AT)?;
    cas.context_header(law.operator().frame().context())?;
    cas.context_header(context)?;
    let region = cas.admissible_region(context)?;
    let syntax = cas.describe_syntax(&region, budget)?;
    if !matches!(
        syntax.node,
        SyntaxNode::Region(AmbientDomain::Assignments, RegionConstructor::Ambient)
    ) || !syntax.scope.is_empty()
        || !syntax.dependencies.is_empty()
    {
        return Err(Error::Domain);
    }
    let first = HypothesisRef {
        context: context.clone(),
        slot: 0,
    };
    match cas.hypothesis_name(&first) {
        Err(error) if error.kind == FaultKind::Invalid(InvalidCode::Reference) => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(Error::Conditional),
    }
}
