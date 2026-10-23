// SPDX-License-Identifier: AGPL-3.0-only
//! Typed views of checked scalar applications; there is only one portable witness language.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{HypothesisRef, Layer};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Condition, Handle, LogicalId, Vertex, size, vertex_key};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, GuardedObligation, Judgment,
    ScopedConditions, Statement,
};
use crate::scalar_model::{Claim, Guard, ProofStep, Relation};

/// Typed scalar theorem conversion location.
const AT: Location = Location::new("scalar statement view");

/// A scalar-only operation cannot reinterpret a different statement category.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedFragment, AT)
}

/// Construct a typed scalar statement on its exact context's admissible region.
pub(crate) fn scalar_statement(
    cas: &Cas,
    context: LogicalId,
    relation: &Relation,
    left: Handle,
    right: Handle,
) -> Result<Statement, Fault> {
    let domain = cas.handle(cas.state.context(context)?.admissible);
    Ok(match relation {
        Relation::EqualOnDomain => Statement::Equality {
            left,
            right,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        Relation::Derivative { variable } => Statement::DerivativeOn {
            function: left,
            derivative: right,
            coordinate: variable.clone(),
            domain,
        },
        Relation::Jacobian { variables } => Statement::Jacobian {
            input: left,
            output: right,
            variables: variables.clone(),
            domain,
        },
        Relation::SquareFree { variable, field } => Statement::SquareFree {
            input: left,
            output: right,
            variable: variable.clone(),
            field: field.clone(),
            domain,
        },
    })
}

/// Read typed scalar endpoints only after checking their explicit domain.
pub(crate) fn scalar_parts(
    cas: &Cas,
    context: &Handle,
    statement: &Statement,
) -> Result<(Relation, Handle, Handle), Fault> {
    let (relation, left, right, domain) = match statement {
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        } => (Relation::EqualOnDomain, left, right, domain),
        Statement::DerivativeOn {
            function,
            derivative,
            coordinate,
            domain,
        } => (
            Relation::Derivative {
                variable: coordinate.clone(),
            },
            function,
            derivative,
            domain,
        ),
        Statement::Jacobian {
            input,
            output,
            variables,
            domain,
        } => (
            Relation::Jacobian {
                variables: variables.clone(),
            },
            input,
            output,
            domain,
        ),
        Statement::SquareFree {
            input,
            output,
            variable,
            field,
            domain,
        } => (
            Relation::SquareFree {
                variable: variable.clone(),
                field: field.clone(),
            },
            input,
            output,
            domain,
        ),
        _ => return Err(invalid()),
    };
    let frame = cas.state.context(cas.checked(context)?)?;
    if cas.checked(domain)? != frame.admissible {
        return Err(Fault::invalid(InvalidCode::Context, AT));
    }
    for endpoint in [left, right] {
        let value = cas.state.expression(cas.checked(endpoint)?)?;
        if value.space.is_some() || crate::header::space_sort(&value.sort) {
            return Err(invalid());
        }
    }
    Ok((relation, left.clone(), right.clone()))
}

/// Translate a typed scalar application to the existing independent algebra matcher.
pub(crate) fn scalar_step(cas: &Cas, value: &ApplicationDraft) -> Result<ProofStep, Fault> {
    if matches!(&value.certificate, Certificate::Congruence(_))
        && value.rule != crate::rule::Primitive::Congruence
    {
        return Err(invalid());
    }
    let (relation, left, right) = scalar_parts(cas, &value.context, &value.conclusion)?;
    let (path, certificate) = match &value.certificate {
        Certificate::None => (Vec::new(), None),
        Certificate::Congruence(path) => (path.clone(), None),
        Certificate::SquareFree(value) => (Vec::new(), Some(value.as_ref().clone())),
        _ => return Err(invalid()),
    };
    Ok(ProofStep {
        primitive: value.rule,
        relation,
        left,
        right,
        premises: value.premises.clone(),
        path,
        substitutions: value.substitutions.clone(),
        certificate,
    })
}

