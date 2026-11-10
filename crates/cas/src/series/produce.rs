// SPDX-License-Identifier: AGPL-3.0-only
//! Finite candidate construction followed by the independent registered checker.

use super::check::{AT, invalid, number, unsupported};
use super::{
    SeriesBinaryRow, SeriesCertificate as C, SeriesObject, SeriesProductRow, SeriesStatement as S,
};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference, size};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, Certificate, VerifiedWitness};

/// A finite object operation; no variant denotes a source expansion or a centre change.
#[derive(Clone, Debug)]
pub enum SeriesOperation {
    /// Same-order rational finite inverse with no ordinary proof slots.
    Reciprocal {
        /// Original admitted object.
        input: Handle,
    },
    /// Formal derivative at exactly one fewer known coefficient; no ordinary proof slots.
    Derive {
        /// Original object of order at least two.
        input: Handle,
    },
    /// Formal integral at exactly one greater order, consuming Defined(C) in slot zero.
    Integrate {
        /// Original admitted object.
        input: Handle,
        /// Exact rational constant independent of the coordinate.
        constant: Handle,
    },
    /// Explicit positive restriction, including a checked equal-order restriction.
    Truncate {
        /// Original admitted finite object.
        input: Handle,
        /// Requested positive coefficient count.
        order: u32,
    },
    /// Coefficientwise sum; absent order records the minimum input order.
    Add {
        /// Original first object.
        left: Handle,
        /// Original second object.
        right: Handle,
        /// Explicit common count, or the recorded minimum default.
        order: Option<u32>,
    },
    /// Exact scalar multiplication, consuming the ordinary definedness proof at slot zero.
    Scale {
        /// Original admitted finite object.
        input: Handle,
        /// Exact rational with its ordinary definedness proof at slot zero.
        scalar: Handle,
    },
    /// Truncated convolution; absent order records the minimum input order.
    Mul {
        /// Original first object.
        left: Handle,
        /// Original second object.
        right: Handle,
        /// Explicit common count, or the recorded minimum default.
        order: Option<u32>,
    },
    /// Zero-constant composition; absent order records the minimum input order.
    Compose {
        /// Original first object.
        left: Handle,
        /// Original second object.
        right: Handle,
        /// Explicit common count, or the recorded minimum default.
        order: Option<u32>,
    },
    /// One exact coefficient, with bounds checked before indexing.
    Coefficient {
        /// Original admitted finite object.
        input: Handle,
        /// Zero-based coefficient position.
        index: u32,
    },
}

impl SeriesOperation {
    /// Original object fixes the retained semantic axes.
    const fn input(&self) -> &Handle {
        match self {
            Self::Reciprocal { input, .. }
            | Self::Truncate { input, .. }
            | Self::Derive { input, .. }
            | Self::Integrate { input, .. }
            | Self::Scale { input, .. }
            | Self::Coefficient { input, .. } => input,
            Self::Add { left, .. } | Self::Mul { left, .. } | Self::Compose { left, .. } => left,
        }
    }

    /// The second object contributes size bounds but never substitutes for a compatibility proof.
    const fn right(&self) -> Option<&Handle> {
        match self {
            Self::Add { right, .. } | Self::Mul { right, .. } | Self::Compose { right, .. } => {
                Some(right)
            }
            _ => None,
        }
    }

