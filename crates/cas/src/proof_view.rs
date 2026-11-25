// SPDX-License-Identifier: AGPL-3.0-only
//! Typed views of checked scalar applications; there is only one portable witness language.

mod transport;

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
    budget: &mut crate::budget::Budget,
) -> Result<Statement, Fault> {
    let domain = cas.handle(cas.state.context(context, budget)?.admissible);
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
    budget: &mut crate::budget::Budget,
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
    let frame = cas.state.context(cas.checked(context, budget)?, budget)?;
    if cas.checked(domain, budget)? != frame.admissible {
        return Err(Fault::invalid(InvalidCode::Context, AT));
    }
    for endpoint in [left, right] {
        let value = cas
            .state
            .expression(cas.checked(endpoint, budget)?, budget)?;
        if value.space.is_some() || crate::header::space_sort(&value.sort) {
            return Err(invalid());
        }
    }
    Ok((relation, left.clone(), right.clone()))
}

/// Translate a typed scalar application to the existing independent algebra matcher.
pub(crate) fn scalar_step(
    cas: &Cas,
    value: &ApplicationDraft,
    budget: &mut crate::budget::Budget,
) -> Result<ProofStep, Fault> {
    if matches!(&value.certificate, Certificate::Congruence(_))
        && value.rule != crate::rule::Primitive::Congruence
    {
        return Err(invalid());
    }
    let (relation, left, right) = scalar_parts(cas, &value.context, &value.conclusion, budget)?;
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
    let term = cas.checked(&guard.term, budget)?;
    let expression = cas.state.expression(term, budget)?;
    let header = cas.state.context(context, budget)?.header_id;
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
    let id = cas
        .state
        .intern
        .get(&key, 0, budget, AT)?
        .copied()
        .ok_or_else(invalid)?;
    Ok(cas.handle(id))
}

/// Preserve the full guarded domain of each scalar requirement.
fn guarded(
    cas: &Cas,
    context: LogicalId,
    guard: &Guard,
    budget: &mut Budget,
) -> Result<GuardedObligation, Fault> {
    let term = cas
        .state
        .expression(cas.checked(&guard.term, budget)?, budget)?;
    Ok(GuardedObligation {
        home: cas.handle(context),
        scope: term.scope.clone(),
        layer: Layer::Pointwise,
        space: None,
        region: cas.handle(cas.state.context(context, budget)?.admissible),
        predicate: predicate(cas, context, guard, budget)?,
    })
}

/// Materialize a scalar expected theorem as a fully typed judgment.
pub(crate) fn typed_claim(
    cas: &Cas,
    value: &Claim,
    budget: &mut Budget,
) -> Result<Judgment, Fault> {
    let context = cas.checked(&value.context, budget)?;
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
            budget,
        )?,
        residuals: crate::statement::union(cas, &residuals, budget)?,
    })
}