/// Resolve the exact predicate node already retained by scalar proof publication.
fn predicate(
    cas: &Cas,
    context: LogicalId,
    guard: &Guard,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let term = cas.checked(&guard.term)?;
    let expression = cas.state.expression(term)?;
    let header = cas.state.context(context)?.header_id;
    if !crate::header::embeds(&cas.state, expression.header, header, budget)? {
        return Err(Fault::invalid(InvalidCode::Context, AT));
    }
    let value = Vertex::Condition(Condition {
        kind: guard.kind.predicate(),
        header,
        scope: expression.scope.clone(),
        arguments: vec![term],
    });
    let (_, key) = vertex_key(&value, &cas.state, budget)?;
    let id = cas.state.intern.get(&key).copied().ok_or_else(invalid)?;
    Ok(cas.handle(id))
}

/// Preserve the full guarded domain of each scalar requirement.
fn guarded(
    cas: &Cas,
    context: LogicalId,
    guard: &Guard,
    budget: &mut Budget,
) -> Result<GuardedObligation, Fault> {
    let term = cas.state.expression(cas.checked(&guard.term)?)?;
    Ok(GuardedObligation {
        home: cas.handle(context),
        scope: term.scope.clone(),
        layer: Layer::Pointwise,
        space: None,
        region: cas.handle(cas.state.context(context)?.admissible),
        predicate: predicate(cas, context, guard, budget)?,
    })
}

/// Materialize a scalar expected theorem as a fully typed judgment.
pub(crate) fn typed_claim(
    cas: &Cas,
    value: &Claim,
    budget: &mut Budget,
) -> Result<Judgment, Fault> {
    let context = cas.checked(&value.context)?;
    let mut residuals = Vec::new();
    residuals
        .try_reserve_exact(value.residuals.len())
        .map_err(|_| Fault::allocation(AT))?;
    for residual in &value.residuals {
        residuals.push(guarded(cas, context, residual, budget)?);
    }
    Ok(Judgment {
        context: value.context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: scalar_statement(
            cas,
            context,
            &value.relation,
            value.left.clone(),
            value.right.clone(),
        )?,
        residuals: crate::statement::union(cas, &residuals, budget)?,
    })
}

/// Convert only a lossless full-domain scalar judgment for the scalar search machinery.
#[expect(
    clippy::suspicious_operation_groupings,
    reason = "a residual's home must equal the judgment's context, not a nonexistent home field"
)]
pub(crate) fn scalar_claim(cas: &Cas, value: &Judgment) -> Result<Claim, Fault> {
    crate::context::require_closed(&cas.state, cas.checked(&value.context)?)?;
    if value.layer != Layer::Pointwise || value.space.is_some() {
        return Err(invalid());
    }
    let (relation, left, right) = scalar_parts(cas, &value.context, &value.statement)?;
    let domain = cas.state.context(cas.checked(&value.context)?)?.admissible;
    let mut residuals = Vec::new();
    residuals
        .try_reserve_exact(value.residuals.len())
        .map_err(|_| Fault::allocation(AT))?;
    for required in &value.residuals {
        if required.home != value.context
            || required.layer != value.layer
            || required.space.is_some()
            || cas.checked(&required.region)? != domain
        {
            return Err(invalid());
        }
        let Vertex::Condition(predicate) = cas.state.node(cas.checked(&required.predicate)?)?
        else {
            return Err(invalid());
        };
        let kind = match predicate.kind {
            crate::model::PredicateKind::Defined => crate::conditions::GuardKind::Defined,
            crate::model::PredicateKind::NonZero => crate::conditions::GuardKind::NonZero,
            _ => return Err(invalid()),
        };
        if predicate.arguments.len() != 1 || predicate.scope != required.scope {
            return Err(invalid());
        }
        residuals.push(Guard {
            kind,
            term: cas.handle(*predicate.arguments.first().ok_or_else(invalid)?),
        });
    }
    Ok(Claim {
        context: value.context.clone(),
        relation,
        left,
        right,
        residuals,
    })
}