    /// Requested precision cannot invent coefficients beyond the admitted objects.
    fn order(&self, cas: &Cas, budget: &mut Budget) -> Result<u32, Fault> {
        let available = cas.series_object(self.input())?.order;
        let order = match self {
            Self::Derive { .. } => available
                .checked_sub(1)
                .filter(|n| *n > 0)
                .ok_or_else(unsupported)?,
            Self::Integrate { .. } => return available.checked_add(1).ok_or_else(unsupported),
            Self::Truncate { order, .. } => *order,
            Self::Add { right, order, .. }
            | Self::Mul { right, order, .. }
            | Self::Compose { right, order, .. } => {
                let common = available.min(cas.series_object(right)?.order);
                let chosen = order.unwrap_or(common);
                if chosen > common {
                    if matches!(self, Self::Compose { .. }) {
                        return Err(super::compose::precision(
                            available,
                            cas.series_object(right)?.order,
                            chosen,
                            budget,
                        )?);
                    }
                    return Err(unsupported());
                }
                chosen
            }
            Self::Coefficient { index, .. } => {
                if *index >= available {
                    return Err(unsupported());
                }
                available
            }
            Self::Reciprocal { .. } | Self::Scale { .. } => available,
        };
        if order == 0 || order > available {
            if let Self::Compose { right, .. } = self {
                return Err(super::compose::precision(
                    available,
                    cas.series_object(right)?.order,
                    order,
                    budget,
                )?);
            }
            return Err(unsupported());
        }
        Ok(order)
    }
}

