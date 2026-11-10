// SPDX-License-Identifier: AGPL-3.0-only
//! Independent same-order identity convolution and untrusted inverse recurrence.

use super::check::{AT, coefficient, compatible, equal, invalid};
use super::{SeriesCertificate as C, SeriesProductRow, SeriesStatement as S};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, size};
use crate::number::Rational;
use crate::proof_model::CheckedApplication;

#[cfg(test)]
thread_local! {
    static VISITS: std::cell::Cell<[u64; 3]> = const { std::cell::Cell::new([0; 3]) };
}

/// Phase-local observation attaches to the actual borrowed read, after its charge.
#[cfg(test)]
fn visited(phase: usize) {
    VISITS.with(|cell| {
        let mut values = cell.get();
        values[phase] += 1;
        cell.set(values);
    });
}

#[cfg(not(test))]
/// Observation has no production effect or runtime allocation.
const fn visited(_phase: usize) {}

/// This boundary deliberately keeps the charge and actual coefficient visit adjacent.
fn read<'a>(
    cas: &'a Cas,
    object: &super::SeriesObject<Handle>,
    index: u32,
    phase: usize,
    budget: &mut Budget,
) -> Result<&'a Rational, Fault> {
    if index >= object.order {
        return Err(invalid());
    }
    budget.specialized(BudgetKind::JetEntries, 1, true, AT)?;
    visited(phase);
    super::check::number(
        cas,
        object
            .coefficients
            .get(usize::try_from(index).map_err(|_| invalid())?)
            .ok_or_else(invalid)?,
    )
}

