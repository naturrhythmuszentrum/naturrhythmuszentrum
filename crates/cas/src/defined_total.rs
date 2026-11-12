// SPDX-License-Identifier: AGPL-3.0-only
//! Premise-free definedness by induction over the original closed numeric syntax.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::Layer;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Expression, Handle, LogicalId, Vertex, size};
use crate::model::{AtomicSort, Branch, Constant, Field, Operator, Sort};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement, VerifiedWitness,
};
use crate::rule::Primitive;

/// Structural induction and exact-frame accounting location.
const AT: Location = Location::new("structural totality");

/// Preserve the typed refusal category at this checker boundary.
const fn invalid(code: InvalidCode) -> Fault {
    Fault::invalid(code, AT)
}

/// The closed value sorts interpreted by structural induction.
const fn supported(sort: &Sort) -> bool {
    matches!(
        sort,
        Sort::Value(
            AtomicSort::Integer
                | AtomicSort::Scalar(Field::Rational | Field::Real | Field::Complex)
        )
    )
}

/// Exact frames are stronger than the generic conservative-header embedding check.
fn frame(
    cas: &Cas,
    function: LogicalId,
    context: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let value = cas.state.expression(function)?;
    let context = cas.state.context(cas.checked(context)?)?;
    let region = cas.state.region(context.admissible)?;
    crate::validation::scope_limit(&value.scope, budget)?;
    crate::rule_check::prepay_shape(value, budget)?;
    budget.work(5, AT)?;
    if value.header != context.header_id || region.header != context.header_id {
        return Err(invalid(InvalidCode::Context));
    }
    if value.scope != region.scope {
        return Err(invalid(InvalidCode::Scope));
    }
    let environment = &cas.state.header(value.header)?.environment;
    if !matches!(
        environment.field,
        Field::Rational | Field::Real | Field::Complex
    ) || !matches!(
        environment.branch,
        Branch::StrictReal | Branch::PrincipalComplex
    ) {
        return Err(invalid(InvalidCode::UnsupportedFragment));
    }
    Ok(())
}

/// Recheck declaration identities and retained binder slots, without interpreting display names.
fn leaf(cas: &Cas, value: &Expression, budget: &mut Budget) -> Result<(), Fault> {
    budget.work(1, AT)?;
    let valid = match &value.operator {
        Operator::Rational(number) => {
            value.sort != Sort::Value(AtomicSort::Integer) || number.is_integer()
        }
        Operator::Constant(Constant::Pi | Constant::E) => matches!(
            value.sort,
            Sort::Value(AtomicSort::Scalar(Field::Real | Field::Complex))
        ),
        Operator::Free(key) => {
            let declarations = &cas.state.header(value.header)?.environment.declarations;
            let index = crate::header::declaration_index(declarations, key, budget, AT)?;
            index
                .and_then(|index| declarations.get(index))
                .is_some_and(|declaration| declaration.sort == value.sort)
        }
        Operator::Bound { depth, slot } => {
            let depth = usize::try_from(*depth).map_err(|_| invalid(InvalidCode::Scope))?;
            let slot = usize::try_from(*slot).map_err(|_| invalid(InvalidCode::Scope))?;
            let actual = value
                .scope
                .len()
                .checked_sub(depth)
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| value.scope.get(index))
                .and_then(|frame| frame.get(slot));
            matches!((&value.sort, actual), (Sort::Value(expected), Some(actual)) if expected == actual)
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid(InvalidCode::UnsupportedFragment))
    }
}

/// Only the original literal denominator supplies a nonzero unit in this fragment.
fn constructor(cas: &Cas, value: &Expression, budget: &mut Budget) -> Result<(), Fault> {
    budget.work(1, AT)?;
    match (&value.operator, value.operands.as_slice()) {
        (
            Operator::Rational(_)
            | Operator::Constant(_)
            | Operator::Free(_)
            | Operator::Bound { .. },
            [],
        ) => leaf(cas, value, budget),
        (Operator::Neg, [_]) | (Operator::Add | Operator::Sub | Operator::Mul, [_, _]) => Ok(()),
        (Operator::IntegerPower(exponent), [_]) if *exponent > 0 => Ok(()),
        (Operator::IntegerEmbed, [child]) => {
            if cas.state.expression(*child)?.sort != Sort::Value(AtomicSort::Integer)
                || !matches!(value.sort, Sort::Value(AtomicSort::Scalar(_)))
            {
                return Err(invalid(InvalidCode::Sort));
            }
            Ok(())
        }
        (Operator::Div, [_, denominator]) => {
            let denominator = cas.state.expression(*denominator)?;
            if !matches!(value.sort, Sort::Value(AtomicSort::Scalar(_))) {
                return Err(invalid(InvalidCode::UnsupportedFragment));
            }
            let Operator::Rational(number) = &denominator.operator else {
                return Err(invalid(InvalidCode::UnsupportedFragment));
            };
            if !denominator.operands.is_empty() {
                return Err(invalid(InvalidCode::UnsupportedFragment));
            }
            if number.is_zero() {
                Err(invalid(InvalidCode::RefutedDomain))
            } else {
                Ok(())
            }
        }
        _ => Err(invalid(InvalidCode::UnsupportedFragment)),
    }
}