/// Prepay every inspected entry and reserve simultaneous candidates before any rational copy.
fn workspace(
    cas: &Cas,
    operation: &SeriesOperation,
    order: u32,
    budget: &mut Budget,
) -> Result<(u64, u64), Fault> {
    let mut bits = 1_u64;
    let mut metadata = 0_u64;
    for handle in std::iter::once(operation.input()).chain(operation.right()) {
        let object = cas.series_object(handle)?;
        object.preflight(budget)?;
        let mut out = crate::encoding::Encoder::new(budget, AT);
        crate::encoding::scope(&object.scope, &mut out)?;
        metadata = metadata
            .checked_add(size(out.finish().len(), AT)?)
            .ok_or_else(invalid)?;
        for coefficient in &object.coefficients {
            budget.work(1, AT)?;
            let (a, b) = number(cas, coefficient)?.component_bits();
            bits = bits.max(a).max(b);
        }
    }
    if matches!(operation, SeriesOperation::Reciprocal { .. }) {
        let scope_work = metadata
            .checked_mul(u64::from(order) + 1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
        let bytes = super::reciprocal::workspace(order, budget)?
            .checked_add(
                metadata
                    .checked_mul(4)
                    .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
            )
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        return Ok((bytes, scope_work));
    }
    if let SeriesOperation::Scale { scalar, .. } = operation {
        budget.work(1, AT)?;
        let (a, b) = number(cas, scalar)?.component_bits();
        bits = bits.max(a).max(b);
    }
    if let SeriesOperation::Integrate { constant, .. } = operation {
        budget.work(1, AT)?;
        let (a, b) = number(cas, constant)?.component_bits();
        bits = bits.max(a).max(b);
    }
    if matches!(
        operation,
        SeriesOperation::Derive { .. } | SeriesOperation::Integrate { .. }
    ) {
        bits = bits.max(u64::from(order.bit_width()));
    }
    let n = u64::from(order);
    let rows = if matches!(operation, SeriesOperation::Compose { .. }) {
        super::compose::entries(order)?
    } else if matches!(operation, SeriesOperation::Mul { .. }) {
        n.checked_mul(n.checked_add(1).ok_or_else(invalid)?)
            .ok_or_else(invalid)?
            / 2
    } else {
        n
    };
    budget.check(BudgetKind::JetEntries, rows, AT)?;
    // Each partial sum contains at most n products; this bound includes unreduced denominators.
    if matches!(operation, SeriesOperation::Compose { .. }) {
        bits = bits
            .checked_mul(n)
            .and_then(|v| v.checked_mul(n))
            .ok_or_else(invalid)?;
    }
    let number_bytes = bits
        .checked_mul(n)
        .and_then(|v| v.checked_mul(4))
        .and_then(|v| v.checked_add(8))
        .ok_or_else(invalid)?;
    let bytes = rows
        .checked_add(n)
        .and_then(|v| v.checked_mul(number_bytes.checked_mul(16)?.checked_add(512)?))
        .and_then(|v| v.checked_add(metadata.checked_mul(4)?))
        .ok_or_else(invalid)?;
    budget.work(
        rows.checked_mul(96)
            .and_then(|v| v.checked_add(metadata.checked_mul(n.checked_add(1)?)?))
            .ok_or_else(invalid)?,
        AT,
    )?;
    Ok((bytes, 0))
}

/// Copy a rational only after charging its exact retained component extent.
pub(super) fn copied(value: &Rational, budget: &mut Budget) -> Result<Rational, Fault> {
    value.check(budget, AT)?;
    let (a, b) = value.component_bits();
    budget.work(
        a.checked_add(b)
            .and_then(|v| v.checked_add(7))
            .ok_or_else(invalid)?
            / 8
            + 1,
        AT,
    )?;
    Ok(value.clone())
}

/// Admit an output literal under the original checked declaration and binder frame.
pub(super) fn literal(
    cas: &mut Cas,
    input: &Handle,
    value: Rational,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let object = cas.series_object(input)?;
    let header = cas.state.context(cas.checked(&object.context)?)?.header_id;
    #[cfg(test)]
    SCOPE_COPIES.with(|copies| copies.set(copies.get() + 1));
    let scope = object.scope.clone();
    cas.admit(
        &[Draft::Expression(ExpressionDraft {
            header: Reference::Existing(cas.handle(header)),
            space: None,
            operator: Operator::Rational(value),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            scope,
            binding: None,
            operands: Vec::new(),
        })],
        budget,
    )?
    .pop()
    .ok_or_else(invalid)
}

/// The complete original metadata is copied only within the prepaid workspace.
pub(super) fn output(
    cas: &mut Cas,
    input: &Handle,
    coefficients: Vec<Handle>,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let basis = cas.series_object(input)?;
    #[cfg(test)]
    SCOPE_COPIES.with(|copies| copies.set(copies.get() + 1));
    let value = SeriesObject {
        field: Reference::Existing(basis.field.clone()),
        centre: Reference::Existing(basis.centre.clone()),
        variable: Reference::Existing(basis.variable.clone()),
        scope: basis.scope.clone(),
        context: Reference::Existing(basis.context.clone()),
        order: u32::try_from(coefficients.len()).map_err(|_| invalid())?,
        coefficients: coefficients.into_iter().map(Reference::Existing).collect(),
    };
    cas.admit(&[Draft::SeriesObject(value)], budget)?
        .pop()
        .ok_or_else(invalid)
}

/// The candidate recurrence is separate from the checker and retains every explicit partial row.
#[expect(
    clippy::too_many_lines,
    reason = "The closed finite-operation dispatcher retains candidate and certificate construction together"
)]
fn candidate(
    cas: &mut Cas,
    operation: &SeriesOperation,
    order: u32,
    scope_work: u64,
    budget: &mut Budget,
) -> Result<(S, C), Fault> {
    match operation {
        SeriesOperation::Reciprocal { input } => {
            return super::reciprocal::produce(cas, input, scope_work, budget);
        }
        SeriesOperation::Compose {
            left,
            right,
            order: requested,
        } => {
            return super::compose::produce(cas, left, right, order, requested.is_none(), budget);
        }
        SeriesOperation::Derive { input } => {
            return super::calculus::derive(cas, input, order, budget);
        }
        SeriesOperation::Integrate { input, constant } => {
            return super::calculus::integrate(cas, input, constant, order, budget);
        }
        _ => {}
    }
    if let SeriesOperation::Coefficient { input, index } = operation {
        return extracted(cas, input, *index, budget);
    }
    let mut values = Vec::new();
    let mut binary = Vec::new();
    let mut products = Vec::new();
    let mut retained = Vec::new();
    let n = usize::try_from(order).map_err(|_| invalid())?;
    values
        .try_reserve_exact(n)
        .map_err(|_| Fault::allocation(AT))?;
    if matches!(operation, SeriesOperation::Mul { .. }) {
        let rows = n
            .checked_mul(n.checked_add(1).ok_or_else(invalid)?)
            .ok_or_else(invalid)?
            / 2;
        products
            .try_reserve_exact(rows)
            .map_err(|_| Fault::allocation(AT))?;
    } else if matches!(operation, SeriesOperation::Truncate { .. }) {
        retained
            .try_reserve_exact(n)
            .map_err(|_| Fault::allocation(AT))?;
    } else {
        binary
            .try_reserve_exact(n)
            .map_err(|_| Fault::allocation(AT))?;
    }
    for degree in 0..order {
        let value = row(cas, operation, degree, &mut binary, &mut products, budget)?;
        if matches!(operation, SeriesOperation::Truncate { .. }) {
            retained.push(copied(&value, budget)?);
        }
        values.push(literal(cas, operation.input(), value, budget)?);
    }
    let output = output(cas, operation.input(), values, budget)?;
    Ok(match operation {
        SeriesOperation::Truncate { input, .. } => (
            S::Truncate {
                input: input.clone(),
                output,
            },
            C::Truncate {
                form: 0,
                coefficients: retained,
            },
        ),
        SeriesOperation::Scale { input, scalar } => (
            S::Scale {
                input: input.clone(),
                scalar: scalar.clone(),
                output,
            },
            C::Scale {
                form: 0,
                rows: binary,
            },
        ),
        SeriesOperation::Add {
            left,
            right,
            order: requested,
        } => (
            S::Add {
                left: left.clone(),
                right: right.clone(),
                output,
                order,
                default_order: requested.is_none(),
            },
            C::Add {
                form: 0,
                order,
                default_order: requested.is_none(),
                rows: binary,
            },
        ),
        SeriesOperation::Mul {
            left,
            right,
            order: requested,
        } => (
            S::Mul {
                left: left.clone(),
                right: right.clone(),
                output,
                order,
                default_order: requested.is_none(),
            },
            C::Mul {
                form: 0,
                order,
                default_order: requested.is_none(),
                rows: products,
            },
        ),
        SeriesOperation::Reciprocal { .. }
        | SeriesOperation::Coefficient { .. }
        | SeriesOperation::Compose { .. }
        | SeriesOperation::Derive { .. }
        | SeriesOperation::Integrate { .. } => return Err(invalid()),
    })
}

