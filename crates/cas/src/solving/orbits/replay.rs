// SPDX-License-Identifier: AGPL-3.0-only
//! Closure and reaching words prove exactly the declared generated orbit.

use super::action::{AT, equals, invalid, permutation};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle, size};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};
use crate::solving::coefficients::storage::{polynomial, vector};
use crate::solving::{
    CertificatePermutationOrbitCoverage, PolynomialData, SolveCertificate, SolveSchema,
    SolveStatement,
};

/// Charge all nested comparison fields before comparing the exact claimed conclusion.
fn conclusion(
    draft: &ApplicationDraft,
    data: &CertificatePermutationOrbitCoverage,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for row in data
        .generators
        .iter()
        .chain(&data.actions)
        .chain(&data.words)
    {
        budget.work(size(row.len(), AT)?.max(1), AT)?;
    }
    budget.work(size(data.members.len(), AT)?.max(1), AT)?;
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::Orbit(value) = statement.as_ref() else {
        return Err(invalid());
    };
    if (
        &value.seed,
        &value.generators,
        &value.members,
        &value.actions,
    ) != (&data.seed, &data.generators, &data.members, &data.actions)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Read distinct normalized members in precisely the seed's coefficient algebra and slot frame.
fn members<'a>(
    cas: &'a Cas,
    data: &CertificatePermutationOrbitCoverage,
    budget: &mut Budget,
) -> Result<Vec<&'a PolynomialData<Handle>>, Fault> {
    budget.solving(BudgetKind::OrbitMembers, size(data.members.len(), AT)?, AT)?;
    let seed = polynomial(&cas.state, &data.seed)?;
    if (
        usize::try_from(data.degree).map_err(|_| invalid())?,
        data.members.is_empty(),
        data.actions.len(),
        data.words.len(),
    ) != (
        seed.variables.len(),
        false,
        data.members.len(),
        data.members.len(),
    ) {
        return Err(invalid());
    }
    for generator in &data.generators {
        permutation(generator, &seed.variables, budget)?;
    }
    let mut result: Vec<&PolynomialData<Handle>> = vector(data.members.len(), budget)?;
    for handle in &data.members {
        cas.checked(handle)?;
        let member = polynomial(&cas.state, handle)?;
        budget.work(size(member.variables.len(), AT)?.max(1), AT)?;
        if (&member.domain, &member.variables) != (&seed.domain, &seed.variables) {
            return Err(invalid());
        }
        for earlier in &result {
            if equals(&cas.state, earlier, member, None, budget)? {
                return Err(invalid());
            }
        }
        result.push(member);
    }
    Ok(result)
}

/// Recompute every action, including duplicate images, before trusting any table entry.
fn closure(
    cas: &Cas,
    data: &CertificatePermutationOrbitCoverage,
    orbit: &[&PolynomialData<Handle>],
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (source, row) in orbit.iter().zip(&data.actions) {
        if row.len() != data.generators.len() {
            return Err(invalid());
        }
        for (generator, destination) in data.generators.iter().zip(row) {
            budget.solving(BudgetKind::OrbitPermutations, 1, AT)?;
            let target = orbit
                .get(usize::try_from(*destination).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            if !equals(&cas.state, source, target, Some(generator), budget)? {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// A reaching word is interpreted in the already independently verified action table.
fn reachability(
    cas: &Cas,
    data: &CertificatePermutationOrbitCoverage,
    orbit: &[&PolynomialData<Handle>],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let seed = polynomial(&cas.state, &data.seed)?;
    let mut start = None;
    for (index, member) in orbit.iter().enumerate() {
        if equals(&cas.state, seed, member, None, budget)? {
            start = Some(index);
            break;
        }
    }
    let start = start.ok_or_else(invalid)?;
    for (destination, word) in data.words.iter().enumerate() {
        let mut current = start;
        for generator in word {
            budget.solving(BudgetKind::OrbitPermutations, 1, AT)?;
            let row = data.actions.get(current).ok_or_else(invalid)?;
            let target = row
                .get(usize::try_from(*generator).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            current = usize::try_from(*target).map_err(|_| invalid())?;
        }
        if current != destination {
            return Err(invalid());
        }
    }
    Ok(())
}

/// The common checker owns atomic publication and preservation of all inherited conditions.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let inherited = crate::solving::coefficients::proof::conditions(cas, draft, premises, budget)?;
    let Certificate::Solve(certificate) = &draft.certificate else {
        return Err(invalid());
    };
    let SolveCertificate::PermutationOrbitCoverage(data) = certificate.as_ref() else {
        return Err(invalid());
    };
    if draft.rule != crate::rule::Primitive::Solve(SolveSchema::PermutationOrbitCoverage) {
        return Err(invalid());
    }
    cas.checked(&data.seed)?;
    conclusion(draft, data, budget)?;
    let orbit = members(cas, data, budget)?;
    closure(cas, data, &orbit, budget)?;
    reachability(cas, data, &orbit, budget)?;
    Ok(inherited)
}