/// Reserve the entire shape scan before visiting even its first row.
fn shape(rows: &[SeriesProductRow], order: u32, budget: &mut Budget) -> Result<(), Fault> {
    budget.work(size(rows.len(), AT)?, AT)?;
    let mut supplied = rows.iter();
    for degree in 0..order {
        for i in 0..=degree {
            visited(0);
            let row = supplied.next().ok_or_else(invalid)?;
            if (row.degree, row.left_index, row.right_index) != (degree, i, degree - i) {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Validate both the mathematical count and the portable/container capacities before allocation.
pub(super) fn entries(order: u32) -> Result<u64, Fault> {
    let n = u64::from(order);
    let count = n
        .checked_mul(n.checked_add(1).ok_or_else(invalid)?)
        .ok_or_else(invalid)?
        / 2;
    if order == 0 {
        return Err(invalid());
    }
    u32::try_from(count).map_err(|_| invalid())?;
    let capacity = usize::try_from(count).map_err(|_| invalid())?;
    let bytes = capacity
        .checked_mul(std::mem::size_of::<SeriesProductRow>())
        .ok_or_else(invalid)?;
    isize::try_from(bytes).map_err(|_| invalid())?;
    Ok(count)
}

/// Reserve candidate coefficients, complete rows, units and independent arithmetic temporaries.
pub(super) fn workspace(order: u32, budget: &Budget) -> Result<u64, Fault> {
    let count = entries(order)?;
    budget.check(BudgetKind::JetEntries, count, AT)?;
    let bits = budget
        .limits()
        .component_bits
        .max(budget.limits().intermediate_bits);
    let cell = bits
        .checked_add(7)
        .map(|n| n / 8)
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(128))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    count
        .checked_mul(4)
        .and_then(|n| n.checked_add(u64::from(order).checked_mul(2)?))
        .and_then(|n| n.checked_add(12))
        .and_then(|n| n.checked_mul(cell))
        .and_then(|n| n.checked_add(count.checked_mul(256)?))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))
}

/// Observe the admitted constant before reading any claimed inverse or allocating rows.
pub(super) fn nonunit(cas: &Cas, input: &Handle, budget: &mut Budget) -> Result<(), Fault> {
    let object = cas.series_object(input)?;
    let number = coefficient(cas, object, 0, budget)?;
    if number.is_zero() {
        let bytes = 257 + 2 * std::mem::size_of::<usize>() as u64;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        return budget.with_retained_payload(bytes, AT, |budget| {
            budget.work(bytes, AT)?;
            let value = super::produce::copied(number, budget)?;
            let [input, coefficient] =
                crate::error::SeriesDiagnosticIdentity::pair(input, &object.coefficients[0]);
            let mut fault = Fault::invalid(InvalidCode::SeriesNonunitConstant, AT);
            fault.series_evidence = Some(std::sync::Arc::new(
                crate::error::SeriesRefusalEvidence::Constant(
                    crate::error::SeriesConstantEvidence {
                        input,
                        coefficient,
                        index: 0,
                        value,
                    },
                ),
            ));
            Err(fault)
        });
    }
    Ok(())
}

/// Rebuild every identity coefficient without trusting certificate partial sums.
pub(super) fn check(
    cas: &Cas,
    statement: &S,
    certificate: &C,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let S::Reciprocal { input, output } = statement else {
        return Err(invalid());
    };
    let C::Reciprocal {
        constant,
        inverse,
        rows,
        ..
    } = certificate
    else {
        return Err(invalid());
    };
    let p = cas.series_object(input)?;
    let q = cas.series_object(output)?;
    compatible(p, q, budget)?;
    if !premises.is_empty()
        || p.order == 0
        || p.order != q.order
        || size(rows.len(), AT)? != entries(p.order)?
    {
        return Err(invalid());
    }
    shape(rows, p.order, budget)?;
    let p0 = read(cas, p, 0, 1, budget)?;
    equal(constant, p0, budget)?;
    equal(&p0.mul(inverse, budget)?, &Rational::one(), budget)?;
    equal(inverse, read(cas, q, 0, 1, budget)?, budget)?;
    let mut supplied = rows.iter();
    for degree in 0..p.order {
        let mut sum = Rational::zero();
        for i in 0..=degree {
            budget.work(1, AT)?;
            let row = supplied.next().ok_or_else(invalid)?;
            if (row.degree, row.left_index, row.right_index) != (degree, i, degree - i) {
                return Err(invalid());
            }
            let a = read(cas, p, i, 2, budget)?;
            let b = read(cas, q, degree - i, 2, budget)?;
            equal(&row.left, a, budget)?;
            equal(&row.right, b, budget)?;
            let product = a.mul(b, budget)?;
            equal(&row.product, &product, budget)?;
            sum = sum.add(&product, budget)?;
            equal(&row.sum, &sum, budget)?;
        }
        equal(
            &sum,
            &if degree == 0 {
                Rational::one()
            } else {
                Rational::zero()
            },
            budget,
        )?;
    }
    Ok(())
}

/// Allocate only beneath the enclosing reservation after portable count validation.
fn vector<T>(count: u64) -> Result<Vec<T>, Fault> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(usize::try_from(count).map_err(|_| invalid())?)
        .map_err(|_| Fault::allocation(AT))?;
    Ok(result)
}