/// Extraction copies only the exact requested rational under the prepaid candidate workspace.
fn extracted(cas: &Cas, input: &Handle, index: u32, budget: &mut Budget) -> Result<(S, C), Fault> {
    let handle = cas
        .series_object(input)?
        .coefficients
        .get(usize::try_from(index).map_err(|_| invalid())?)
        .ok_or_else(invalid)?
        .clone();
    let value = copied(number(cas, &handle)?, budget)?;
    Ok((
        S::Coefficient {
            input: input.clone(),
            index,
            value: handle,
        },
        C::Coefficient {
            form: 0,
            index,
            value,
        },
    ))
}

/// Borrow an original coefficient within the already validated positive truncation.
pub(super) fn entry<'a>(cas: &'a Cas, input: &Handle, index: u32) -> Result<&'a Rational, Fault> {
    number(
        cas,
        cas.series_object(input)?
            .coefficients
            .get(usize::try_from(index).map_err(|_| invalid())?)
            .ok_or_else(invalid)?,
    )
}

/// Generate the complete row, including every zero term in a convolution.
fn row(
    cas: &Cas,
    operation: &SeriesOperation,
    degree: u32,
    binary: &mut Vec<SeriesBinaryRow>,
    products: &mut Vec<SeriesProductRow>,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    match operation {
        SeriesOperation::Truncate { input, .. } => copied(entry(cas, input, degree)?, budget),
        SeriesOperation::Scale { input, scalar } => {
            let a = number(cas, scalar)?;
            let b = entry(cas, input, degree)?;
            let result = a.mul(b, budget)?;
            binary.push(SeriesBinaryRow {
                index: degree,
                left: copied(a, budget)?,
                right: copied(b, budget)?,
                result: copied(&result, budget)?,
            });
            Ok(result)
        }
        SeriesOperation::Add { left, right, .. } => {
            let a = entry(cas, left, degree)?;
            let b = entry(cas, right, degree)?;
            let result = a.add(b, budget)?;
            binary.push(SeriesBinaryRow {
                index: degree,
                left: copied(a, budget)?,
                right: copied(b, budget)?,
                result: copied(&result, budget)?,
            });
            Ok(result)
        }
        SeriesOperation::Mul { left, right, .. } => {
            let mut sum = Rational::zero();
            for i in 0..=degree {
                let j = degree - i;
                let a = entry(cas, left, i)?;
                let b = entry(cas, right, j)?;
                let product = a.mul(b, budget)?;
                sum = sum.add(&product, budget)?;
                products.push(SeriesProductRow {
                    degree,
                    left_index: i,
                    right_index: j,
                    left: copied(a, budget)?,
                    right: copied(b, budget)?,
                    product,
                    sum: copied(&sum, budget)?,
                });
            }
            Ok(sum)
        }
        SeriesOperation::Reciprocal { .. }
        | SeriesOperation::Coefficient { .. }
        | SeriesOperation::Compose { .. }
        | SeriesOperation::Derive { .. }
        | SeriesOperation::Integrate { .. } => Err(invalid()),
    }
}