/// Read any authoritative application as a complete typed statement and condition set.
pub(crate) fn application(
    cas: &Cas,
    proof: LogicalId,
    budget: &mut Budget,
) -> Result<CheckedApplication, Fault> {
    let Vertex::Proof(id) = cas.state.node(proof)? else {
        return Err(invalid());
    };
    match cas.state.node(*id)? {
        Vertex::ScopedApplication(value) => {
            prepay_scoped_copy(value, budget)?;
            Ok(value.as_ref().clone())
        }
        Vertex::Application(value) => {
            prepay_scalar_copy(value, budget)?;
            let claim = crate::proof_check::claim(cas, value, budget)?;
            let judgment = typed_claim(cas, &claim, budget)?;
            let mut conditions = ScopedConditions {
                residuals: judgment.residuals.clone(),
                ..ScopedConditions::default()
            };
            for required in &value.conditions.requirements {
                conditions
                    .requirements
                    .try_reserve(1)
                    .map_err(|_| Fault::allocation(AT))?;
                conditions.requirements.push(guarded(
                    cas,
                    value.context,
                    &Guard {
                        kind: required.kind,
                        term: cas.handle(required.term),
                    },
                    budget,
                )?);
            }
            let mut root = value.context;
            while let Some(parent) = cas.state.context(root)?.parent {
                budget.condition(1, AT)?;
                root = parent;
            }
            let assumptions = &cas.state.context(root)?.assumptions;
            for key in &value.conditions.hypotheses {
                let slot = assumptions
                    .iter()
                    .position(|(name, _)| name == key)
                    .ok_or_else(invalid)?;
                conditions.hypotheses.push(HypothesisRef {
                    context: cas.handle(root),
                    slot: u32::try_from(slot)
                        .map_err(|_| Fault::capacity(BudgetKind::ConditionAtoms, AT))?,
                });
            }
            let certificate = value.certificate.as_ref().map_or_else(
                || {
                    if value.path.is_empty() {
                        Certificate::None
                    } else {
                        Certificate::Congruence(value.path.clone())
                    }
                },
                |certificate| Certificate::SquareFree(Box::new(certificate.clone())),
            );
            let mut substitutions = Vec::new();
            substitutions
                .try_reserve_exact(value.substitutions.len())
                .map_err(|_| Fault::allocation(AT))?;
            for (key, replacement) in &value.substitutions {
                substitutions.push(crate::substitution::Substitution {
                    key: key.clone(),
                    replacement: cas.handle(*replacement),
                });
            }
            crate::statement::canonical_hypotheses(cas, &mut conditions.hypotheses, budget)?;
            Ok(CheckedApplication {
                scalar: Some(value.clone()),
                rule: value.primitive,
                judgment,
                premises: value.premises.clone(),
                substitutions,
                certificate,
                conditions,
                depth: value.depth,
            })
        }
        _ => Err(invalid()),
    }
}

/// Reserve only the scalar application being copied, including its owned certificate.
fn prepay_scalar_copy(
    value: &crate::scalar_model::CheckedStep,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let bytes = crate::proof_storage::pending_bytes(value, value.premises.len(), budget)?;
    budget.work(bytes, AT)?;
    Ok(bytes)
}

/// Native payload and its optional scalar cache are bounded before either is cloned.
fn prepay_scoped_copy(value: &CheckedApplication, budget: &mut Budget) -> Result<(), Fault> {
    let mut out = crate::encoding::Encoder::new(budget, AT);
    crate::statement::application(value, &mut out)?;
    let bytes = size(out.finish().len(), AT)?
        .checked_mul(4)
        .and_then(|n| n.checked_add(256))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    let scalar = value
        .scalar
        .as_ref()
        .map_or_else(|| Ok(0), |value| prepay_scalar_copy(value, budget))?;
    let retained = match &value.certificate {
        Certificate::SquareFree(value) => value.payload(budget)?,
        _ => 0,
    };
    let total = bytes
        .checked_add(scalar)
        .and_then(|n| n.checked_add(retained))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(BudgetKind::StagedPayloadBytes, total, AT)?;
    budget.work(
        bytes
            .checked_add(retained)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
        AT,
    )
}