/// Every occurrence is visited; a repeated child never gains a memoized totality flag.
fn walk(cas: &Cas, function: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
    let root = cas.state.expression(function)?;
    budget.check(BudgetKind::Depth, root.depth, AT)?;
    let capacity =
        usize::try_from(root.depth).map_err(|_| Fault::capacity(BudgetKind::Depth, AT))?;
    let bytes = root
        .depth
        .checked_mul(size(std::mem::size_of::<(LogicalId, usize)>(), AT)?)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.with_retained_payload(bytes, AT, |budget| {
        let mut stack = Vec::new();
        stack
            .try_reserve_exact(capacity)
            .map_err(|_| Fault::allocation(AT))?;
        budget.work(size(std::mem::size_of::<(LogicalId, usize)>(), AT)?, AT)?;
        stack.push((function, 0_usize));
        while let Some((id, next)) = stack.last_mut() {
            let value = cas.state.expression(*id)?;
            if *next == 0 {
                budget.condition(1, AT)?;
                crate::rule_check::prepay_shape(value, budget)?;
                budget.work(8, AT)?;
                if value.space.is_some()
                    || value.binding.is_some()
                    || value.attachment.is_some()
                    || value.layer != Layer::Pointwise
                    || !supported(&value.sort)
                    || !matches!(
                        value.operator,
                        Operator::Rational(_)
                            | Operator::Constant(_)
                            | Operator::Free(_)
                            | Operator::Bound { .. }
                            | Operator::Neg
                            | Operator::Add
                            | Operator::Sub
                            | Operator::Mul
                            | Operator::Div
                            | Operator::IntegerEmbed
                            | Operator::IntegerPower(_)
                    )
                {
                    return Err(invalid(InvalidCode::UnsupportedFragment));
                }
                if value.header != root.header {
                    return Err(invalid(InvalidCode::Context));
                }
                if value.scope != root.scope {
                    return Err(invalid(InvalidCode::Scope));
                }
            }
            if let Some(child) = value.operands.get(*next) {
                budget.work(3 + size(std::mem::size_of::<(LogicalId, usize)>(), AT)?, AT)?;
                let child_value = cas.state.expression(*child)?;
                let same_sort_required = value.operator != Operator::IntegerEmbed;
                let different_sort = child_value.sort != value.sort;
                if same_sort_required && different_sort {
                    return Err(invalid(InvalidCode::Sort));
                }
                *next = next
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Edges, AT))?;
                budget.check(
                    BudgetKind::Depth,
                    size(stack.len(), AT)?
                        .checked_add(1)
                        .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?,
                    AT,
                )?;
                stack.push((*child, 0));
            } else {
                constructor(cas, value, budget)?;
                stack.pop();
            }
        }
        Ok(())
    })
}

/// The ordinary supplied checker and portable importer both repeat this structural induction.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if draft.certificate != Certificate::None
        || !draft.substitutions.is_empty()
        || !draft.premises.is_empty()
        || !premises.is_empty()
    {
        return Err(invalid(InvalidCode::Rule));
    }
    let Statement::PredicateFact { region, predicate } = &draft.conclusion else {
        return Err(invalid(InvalidCode::Rule));
    };
    let context = cas.state.context(cas.checked(&draft.context)?)?;
    budget.work(5, AT)?;
    if cas.checked(region)? != context.admissible {
        return Err(invalid(InvalidCode::Rule));
    }
    let Vertex::Condition(condition) = cas.state.node(cas.checked(predicate)?)? else {
        return Err(invalid(InvalidCode::Rule));
    };
    if condition.kind != crate::model::PredicateKind::Defined {
        return Err(invalid(InvalidCode::Rule));
    }
    let [function] = condition.arguments.as_slice() else {
        return Err(invalid(InvalidCode::Rule));
    };
    frame(cas, *function, &draft.context, budget)?;
    let value = cas.state.expression(*function)?;
    if condition.header != value.header {
        return Err(invalid(InvalidCode::Context));
    }
    if condition.scope != value.scope {
        return Err(invalid(InvalidCode::Scope));
    }
    walk(cas, *function, budget)?;
    Ok(ScopedConditions::default())
}

impl Cas {
    /// Prove structural definedness on exactly the context's admissible region, without premises.
    ///
    /// The closed numeric grammar retains every original operand and admits division only by
    /// a nonzero rational literal. No region decision or assumption supplies evidence.
    /// # Errors
    /// Returns typed frame faults, unsupported syntax, a refuted literal-zero denominator,
    /// or exhaustion of the caller's cumulative ledger. Failure publishes no partial result.
    pub fn defined_total(
        &mut self,
        function: &Handle,
        context: &Handle,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                frame(cas, cas.checked(function)?, context, budget)?;
                let pending = std::mem::size_of::<ApplicationDraft>()
                    + std::mem::size_of::<crate::graph::Draft>()
                    + std::mem::size_of::<crate::graph::Reference>();
                budget.with_retained_payload(size(pending, AT)?, AT, |budget| {
                    budget.work(size(pending, AT)?, AT)?;
                    let region = cas.handle(cas.state.context(cas.checked(context)?)?.admissible);
                    let predicate = crate::trig::defined::predicate(cas, function, budget)?;
                    let draft = ApplicationDraft {
                        rule: Primitive::DefinedTotal,
                        context: context.clone(),
                        conclusion: Statement::PredicateFact { region, predicate },
                        premises: Vec::new(),
                        substitutions: Vec::new(),
                        certificate: Certificate::None,
                    };
                    let checked = crate::scoped_check::check_application(cas, &draft, &[], budget)?;
                    let proof = cas.publish_scoped(checked, budget)?;
                    cas.seal_scoped(&proof, budget)
                })
            })
        })
    }
}