/// Convert only a lossless full-domain scalar judgment for the scalar search machinery.
#[expect(
    clippy::suspicious_operation_groupings,
    reason = "a residual's home must equal the judgment's context, not a nonexistent home field"
)]
pub(crate) fn scalar_claim(
    cas: &Cas,
    value: &Judgment,
    budget: &mut Budget,
) -> Result<Claim, Fault> {
    crate::context::require_closed(&cas.state, cas.checked(&value.context, budget)?, budget)?;
    if value.layer != Layer::Pointwise || value.space.is_some() {
        return Err(invalid());
    }
    let (relation, left, right) = scalar_parts(cas, &value.context, &value.statement, budget)?;
    let mut residuals = Vec::new();
    residuals
        .try_reserve_exact(value.residuals.len())
        .map_err(|_| Fault::allocation(AT))?;
    for required in &value.residuals {
        if required.home != value.context
            || required.layer != value.layer
            || required.space.is_some()
        {
            return Err(invalid());
        }
        let Vertex::Condition(predicate) = cas
            .state
            .node(cas.checked(&required.predicate, budget)?, budget)?
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
        let context = cas.checked(&value.context, budget)?;
        let region = cas.checked(&required.region, budget)?;
        if region != cas.state.context(context, budget)?.admissible {
            crate::statement::validate_guard(cas, required, budget)?;
        }
        transport::check(cas, context, region, budget)?;
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

#[cfg(test)]
thread_local! {
    /// Observe owned application copies independently of sizing and charged Work.
    static COPIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Thread-local observation does not change the graph or the budget ledger.
#[cfg(test)]
pub(crate) fn copied_applications() -> usize {
    COPIES.with(std::cell::Cell::get)
}

/// Read any authoritative application as a complete typed statement and condition set.
pub(crate) fn application(
    cas: &Cas,
    proof: LogicalId,
    budget: &mut Budget,
) -> Result<CheckedApplication, Fault> {
    let Vertex::Proof(id) = cas.state.node(proof, budget)? else {
        return Err(invalid());
    };
    match cas.state.node(*id, budget)? {
        Vertex::AnonymousLemma(_) => {
            let value = cas.series_application(*id, budget)?;
            prepay_scoped_copy(value, budget)?;
            #[cfg(test)]
            COPIES.with(|copies| copies.set(copies.get().saturating_add(1)));
            Ok(value.clone())
        }
        Vertex::ScopedApplication(value) => {
            prepay_scoped_copy(value, budget)?;
            #[cfg(test)]
            COPIES.with(|copies| copies.set(copies.get().saturating_add(1)));
            Ok(value.as_ref().clone())
        }
        Vertex::Application(value) => {
            prepay_scalar_copy(value, budget)?;
            #[cfg(test)]
            COPIES.with(|copies| copies.set(copies.get().saturating_add(1)));
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
            while let Some(parent) = cas.state.context(root, budget)?.parent {
                budget.condition(1, AT)?;
                root = parent;
            }
            let assumptions = &cas.state.context(root, budget)?.assumptions;
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
            local_hypotheses(cas, value, &mut conditions, budget)?;
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
    crate::work_observation::observe!(budget, AT, {
        let bytes = crate::proof_storage::pending_bytes(value, value.premises.len(), budget)?;
        budget.work(bytes, AT)?;
        Ok(bytes)
    })
}

/// Native payload and its optional scalar cache are bounded before either is cloned.
fn prepay_scoped_copy(value: &CheckedApplication, budget: &mut Budget) -> Result<u64, Fault> {
    crate::work_observation::observe!(budget, AT, {
        let mut out = crate::encoding::Encoder::new(budget, AT);
        crate::statement::application(value, &mut out)?;
        let bytes = size(out.finish().len(), AT)?
            .checked_mul(4)
            .and_then(|n| n.checked_add(256))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        let scalar = value
            .scalar
            .as_ref()
            .map_or(Ok(0), |value| prepay_scalar_copy(value, budget))?;
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
        )?;
        Ok(total)
    })
}

/// Prepay one authenticated premise copy before reserving an aggregate retained inventory.
pub(crate) fn application_bytes(
    cas: &Cas,
    proof: LogicalId,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let Vertex::Proof(id) = cas.state.node(proof, budget)? else {
        return Err(invalid());
    };
    match cas.state.node(*id, budget)? {
        Vertex::AnonymousLemma(_) => {
            prepay_scoped_copy(cas.series_application(*id, budget)?, budget)
        }
        Vertex::ScopedApplication(value) => prepay_scoped_copy(value, budget),
        Vertex::Application(value) => prepay_scalar_copy(value, budget),
        _ => Err(invalid()),
    }
}

/// Refuse surplus typed slots before inspecting any referenced proof payload.
/// Variable shapes use the declared `ProofNodes` ceiling and retain mode-specific
/// validation in their checkers; aggregate payload reservation bounds their storage.
pub(crate) fn premise_arity(
    cas: &Cas,
    rule: crate::rule::Primitive,
    statement: &Statement,
    count: usize,
    budget: &mut Budget,
    at: Location,
) -> Result<(), Fault> {
    use crate::rule::PremiseShape;

    let count = size(count, at)?;
    let valid = match rule.specification().premises {
        PremiseShape::Exact(expected) => count == u64::from(expected),
        PremiseShape::SeriesEvidence => {
            let Statement::Series(value) = statement else {
                return Err(Fault::invalid(InvalidCode::Rule, at));
            };
            value.primitive() == rule
                && count == size(crate::series::check::premise_count(cas, value, budget)?, at)?
        }
        PremiseShape::AtLeast(minimum) => {
            // Formal coefficient closure permits one anchored identity; the
            // checker still validates its exact frame, scope and certificate.
            let formal_close = rule == crate::rule::Primitive::ContextClose
                && matches!(statement, Statement::Solve(value)
                    if matches!(value.as_ref(), crate::solving::SolveStatement::CoefficientEquality(_)));
            count >= if formal_close { 1 } else { u64::from(minimum) }
        }
        PremiseShape::SolveEvidence
        | PremiseShape::DistributionEvidence
        | PremiseShape::BoundaryEvidence
        | PremiseShape::CaseEvidence
        | PremiseShape::JacobianCells => true,
    };
    if !valid {
        return Err(Fault::invalid(InvalidCode::Rule, at));
    }
    // Dependent shapes retain the declared proof inventory ceiling; their checker
    // validates the certificate-specific consumption of each admitted slot.
    budget.check(BudgetKind::ProofNodes, count, at)
}

/// Count repeated references once per owned occurrence before the first copy.
pub(crate) fn premises_bytes(
    cas: &Cas,
    proofs: &[LogicalId],
    budget: &mut Budget,
    at: Location,
) -> Result<u64, Fault> {
    let mut bytes = 0_u64;
    for proof in proofs {
        bytes = bytes
            .checked_add(application_bytes(cas, *proof, budget)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
    }
    Ok(bytes)
}

/// Prepay the new lattice proposition without extending the shared preflight dispatch.
fn lattice_selection_objects(
    value: &crate::lattice::LatticeSelection,
    budget: &mut Budget,
) -> Result<usize, Fault> {
    let bytes = crate::lattice::cover::check::payload(&value.cover, budget)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    Ok(value
        .cover
        .index
        .families
        .len()
        .saturating_mul(7)
        .saturating_add(8)
        .saturating_add(value.values.len()))
}

/// Share the cover's bounded role inventory without extending the application dispatcher.
fn lattice_cover_objects(
    value: &crate::lattice::LatticeCover,
    budget: &mut Budget,
) -> Result<usize, Fault> {
    let bytes = crate::lattice::cover::check::payload(value, budget)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    Ok(value
        .index
        .families
        .len()
        .saturating_mul(7)
        .saturating_add(7))
}

/// Bound the number and payload of a typed application before caller-owned copies.
pub(crate) fn measure(
    cas: &Cas,
    value: &ApplicationDraft,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::work_observation::observe!(budget, AT, {
        budget.check(BudgetKind::ProofNodes, size(value.premises.len(), AT)?, AT)?;
        cas.state
            .context(cas.checked(&value.context, budget)?, budget)?;
        crate::distribution_measure::measure(cas, &value.conclusion, &value.certificate, budget)?;
        if let Certificate::Series(certificate) = &value.certificate {
            crate::series::codec::measure(certificate, budget)?;
        }
        if let Certificate::LatticeSelection(certificate) = &value.certificate {
            crate::lattice::selection::wire::measure(cas, certificate, budget)?;
        }
        if let Certificate::IntegerSelection(certificate) = &value.certificate {
            crate::decision_integer_wire::measure(cas, certificate, budget)?;
        }
        if let Certificate::Decision(certificate) = &value.certificate {
            crate::decision_wire::measure(cas, certificate, budget)?;
        }
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
                cas.checked(object, budget)?;
            }
        }
        let objects = match &value.conclusion {
            Statement::LatticeSelection(value) | Statement::LatticeNearestSelection(value) => {
                lattice_selection_objects(value, budget)?
            }
            Statement::LatticeCover(value) => lattice_cover_objects(value, budget)?,
            Statement::IntegerSelection(value) => 7 + usize::from(value.family.count.is_some()),
            Statement::IntegerFamily(value) => 6 + usize::from(value.count.is_some()),
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
            cas.checked(object, budget)?;
        }
        if let Statement::Cover { members, .. } = &value.conclusion {
            let mut unique = std::collections::BTreeSet::new();
            for member in members {
                budget.condition(size(members.len(), AT)?, AT)?;
                if !unique.insert(cas.checked(member, budget)?) {
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
    })
}

/// Check the complete scope and layer before interpreting a caller's expected theorem.
pub(crate) fn validate_judgment(
    cas: &Cas,
    value: &Judgment,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::statement_check::validate(cas, &value.statement, budget)?;
    if let Statement::HalfPiWitness { context, .. } = &value.statement
        && *context != value.context
    {
        return Err(Fault::invalid(InvalidCode::Context, AT));
    }
    let context = cas
        .state
        .context(cas.checked(&value.context, budget)?, budget)?;
    if value.layer != crate::statement::layer(&value.statement)
        || value.space != crate::statement::space(&value.statement)
    {
        return Err(Fault::invalid(InvalidCode::Sort, AT));
    }
    for object in crate::statement::objects(&value.statement) {
        let header = match cas.state.node(cas.checked(object, budget)?, budget)? {
            Vertex::Solve(value) => {
                let Some((header, _)) = crate::solving::graph::metadata(&cas.state, value, budget)?
                else {
                    continue;
                };
                header
            }
            Vertex::SeriesObject(object) if matches!(value.statement, Statement::Series(_)) => {
                if object.context != value.context {
                    return Err(Fault::invalid(InvalidCode::Context, AT));
                }
                cas.state
                    .context(cas.checked(&object.context, budget)?, budget)?
                    .header_id
            }
            Vertex::Expression(value) => value.header,
            Vertex::Region(value) => value.header,
            Vertex::Boundary(value) => value.header,
            Vertex::CaseArm(value) => value.header,
            Vertex::SourceForm(value) => value.header,
            Vertex::Condition(value) => value.header,
            Vertex::Space(value) => value.header,
            Vertex::Context(frame)
                if matches!(
                    &value.statement,
                    Statement::LatticeCover(_)
                        | Statement::HalfPiWitness { .. }
                        | Statement::LatticeSelection(_)
                        | Statement::LatticeNearestSelection(_)
                ) =>
            {
                frame.header_id
            }
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
    if let Some(checked) = &value.scalar {
        prepay_scalar_copy(checked, budget)?;
        return Ok(checked.clone());
    }
    let claim = scalar_claim(cas, &value.judgment, budget)?;
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
            let guard = scalar_claim(cas, &view, budget)?
                .residuals
                .into_iter()
                .next()
                .ok_or_else(invalid)?;
            target.insert(crate::conditions::Obligation {
                kind: guard.kind,
                term: cas.checked(&guard.term, budget)?,
            });
        }
    }
    let context = cas.checked(&value.judgment.context, budget)?;
    for reference in &value.conditions.hypotheses {
        let frame = cas
            .state
            .context(cas.checked(&reference.context, budget)?, budget)?;
        if frame.parent.is_none() {
            let slot = usize::try_from(reference.slot).map_err(|_| invalid())?;
            conditions
                .hypotheses
                .insert(frame.assumptions.get(slot).ok_or_else(invalid)?.0.clone());
        } else {
            let introducing = cas.checked(&reference.context, budget)?;
            if reference.slot == 0
                || !crate::context_check::ancestor(cas, introducing, context, budget)?
            {
                return Err(invalid());
            }
            crate::context_check::predicate(cas, reference, budget)?;
            let bytes = size(conditions.qualified.len().saturating_add(1), AT)?
                .checked_mul(96)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
            budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
            budget.work(96, AT)?;
            conditions.qualified.insert((introducing, reference.slot));
        }
    }
    budget.condition(size(conditions.requirements.len(), AT)?, AT)?;
    Ok(crate::scalar_model::CheckedStep {
        primitive: value.rule,
        relation: claim.relation,
        context,
        left: cas.checked(&claim.left, budget)?,
        right: cas.checked(&claim.right, budget)?,
        premises: value.premises.clone(),
        path: match &value.certificate {
            Certificate::Congruence(path) => path.clone(),
            _ => Vec::new(),
        },
        substitutions: value
            .substitutions
            .iter()
            .map(|item| Ok((item.key.clone(), cas.checked(&item.replacement, budget)?)))
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
    let header = cas.handle(cas.state.context(value.context, budget)?.header_id);
    for required in &value.conditions.requirements {
        let scope = cas.state.expression(required.term, budget)?.scope.clone();
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
    while let Some(parent) = cas.state.context(root, budget)?.parent {
        budget.condition(1, AT)?;
        root = parent;
    }
    for key in &value.conditions.hypotheses {
        let slot = cas
            .state
            .context(root, budget)?
            .assumptions
            .iter()
            .position(|(name, _)| name == key)
            .ok_or_else(invalid)?;
        result.hypotheses.push(HypothesisRef {
            context: cas.handle(root),
            slot: u32::try_from(slot).map_err(|_| invalid())?,
        });
    }
    local_hypotheses(cas, value, &mut result, budget)?;
    Ok(result)
}

/// Retain checked local scalar provenance in the existing portable reference form.
fn local_hypotheses(
    cas: &Cas,
    value: &crate::scalar_model::CheckedStep,
    result: &mut ScopedConditions,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if !value.conditions.qualified.is_empty() {
        let bytes = size(value.conditions.qualified.len(), AT)?
            .checked_mul(96)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.work(bytes, AT)?;
        result
            .hypotheses
            .try_reserve_exact(value.conditions.qualified.len())
            .map_err(|_| Fault::allocation(AT))?;
    }
    for (frame, slot) in &value.conditions.qualified {
        budget.condition(1, AT)?;
        if *slot == 0 || !crate::context_check::ancestor(cas, *frame, value.context, budget)? {
            return Err(invalid());
        }
        let reference = HypothesisRef {
            context: cas.handle(*frame),
            slot: *slot,
        };
        crate::context_check::predicate(cas, &reference, budget)?;
        result.hypotheses.push(reference);
    }
    Ok(())
}

/// Transfer a sealed scalar producer result into the public scoped theorem vocabulary.
pub(crate) fn native_witness(
    cas: &Cas,
    value: crate::scalar_model::VerifiedWitness,
    budget: &mut Budget,
) -> Result<crate::proof_model::VerifiedWitness, Fault> {
    let application = application(cas, cas.checked(&value.proof, budget)?, budget)?;
    Ok(crate::proof_model::VerifiedWitness {
        proof: value.proof,
        judgment: application.judgment,
        hypotheses: application.conditions.hypotheses,
        bytes: value.bytes,
    })
}