/// Bound the number and payload of a typed application before caller-owned copies.
pub(crate) fn measure(
    cas: &Cas,
    value: &ApplicationDraft,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.check(BudgetKind::ProofNodes, size(value.premises.len(), AT)?, AT)?;
    cas.state.context(cas.checked(&value.context)?)?;
    crate::distribution_measure::measure(cas, &value.conclusion, &value.certificate, budget)?;
    if let Certificate::Emptiness(certificate) = &value.certificate {
        crate::emptiness::measure(cas, certificate, budget)?;
    }
    if let Certificate::Pullback(certificate) = &value.certificate {
        crate::pullback_check::measure(certificate, budget)?;
    }
    if let Certificate::Germ(certificate) = &value.certificate {
        crate::germ_wire::measure(certificate, budget)?;
    }
    if let Certificate::Split(certificate) = &value.certificate {
        crate::split_check::measure(certificate, budget)?;
    }
    crate::boundary_measure::statement(&value.conclusion, budget)?;
    crate::boundary_measure::certificate(cas, &value.certificate, budget)?;
    if let Certificate::Solve(certificate) = &value.certificate {
        for object in certificate.objects() {
            cas.checked(object)?;
        }
    }
    let objects = match &value.conclusion {
        Statement::BoundaryPartition(value) => {
            budget.check(
                BudgetKind::BoundaryNodes,
                size(value.boundaries.len(), AT)?,
                AT,
            )?;
            budget.check(BudgetKind::RegionCells, size(value.cells.len(), AT)?, AT)?;
            value
                .boundaries
                .len()
                .checked_add(value.cells.len())
                .and_then(|count| count.checked_add(2))
                .ok_or_else(invalid)?
        }
        Statement::Cover { members, .. } => {
            budget.check(BudgetKind::RegionNodes, size(members.len(), AT)?, AT)?;
            members.len().checked_add(1).ok_or_else(invalid)?
        }
        Statement::Jet { values, .. } => {
            budget.check(BudgetKind::JetEntries, size(values.len(), AT)?, AT)?;
            values.len().checked_add(3).ok_or_else(invalid)?
        }
        Statement::Jacobian { variables, .. } => {
            budget.check(
                BudgetKind::PolynomialVariables,
                size(variables.len(), AT)?,
                AT,
            )?;
            3
        }
        _ => 4,
    };
    let bytes = size(objects, AT)?.checked_mul(128).ok_or_else(invalid)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    for object in crate::statement::objects(&value.conclusion) {
        cas.checked(object)?;
    }
    if let Statement::Cover { members, .. } = &value.conclusion {
        let mut unique = std::collections::BTreeSet::new();
        for member in members {
            budget.condition(size(members.len(), AT)?, AT)?;
            if !unique.insert(cas.checked(member)?) {
                return Err(Fault::invalid(InvalidCode::Rule, AT));
            }
        }
    }
    let mut out = crate::encoding::Encoder::new(budget, AT);
    crate::statement::encode(&value.conclusion, &mut out, crate::statement::handle)?;
    crate::statement::certificate(&value.certificate, &mut out, crate::statement::handle)?;
    let bytes = size(out.finish().len(), AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)
}

/// Check the complete scope and layer before interpreting a caller's expected theorem.
pub(crate) fn validate_judgment(
    cas: &Cas,
    value: &Judgment,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::statement_check::validate(cas, &value.statement, budget)?;
    let context = cas.state.context(cas.checked(&value.context)?)?;
    if value.layer != crate::statement::layer(&value.statement)
        || value.space != crate::statement::space(&value.statement)
    {
        return Err(Fault::invalid(InvalidCode::Sort, AT));
    }
    for object in crate::statement::objects(&value.statement) {
        let header = match cas.state.node(cas.checked(object)?)? {
            Vertex::Solve(value) => {
                let Some((header, _)) = crate::solving::graph::metadata(&cas.state, value)? else {
                    continue;
                };
                header
            }
            Vertex::Expression(value) => value.header,
            Vertex::Region(value) => value.header,
            Vertex::Boundary(value) => value.header,
            Vertex::CaseArm(value) => value.header,
            Vertex::SourceForm(value) => value.header,
            Vertex::Condition(value) => value.header,
            Vertex::Space(value) => value.header,
            _ => return Err(Fault::invalid(InvalidCode::Reference, AT)),
        };
        if !crate::header::embeds(&cas.state, header, context.header_id, budget)? {
            return Err(Fault::invalid(InvalidCode::Context, AT));
        }
    }
    for residual in &value.residuals {
        crate::statement::validate_guard(cas, residual, budget)?;
    }
    Ok(())
}

