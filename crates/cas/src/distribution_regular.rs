// SPDX-License-Identifier: AGPL-3.0-only
//! Finite polynomial regularity and explicit regular embeddings.

use crate::budget::Budget;
use crate::distribution_model::{
    DistributionCertificate, DistributionStatement, RegularCertificate, RegularityKind,
};
use crate::distribution_syntax::{AT, invalid};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};

/// Reconstruct the complete geometry and every inhabited open polynomial cell.
pub(crate) fn certificate(
    cas: &mut Cas,
    function: &Handle,
    space: &Handle,
    budget: &mut Budget,
) -> Result<RegularCertificate, Fault> {
    let definition = cas.state.space(cas.checked(space)?)?.clone();
    let domain = cas.handle(definition.domain);
    crate::boundary_partition::inventory(cas, function, &domain, budget)?;
    let root = cas.checked(function)?;
    let analysis = crate::case_cells::analyze(cas, root, root, definition.domain, budget)?;
    let polynomials =
        analysis.polynomial_cells(cas, root, definition.domain, &definition.coordinate, budget)?;
    Ok(RegularCertificate {
        regions: analysis.regions.iter().map(|id| cas.handle(*id)).collect(),
        partition: analysis.cells,
        polynomials,
    })
}

/// Find a previously checked complete finite polynomial certificate for this exact representative.
pub(crate) fn evidence<'a>(
    function: &Handle,
    space: &Handle,
    premises: &'a [CheckedApplication],
) -> Result<&'a RegularCertificate, Fault> {
    for premise in premises {
        let Statement::Distribution(statement) = &premise.judgment.statement else {
            continue;
        };
        let DistributionStatement::Regularity {
            function: input,
            space: actual,
            ..
        } = statement.as_ref()
        else {
            continue;
        };
        if input == function
            && actual == space
            && let Certificate::Distribution(certificate) = &premise.certificate
            && let DistributionCertificate::Regular(value) = certificate.as_ref()
        {
            return Ok(value);
        }
    }
    Err(Fault::invalid(InvalidCode::PendingPremise, AT))
}

/// The regular schema establishes regularity, explicit embedding or its defining test action.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Distribution(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    match statement.as_ref() {
        DistributionStatement::Regularity {
            function,
            space,
            property,
        } => {
            if !premises.is_empty() {
                return Err(invalid());
            }
            let expected = certificate(cas, function, space, budget)?;
            if draft.certificate
                != Certificate::Distribution(Box::new(DistributionCertificate::Regular(Box::new(
                    expected,
                ))))
            {
                return Err(invalid());
            }
            if *property == RegularityKind::Smooth {
                polynomial(cas, function, space, budget)?;
            }
            Ok(())
        }
        DistributionStatement::Embedding {
            function,
            embedding,
            space,
        } => {
            if premises.len() != 1 || draft.certificate != Certificate::None {
                return Err(invalid());
            }
            evidence(function, space, premises)?;
            let value = cas.state.expression(cas.checked(embedding)?)?;
            if value.operator != Operator::RegularEmbed || value.operands != [function.logical_id()]
            {
                return Err(invalid());
            }
            Ok(())
        }
        DistributionStatement::Pairing {
            pairing,
            value,
            space,
        } => {
            if draft.certificate != Certificate::None {
                return Err(invalid());
            }
            action(cas, pairing, value, space, premises, budget)
        }
        _ => Err(invalid()),
    }
}

/// A single exact polynomial provides global smoothness without a supplied Boolean premise.
pub(crate) fn polynomial(
    cas: &Cas,
    function: &Handle,
    space: &Handle,
    budget: &mut Budget,
) -> Result<Vec<crate::number::Rational>, Fault> {
    let definition = cas.state.space(cas.checked(space)?)?;
    crate::polynomial::univariate(
        &cas.state,
        cas.checked(function)?,
        &definition.coordinate,
        budget,
    )
}

/// The defining action remains symbolic; no integration algorithm is smuggled into this rule.
fn action(
    cas: &Cas,
    pairing: &Handle,
    value: &Handle,
    space: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.specialized(crate::budget::BudgetKind::PairingSteps, 1, true, AT)?;
    let pair = cas.state.expression(cas.checked(pairing)?)?.clone();
    let [distribution, test] = pair.operands.as_slice() else {
        return Err(invalid());
    };
    if pair.operator != Operator::Pair {
        return Err(invalid());
    }
    crate::distribution_check::membership(cas, *distribution, space, premises, budget)?;
    crate::distribution_check::test(cas, *test, space, budget)?;
    let regular = cas.state.expression(*distribution)?;
    let [function] = regular.operands.as_slice() else {
        return Err(invalid());
    };
    let output = cas.state.expression(cas.checked(value)?)?;
    if regular.operator != Operator::RegularEmbed
        || output.operator != Operator::RegularAction
        || output.operands != [*function, *test]
    {
        return Err(invalid());
    }
    Ok(())
}