impl Cas {
    /// Construct finite candidates and replay them through the registered independent checker.
    ///
    /// Scale and integration consume one ordinary scalar-definedness proof. Derivation consumes
    /// no ordinary proofs. Binary operations consume ordered explicit truncation proofs for
    /// higher-order inputs. Output syntax is admitted atomically.
    /// # Errors
    /// Returns typed unsupported capability for unavailable precision, invalid evidence for false
    /// premise joins, and typed exhaustion without publishing a partial object or judgment.
    pub fn compute_series(
        &mut self,
        operation: &SeriesOperation,
        proofs: &[Handle],
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                budget.work(1, AT)?;
                let order = operation.order(cas, budget)?;
                let count = match operation {
                    SeriesOperation::Reciprocal { .. }
                    | SeriesOperation::Truncate { .. }
                    | SeriesOperation::Coefficient { .. }
                    | SeriesOperation::Derive { .. } => 0,
                    SeriesOperation::Scale { .. } | SeriesOperation::Integrate { .. } => 1,
                    SeriesOperation::Add { left, right, .. }
                    | SeriesOperation::Compose { left, right, .. }
                    | SeriesOperation::Mul { left, right, .. } => {
                        usize::from(cas.series_object(left)?.order != order)
                            + usize::from(cas.series_object(right)?.order != order)
                    }
                };
                if proofs.len() != count {
                    return Err(invalid());
                }
                if matches!(
                    operation,
                    SeriesOperation::Derive { .. } | SeriesOperation::Integrate { .. }
                ) {
                    budget.check(BudgetKind::JetOrder, u64::from(order - 1), AT)?;
                    budget.check(BudgetKind::JetEntries, u64::from(order), AT)?;
                }
                if let SeriesOperation::Integrate { input, constant } = operation {
                    super::check::constant(cas, cas.series_object(input)?, constant, budget)?;
                }
                if let SeriesOperation::Reciprocal { input } = operation {
                    super::reciprocal::nonunit(cas, input, budget)?;
                }
                let (mut bytes, scope_work) = workspace(cas, operation, order, budget)?;
                for proof in proofs {
                    bytes = bytes
                        .checked_add(crate::proof_view::application_bytes(
                            cas,
                            cas.checked(proof)?,
                            budget,
                        )?)
                        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
                }
                // All owned premise payloads coexist with candidate, checking and publication scratch.
                // Keep their aggregate reservation until the copies leave this closure.
                budget.with_retained_payload(bytes, AT, |budget| {
                    let mut premises = Vec::new();
                    for proof in proofs {
                        premises.push(crate::proof_view::application(
                            cas,
                            cas.checked(proof)?,
                            budget,
                        )?);
                    }
                    let retained =
                        cas.judgment_reservation(cas.retained_judgment_bytes(), budget, AT)?;
                    let (statement, certificate) =
                        budget.with_retained_payload(retained, AT, |budget| {
                            candidate(cas, operation, order, scope_work, budget)
                        })?;
                    let draft = ApplicationDraft {
                        rule: statement.primitive(),
                        context: cas.series_object(operation.input())?.context.clone(),
                        conclusion: crate::proof_model::Statement::Series(Box::new(statement)),
                        premises: Vec::new(),
                        substitutions: Vec::new(),
                        certificate: Certificate::Series(Box::new(certificate)),
                    };
                    let mut checked =
                        crate::scoped_check::check_application(cas, &draft, &premises, budget)?;
                    checked.premises = proofs.iter().map(Handle::logical_id).collect();
                    let proof = cas.publish_scoped(checked, budget)?;
                    cas.seal_scoped(&proof, budget)
                })
            })
        })
    }
}