/// The recurrence supplies a candidate; the independent checker establishes the identity.
pub(super) fn produce(
    cas: &mut Cas,
    input: &Handle,
    scope_work: u64,
    budget: &mut Budget,
) -> Result<(S, C), Fault> {
    use super::produce::{copied, literal, output};
    let p = cas.series_object(input)?;
    let order = p.order;
    let count = entries(order)?;
    let constant = copied(coefficient(cas, p, 0, budget)?, budget)?;
    let inverse = Rational::one().div(&constant, budget)?;
    let mut values: Vec<Rational> = vector(u64::from(order))?;
    values.push(copied(&inverse, budget)?);
    for degree in 1..order {
        let mut sum = Rational::zero();
        for i in 1..=degree {
            let a = coefficient(cas, p, i, budget)?;
            let b = values
                .get(usize::try_from(degree - i).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            sum = sum.add(&a.mul(b, budget)?, budget)?;
        }
        values.push(inverse.mul(&sum, budget)?.negated(budget)?);
    }
    let mut rows = vector(count)?;
    for degree in 0..order {
        let mut sum = Rational::zero();
        for i in 0..=degree {
            let a = coefficient(cas, p, i, budget)?;
            let b = values
                .get(usize::try_from(degree - i).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            let product = a.mul(b, budget)?;
            sum = sum.add(&product, budget)?;
            rows.push(SeriesProductRow {
                degree,
                left_index: i,
                right_index: degree - i,
                left: copied(a, budget)?,
                right: copied(b, budget)?,
                product,
                sum: copied(&sum, budget)?,
            });
        }
    }
    // The workspace measured this exact scope once: n literals and one object copy it.
    budget.work(scope_work, AT)?;
    let mut coefficients = vector(u64::from(order))?;
    for value in values {
        coefficients.push(literal(cas, input, value, budget)?);
    }
    let output = output(cas, input, coefficients, budget)?;
    Ok((
        S::Reciprocal {
            input: input.clone(),
            output,
        },
        C::Reciprocal {
            form: 0,
            constant,
            inverse,
            rows,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Limits;
    use crate::error::FaultKind;
    use crate::testing::common::{TestResult, default_budget};
    use crate::testing::series_residual::Fixture;

    /// A representational workspace overflow is not false supplied evidence.
    /// ´test:unit:series-reciprocal-workspace-capacity´
    #[test]
    fn reciprocal_workspace_capacity() -> TestResult {
        use super::super::SeriesOperation;
        use crate::proof_model::{ApplicationDraft, Certificate, Statement};
        use crate::rule::Primitive;
        let mut f = Fixture::new(&["1"], "0", false, false)?;
        // Public supplied checking intersects caller and permanent store ceilings.
        f.cas.limits.intermediate_bits = u64::MAX;
        let input = f.object(&[Rational::one()])?;
        let mut step = ApplicationDraft {
            rule: Primitive::SeriesReciprocal,
            context: f.context.clone(),
            conclusion: Statement::Series(Box::new(S::Reciprocal {
                input: input.clone(),
                output: input.clone(),
            })),
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Series(Box::new(C::Reciprocal {
                form: 0,
                constant: Rational::one(),
                inverse: Rational::one(),
                rows: vec![SeriesProductRow {
                    degree: 0,
                    left_index: 0,
                    right_index: 0,
                    left: Rational::one(),
                    right: Rational::one(),
                    product: Rational::one(),
                    sum: Rational::one(),
                }],
            })),
        };
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        // For order one A=18*cell+256. The last case fits the product but not
        // the addition: cell=floor(MAX/18), with cell=bits/4+128 here.
        for bits in [u64::MAX, u64::MAX - 7, (u64::MAX / 18 - 128) * 4] {
            let budget = Budget::new(Limits {
                intermediate_bits: bits,
                ..Limits::default()
            })?;
            assert_eq!(
                workspace(1, &budget).expect_err("workspace capacity").kind,
                FaultKind::Exhausted {
                    reason: crate::error::Exhaustion::Capacity,
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: u64::MAX,
                    needed: u64::MAX
                }
            );
        }
        for supplied in [false, true] {
            let mut budget = Budget::new(Limits {
                intermediate_bits: u64::MAX,
                ..Limits::default()
            })?;
            let result = if supplied {
                f.cas
                    .check_proof(&f.context, std::slice::from_ref(&step), &mut budget)
            } else {
                f.cas.compute_series(
                    &SeriesOperation::Reciprocal {
                        input: input.clone(),
                    },
                    &[],
                    &mut budget,
                )
            };
            let fault = result.expect_err("unrepresentable workspace");
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: crate::error::Exhaustion::Capacity,
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: u64::MAX,
                    needed: u64::MAX
                }
            );
            assert_eq!(fault.location, AT);
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        }
        supplied_workspace_capacity(&mut f, &step, 4_099_276_460_824_344_232, 11)?;
        f.cas.check_proof(
            &f.context,
            std::slice::from_ref(&step),
            &mut default_budget()?,
        )?;
        // Reducing bits by 64 reduces cell by 16 and workspace by 18*16=288.
        // The certificate copies now fit, leaving 47 bytes; the genuinely
        // retained positive judgment makes the adjacent addition overflow.
        supplied_workspace_capacity(&mut f, &step, 4_099_276_460_824_344_168, 299)?;
        let Certificate::Series(certificate) = &mut step.certificate else {
            return Err("certificate".into());
        };
        let C::Reciprocal { inverse, .. } = certificate.as_mut() else {
            return Err("reciprocal".into());
        };
        *inverse = Rational::zero();
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .check_proof(&f.context, &[step], &mut default_budget()?)
                .expect_err("false inverse")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        Ok(())
    }

    /// Exercise the public checker after a representable workspace calculation.
    fn supplied_workspace_capacity(
        f: &mut Fixture,
        step: &crate::proof_model::ApplicationDraft,
        bits: u64,
        remaining: u64,
    ) -> TestResult {
        f.cas.limits.intermediate_bits = bits;
        let limits = Limits {
            intermediate_bits: bits,
            ..Limits::default()
        };
        let mut budget = Budget::new(limits.clone())?;
        let bytes = workspace(1, &budget)?;
        assert_eq!(bytes, u64::MAX - remaining);
        let crate::proof_model::Certificate::Series(certificate) = &step.certificate else {
            return Err("series certificate".into());
        };
        let mut out = crate::encoding::Encoder::new(&mut budget, AT);
        super::super::codec::certificate(certificate, &mut out)?;
        assert_eq!(out.finish().len(), 84);
        if remaining == 11 {
            assert!(bytes.checked_add(3 * 84).is_none());
        } else {
            let with_certificate = bytes.checked_add(3 * 84).ok_or("certificate fits")?;
            assert_eq!(with_certificate, u64::MAX - 47);
            assert!(
                with_certificate
                    .checked_add(f.cas.retained_judgment_bytes())
                    .is_none()
            );
        }
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .cas
            .check_proof(
                &f.context,
                std::slice::from_ref(step),
                &mut Budget::new(limits)?,
            )
            .expect_err("checker reservation capacity");
        assert_eq!(
            fault.kind,
            FaultKind::Exhausted {
                reason: crate::error::Exhaustion::Capacity,
                budget: BudgetKind::StagedPayloadBytes,
                limit: u64::MAX,
                needed: u64::MAX,
            }
        );
        assert_eq!(fault.location, AT);
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        Ok(())
    }

    /// Triangular and portable counts are rejected before attempting a row allocation.
    /// ´test:unit:series-reciprocal-capacity-preflight´
    #[test]
    fn reciprocal_capacity_preflight() -> TestResult {
        assert_eq!(entries(92_681)?, 4_294_930_221);
        for order in [0, 92_682, u32::MAX] {
            assert_eq!(
                entries(order).expect_err("unrepresentable count").kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
        }
        Ok(())
    }

    /// Arithmetic-phase counts exclude admission, encoding and publication reads.
    /// ´test:unit:series-reciprocal-arithmetic-read-counts´
    #[test]
    fn arithmetic_read_counts() -> TestResult {
        for (order, pairs, reads) in [(1, 1, 4), (4, 10, 22), (6, 21, 44)] {
            let mut f = Fixture::new(&["2"], "0", false, false)?;
            let mut p = vec![Rational::zero(); order];
            let mut q = vec![Rational::zero(); order];
            p[0] = Rational::parse("2", &mut default_budget()?)?;
            q[0] = Rational::parse("1/2", &mut default_budget()?)?;
            let input = f.object(&p)?;
            let output = f.object(&q)?;
            let mut rows = Vec::new();
            for degree in 0..order {
                for i in 0..=degree {
                    rows.push(SeriesProductRow {
                        degree: u32::try_from(degree)?,
                        left_index: u32::try_from(i)?,
                        right_index: u32::try_from(degree - i)?,
                        left: p[i].clone(),
                        right: q[degree - i].clone(),
                        product: if degree == 0 {
                            Rational::one()
                        } else {
                            Rational::zero()
                        },
                        sum: if degree == 0 {
                            Rational::one()
                        } else {
                            Rational::zero()
                        },
                    });
                }
            }
            let certificate = C::Reciprocal {
                form: 0,
                constant: p[0].clone(),
                inverse: q[0].clone(),
                rows,
            };
            VISITS.with(|cell| cell.set([0; 3]));
            let mut budget = default_budget()?;
            check(
                &f.cas,
                &S::Reciprocal { input, output },
                &certificate,
                &[],
                &mut budget,
            )?;
            assert_eq!(VISITS.with(std::cell::Cell::get), [pairs, 2, 2 * pairs]);
            assert_eq!(budget.usage().jet_entries, reads);
        }
        Ok(())
    }

    /// Actual unit/product reads are visible only after the prepaid access succeeds.
    /// ´test:unit:series-reciprocal-ordered-coefficient-visits´
    #[test]
    fn ordered_coefficient_visits() -> TestResult {
        let mut f = Fixture::new(&["2"], "0", false, false)?;
        let input = f.object(&[Rational::parse("2", &mut default_budget()?)?])?;
        let object = f.cas.series_object(&input)?;
        for phase in [1, 2] {
            for work in [0, 1] {
                VISITS.with(|cell| cell.set([0; 3]));
                let mut budget = Budget::new(Limits {
                    work,
                    ..Limits::default()
                })?;
                let result = read(&f.cas, object, 0, phase, &mut budget);
                if work == 0 {
                    assert_eq!(
                        result.expect_err("prepaid read").kind,
                        FaultKind::Exhausted {
                            reason: crate::error::Exhaustion::Limit,
                            budget: BudgetKind::Work,
                            limit: 0,
                            needed: 1
                        }
                    );
                    assert_eq!(VISITS.with(std::cell::Cell::get), [0; 3]);
                } else {
                    result?;
                    let mut expected = [0; 3];
                    expected[phase] = 1;
                    assert_eq!(VISITS.with(std::cell::Cell::get), expected);
                }
            }
        }
        Ok(())
    }

    /// A malformed last row still requires the whole metadata scan to be prepaid.
    /// ´test:unit:series-reciprocal-ordered-shape-visits´
    #[test]
    fn ordered_shape_visits() -> TestResult {
        let mut rows = Vec::new();
        for degree in 0..4 {
            for i in 0..=degree {
                rows.push(SeriesProductRow {
                    degree,
                    left_index: i,
                    right_index: degree - i,
                    left: Rational::zero(),
                    right: Rational::zero(),
                    product: Rational::zero(),
                    sum: Rational::zero(),
                });
            }
        }
        rows.last_mut().ok_or("last row")?.degree = 9;
        for work in [9, 10] {
            VISITS.with(|cell| cell.set([0; 3]));
            let mut budget = Budget::new(Limits {
                work,
                ..Limits::default()
            })?;
            let error = shape(&rows, 4, &mut budget).expect_err("malformed final row");
            assert_eq!(
                error.kind,
                if work == 9 {
                    FaultKind::Exhausted {
                        reason: crate::error::Exhaustion::Limit,
                        budget: BudgetKind::Work,
                        limit: 9,
                        needed: 10,
                    }
                } else {
                    FaultKind::Invalid(InvalidCode::Rule)
                }
            );
            assert_eq!(
                VISITS.with(std::cell::Cell::get),
                [if work == 9 { 0 } else { 10 }, 0, 0]
            );
        }
        Ok(())
    }
}