/// Present a checked native scalar premise without dropping any scalar domain requirement.
pub(crate) fn scalar_application(
    cas: &Cas,
    value: &CheckedApplication,
    budget: &mut Budget,
) -> Result<crate::scalar_model::CheckedStep, Fault> {
    for used in &value.conditions.hypotheses {
        if cas
            .state
            .context(cas.checked(&used.context)?)?
            .parent
            .is_some()
        {
            return Err(invalid());
        }
    }
    if let Some(checked) = &value.scalar {
        prepay_scalar_copy(checked, budget)?;
        return Ok(checked.clone());
    }
    let claim = scalar_claim(cas, &value.judgment)?;
    let mut conditions = crate::conditions::Conditions::default();
    for (target, source) in [
        (&mut conditions.requirements, &value.conditions.requirements),
        (&mut conditions.residuals, &value.conditions.residuals),
    ] {
        for required in source {
            let view = Judgment {
                residuals: vec![required.clone()],
                ..value.judgment.clone()
            };
            let guard = scalar_claim(cas, &view)?
                .residuals
                .into_iter()
                .next()
                .ok_or_else(invalid)?;
            target.insert(crate::conditions::Obligation {
                kind: guard.kind,
                term: cas.checked(&guard.term)?,
            });
        }
    }
    let context = cas.checked(&value.judgment.context)?;
    for reference in &value.conditions.hypotheses {
        let frame = cas.state.context(cas.checked(&reference.context)?)?;
        if frame.parent.is_none() {
            let slot = usize::try_from(reference.slot).map_err(|_| invalid())?;
            conditions
                .hypotheses
                .insert(frame.assumptions.get(slot).ok_or_else(invalid)?.0.clone());
        }
    }
    budget.condition(size(conditions.requirements.len(), AT)?, AT)?;
    Ok(crate::scalar_model::CheckedStep {
        primitive: value.rule,
        relation: claim.relation,
        context,
        left: cas.checked(&claim.left)?,
        right: cas.checked(&claim.right)?,
        premises: value.premises.clone(),
        path: match &value.certificate {
            Certificate::Congruence(path) => path.clone(),
            _ => Vec::new(),
        },
        substitutions: value
            .substitutions
            .iter()
            .map(|item| Ok((item.key.clone(), cas.checked(&item.replacement)?)))
            .collect::<Result<_, Fault>>()?,
        certificate: match &value.certificate {
            Certificate::SquareFree(certificate) => Some(certificate.as_ref().clone()),
            _ => None,
        },
        conditions,
        depth: value.depth,
        cost: value.rule.specification().weight,
    })
}

/// Intern each required predicate before forming its complete guarded obligation.
pub(crate) fn scalar_conditions(
    cas: &mut Cas,
    value: &crate::scalar_model::CheckedStep,
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let header = cas.handle(cas.state.context(value.context)?.header_id);
    for required in &value.conditions.requirements {
        let scope = cas.state.expression(required.term)?.scope.clone();
        cas.admit(
            &[crate::graph::Draft::Condition {
                header: crate::graph::Reference::Existing(header.clone()),
                scope,
                kind: required.kind.predicate(),
                arguments: vec![crate::graph::Reference::Existing(cas.handle(required.term))],
            }],
            budget,
        )?;
    }
    let claim = crate::proof_check::claim(cas, value, budget)?;
    let judgment = typed_claim(cas, &claim, budget)?;
    let mut result = ScopedConditions {
        residuals: judgment.residuals,
        ..ScopedConditions::default()
    };
    for required in &value.conditions.requirements {
        result
            .requirements
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        result.requirements.push(guarded(
            cas,
            value.context,
            &Guard {
                kind: required.kind,
                term: cas.handle(required.term),
            },
            budget,
        )?);
    }
    let mut root = value.context;
    while let Some(parent) = cas.state.context(root)?.parent {
        budget.condition(1, AT)?;
        root = parent;
    }
    for key in &value.conditions.hypotheses {
        let slot = cas
            .state
            .context(root)?
            .assumptions
            .iter()
            .position(|(name, _)| name == key)
            .ok_or_else(invalid)?;
        result.hypotheses.push(HypothesisRef {
            context: cas.handle(root),
            slot: u32::try_from(slot).map_err(|_| invalid())?,
        });
    }
    Ok(result)
}

/// Transfer a sealed scalar producer result into the public scoped theorem vocabulary.
pub(crate) fn native_witness(
    cas: &Cas,
    value: crate::scalar_model::VerifiedWitness,
    budget: &mut Budget,
) -> Result<crate::proof_model::VerifiedWitness, Fault> {
    let application = application(cas, cas.checked(&value.proof)?, budget)?;
    Ok(crate::proof_model::VerifiedWitness {
        proof: value.proof,
        judgment: application.judgment,
        hypotheses: application.conditions.hypotheses,
        bytes: value.bytes,
    })
}