#[cfg(test)]
thread_local! {
    static SCOPE_COPIES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod reciprocal_scope_tests {
    use super::*;
    use crate::budget::Limits;
    use crate::error::{Exhaustion, FaultKind};
    use crate::testing::common::{TestResult, default_budget};
    use crate::testing::series_residual::{Fixture, admit};

    /// Scope copies are observed at the actual literal/object clone sites.
    /// ´test:unit:series-reciprocal-scope-copy-prepayment´
    #[test]
    fn reciprocal_scope_copy_prepayment() -> TestResult {
        use crate::test_api::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
        for width in [1, 1024] {
            let mut f = Fixture::new(&["1"], "0", false, false)?;
            let scope = vec![vec![AtomicSort::Scalar(Field::Real); width]];
            let region = admit(
                &mut f.cas,
                Draft::Region(RegionDraft {
                    header: Reference::Existing(f.header.clone()),
                    scope: scope.clone(),
                    ambient: AmbientDomain::Real("x".into()),
                    expression: RegionExpr::Ambient,
                }),
            )?;
            let context = admit(
                &mut f.cas,
                Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(f.header.clone()),
                    admissible: Reference::Existing(region),
                    assumptions: Vec::new(),
                }),
            )?;
            let mut nodes = Vec::new();
            for operator in [
                Operator::Free("x".into()),
                Operator::Rational(Rational::zero()),
                Operator::Rational(Rational::one()),
            ] {
                nodes.push(admit(
                    &mut f.cas,
                    Draft::Expression(ExpressionDraft {
                        header: Reference::Existing(f.header.clone()),
                        space: None,
                        operator,
                        sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                        scope: scope.clone(),
                        binding: None,
                        operands: Vec::new(),
                    }),
                )?);
            }
            let input = admit(
                &mut f.cas,
                Draft::SeriesObject(SeriesObject {
                    field: Reference::Existing(f.field.clone()),
                    centre: Reference::Existing(nodes[1].clone()),
                    variable: Reference::Existing(nodes[0].clone()),
                    scope,
                    context: Reference::Existing(context),
                    order: 1,
                    coefficients: vec![Reference::Existing(nodes[2].clone())],
                }),
            )?;
            let operation = SeriesOperation::Reciprocal { input };
            let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
            // Measured at the actual output-construction boundary on the frozen compiler.
            let limit = if width == 1 { 1614 } else { 9798 };
            let mut budget = Budget::new(Limits {
                work: limit,
                ..Limits::default()
            })?;
            SCOPE_COPIES.with(|copies| copies.set(0));
            let fault = f
                .cas
                .compute_series(&operation, &[], &mut budget)
                .expect_err("scope prepayment");
            assert_eq!(SCOPE_COPIES.with(std::cell::Cell::get), 0);
            assert_eq!(budget.usage().work, if width == 1 { 1595 } else { 5687 });
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: Exhaustion::Limit,
                    budget: BudgetKind::Work,
                    limit,
                    needed: limit + 1
                }
            );
            assert_eq!(fault.location, AT);
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
            f.cas
                .compute_series(&operation, &[], &mut default_budget()?)?;
            assert_eq!(SCOPE_COPIES.with(std::cell::Cell::get), 2);
        }
        Ok(())
    }
}
