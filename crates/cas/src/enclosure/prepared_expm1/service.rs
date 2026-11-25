// SPDX-License-Identifier: AGPL-3.0-only
//! Original-claim admission and graph-owned preparation/replay on one shared ledger.

use super::{AT, Counts, Outcome, Policy, Prepared, Proof, Request, count, invalid, math};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::model::Operator;
use crate::{Cas, Fault, InvalidCode, Rational};
use std::borrow::Cow;
use std::cmp::Ordering;

/// Checked original scope, including the derived exact-zero shortcut premise.
pub(super) struct Claim {
    /// Portable original claim, domain, target and arithmetic model.
    pub(super) scope: Vec<u8>,
    /// Whether the entire original argument box is exactly zero.
    pub(super) zero: bool,
}

impl Cas {
    /// Prepare and independently check a finite expm1 program, reusing authenticated graph authority.
    /// # Errors
    /// Refuses malformed original claims or environments and preserves resource faults and spent Work.
    pub fn prepare_expm1(
        &mut self,
        request: &Request,
        policy: Policy,
        budget: &mut Budget,
        counts: &mut Counts,
    ) -> Result<Outcome, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = budget.with_work_phase("prepared-expm1", "nrz.prepare", |budget| {
            count(&mut counts.preparations, "preparation")?;
            self.transaction(budget, |cas, budget| {
                crate::decision_kernel::cache::retained(cas, budget, |_| Ok(()))?;
                let bytes = request_inventory(request, budget, counts)?;
                budget.with_retained_payload(bytes, AT, |budget| {
                    prepare(cas, request, policy, budget, counts)
                })
            })
        });
        budget.restore_limits(previous);
        result
    }

    /// Produce one explicitly untrusted candidate for inspection or independent replay.
    /// No graph authority or numerical value is returned by this proposal route.
    /// # Errors
    /// Refuses invalid claims, schedule parameters or exhausted resources.
    pub fn propose_expm1(
        &mut self,
        request: &Request,
        policy: Policy,
        degree: u32,
        budget: &mut Budget,
        counts: &mut Counts,
    ) -> Result<Proof, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = budget.with_work_phase("prepared-expm1", "nrz.proposal", |budget| {
            self.transaction(budget, |cas, budget| {
                crate::decision_kernel::cache::retained(cas, budget, |_| Ok(()))?;
                let bytes = request_inventory(request, budget, counts)?;
                budget.with_retained_payload(bytes, AT, |budget| {
                    let claim = claim(cas, request, policy, budget)?;
                    parameters(degree, policy.coefficient_bits, policy, budget)?;
                    super::produce::candidate(
                        claim.scope,
                        degree,
                        policy.coefficient_bits,
                        budget,
                        counts,
                    )
                })
            })
        });
        budget.restore_limits(previous);
        result
    }

    /// Independently replay a portable schedule against this store's expected original claim.
    /// # Errors
    /// Refuses any invalid proof, claim mismatch or exhausted resource without partial publication.
    pub fn check_expm1(
        &mut self,
        request: &Request,
        policy: Policy,
        proof: &Proof,
        budget: &mut Budget,
        counts: &mut Counts,
    ) -> Result<Prepared, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = budget.with_work_phase("prepared-expm1", "nrz.replay", |budget| {
            self.transaction(budget, |cas, budget| {
                crate::decision_kernel::cache::retained(cas, budget, |_| Ok(()))?;
                let bytes = request_inventory(request, budget, counts)?
                    .saturating_add(proof_inventory(proof, policy, budget, counts)?);
                budget.with_retained_payload(bytes, AT, |budget| {
                    let claim = claim(cas, request, policy, budget)?;
                    budget.with_retained_payload(claim.scope.len() as u64, AT, |budget| {
                        let checked =
                            budget.with_work_phase("prepared-expm1", "nrz.proof", |budget| {
                                super::check::verify(request, &claim, policy, proof, budget, counts)
                            })?;
                        publish(cas, checked, budget, counts)
                    })
                })
            })
        });
        budget.restore_limits(previous);
        result
    }
}

/// Lookup precedes candidate construction and independent checking.
fn prepare(
    cas: &mut Cas,
    request: &Request,
    policy: Policy,
    budget: &mut Budget,
    counts: &mut Counts,
) -> Result<Outcome, Fault> {
    let claim = claim(cas, request, policy, budget)?;
    budget.with_retained_payload(claim.scope.len() as u64, AT, |budget| {
        let prefix = super::wire::prefix(&claim.scope, budget)?;
        budget.with_retained_payload(prefix.len() as u64, AT, |budget| {
            count(&mut counts.lookups, "retained lookup")?;
            let retained = budget.with_work_phase("prepared-expm1", "nrz.lookup", |budget| {
                if let Some(value) = cas.retained_expm1(&prefix, budget)? {
                    parameters(
                        value.proof.degree,
                        value.proof.coefficient_bits,
                        policy,
                        budget,
                    )?;
                    super::check_components(&value.proof, budget)?;
                    return budget.with_retained_payload(super::footprint(value), AT, |budget| {
                        budget.work(super::footprint(value), AT)?;
                        Ok(Some(value.clone()))
                    });
                }
                Ok(None)
            })?;
            if let Some(value) = retained {
                return Ok(Outcome::Ready(Box::new(value)));
            }
            let epsilon = math::two_power(-i64::from(request.bits), budget)?;
            // This lower bound belongs to this conservative checker, not to binary64 impossibility in general.
            let floor =
                math::two_power(-53, budget)?.mul(&Rational::from_ratio(18, 1, budget)?, budget)?;
            if !claim.zero && epsilon.compare(&floor, budget)? == Ordering::Less {
                return Ok(Outcome::TargetNotCertifiable);
            }
            for degree in 1..=policy.degree.min(policy.attempts) {
                if policy.checking_work > budget.limits().work.saturating_sub(budget.usage().work) {
                    return Err(Fault::exhausted(
                        BudgetKind::Work,
                        budget.limits().work,
                        budget.usage().work.saturating_add(policy.checking_work),
                        AT,
                    ));
                }
                let previous = budget.reserve_enclosure_checking(policy.checking_work);
                let candidate =
                    budget.with_work_phase("prepared-expm1", "nrz.proposal", |budget| {
                        clone_candidate(
                            &claim.scope,
                            degree,
                            policy.coefficient_bits,
                            budget,
                            counts,
                        )
                    });
                budget.restore_limits(previous);
                let proof = candidate?;
                let ready =
                    budget.with_retained_payload(super::proof_bytes(&proof), AT, |budget| {
                        if claim.zero
                            || proof.endpoint_ratio.compare(&epsilon, budget)? != Ordering::Greater
                        {
                            let checked = budget.with_work_phase(
                                "prepared-expm1",
                                "nrz.proof",
                                |budget| {
                                    super::check::verify(
                                        request, &claim, policy, &proof, budget, counts,
                                    )
                                },
                            )?;
                            return publish(cas, checked, budget, counts)
                                .map(|value| Some(Outcome::Ready(Box::new(value))));
                        }
                        Ok(None)
                    })?;
                if let Some(value) = ready {
                    return Ok(value);
                }
            }
            Ok(Outcome::TargetNotCertifiable)
        })
    })
}

/// Every fallible key and copy precedes the graph's atomic publication boundary.
fn publish(
    cas: &mut Cas,
    checked: Prepared,
    budget: &mut Budget,
    counts: &mut Counts,
) -> Result<Prepared, Fault> {
    budget.with_work_phase("prepared-expm1", "nrz.publish", |budget| {
        budget.with_retained_payload(super::footprint(&checked), AT, |budget| {
            let key = super::wire::retention(&checked.proof, budget)?;
            let retained = budget.with_retained_payload(
                super::footprint(&checked).saturating_add(key.len() as u64),
                AT,
                |budget| {
                    budget.work(super::footprint(&checked), AT)?;
                    count(&mut counts.publications, "graph publication")?;
                    Ok::<_, Fault>(checked.clone())
                },
            )?;
            // Move the prepaid buffers into the graph's reservation, never both at once.
            cas.retain_expm1(key, retained, budget)?;
            Ok(checked)
        })
    })
}

/// Concrete finite capacities are resource constraints, not new mathematical domains.
pub(super) fn parameters(
    degree: u32,
    bits: u32,
    policy: Policy,
    budget: &Budget,
) -> Result<(), Fault> {
    if degree == 0 || bits < 2 || policy.attempts == 0 {
        return Err(invalid(InvalidCode::Policy));
    }
    for (kind, actual, limit) in [
        (BudgetKind::PolynomialDegree, u64::from(policy.degree), 128),
        (
            BudgetKind::PolynomialDegree,
            u64::from(degree),
            u64::from(policy.degree),
        ),
        (
            BudgetKind::ComponentBits,
            u64::from(policy.coefficient_bits),
            53,
        ),
        (
            BudgetKind::ComponentBits,
            u64::from(bits),
            u64::from(policy.coefficient_bits),
        ),
    ] {
        if actual > limit {
            return Err(Fault::exhausted(kind, limit, actual, AT));
        }
    }
    math::capacity(degree, budget)
}

/// Validate the original unsimplified exp(argument)-1, context and complete ordered box.
fn claim(
    cas: &Cas,
    request: &Request,
    policy: Policy,
    budget: &mut Budget,
) -> Result<Claim, Fault> {
    parameters(1, policy.coefficient_bits, policy, budget)?;
    if request.bits > policy.target_bits {
        return Err(Fault::exhausted(
            BudgetKind::IntegerExponent,
            u64::from(policy.target_bits),
            u64::from(request.bits),
            AT,
        ));
    }
    if request.environment != 1 {
        return Err(invalid(InvalidCode::Expm1Environment));
    }
    let original = &request.original;
    let root = cas
        .state
        .expression(cas.checked(&original.target, budget)?, budget)?;
    let context = cas
        .state
        .context(cas.checked(&original.context, budget)?, budget)?;
    let shape = root.operator == Operator::Sub
        && root.operands.len() == 2
        && root.scope.is_empty()
        && root.header == context.header_id
        && original.goal == super::super::EnclosureGoal::Width;
    if !shape {
        return Err(invalid(InvalidCode::Expm1Claim));
    }
    let exponential = cas.state.expression(root.operands[0], budget)?;
    let one = cas.state.expression(root.operands[1], budget)?;
    if exponential.operator != Operator::Exponential
        || exponential.operands.len() != 1
        || !matches!(&one.operator,Operator::Rational(value) if value == &Rational::one())
    {
        return Err(invalid(InvalidCode::Expm1Claim));
    }
    let argument_id = exponential.operands[0];
    let argument = cas.state.expression(argument_id, budget)?;
    let range = match &argument.operator {
        Operator::Rational(value) if original.coordinates.is_empty() => {
            Cow::Owned(super::super::Interval::singleton(value, budget)?)
        }
        Operator::Free(_)
            if original.coordinates.len() == 1
                && cas.checked(&original.coordinates[0].coordinate, budget)? == argument_id =>
        {
            Cow::Borrowed(&original.coordinates[0].interval)
        }
        _ => return Err(invalid(InvalidCode::Expm1Claim)),
    };
    let minimum = math::two_power(-1022, budget)?;
    let negative_two = Rational::from_ratio(-2, 1, budget)?;
    for endpoint in [range.lower(), range.upper()] {
        if endpoint.compare(&negative_two, budget)? == Ordering::Less
            || endpoint.compare(&Rational::zero(), budget)? == Ordering::Greater
        {
            return Err(invalid(InvalidCode::Expm1OutsideDomain));
        }
        if !endpoint.is_zero()
            && math::abs(endpoint, budget)?.compare(&minimum, budget)? == Ordering::Less
        {
            return Err(invalid(InvalidCode::Expm1BelowNormal));
        }
    }
    let original_key = super::super::scope::key(cas, original, budget)?;
    budget.with_retained_payload(original_key.len() as u64, AT, |budget| {
        let mut out = Encoder::witness(budget, AT);
        out.name(super::SCHEMA.name)?;
        out.unsigned(1)?;
        out.name("binary64-rn-gradual-separate-lu-identity@1")?;
        out.unsigned(u64::from(request.environment))?;
        out.unsigned(u64::from(request.bits))?;
        out.count(original_key.len())?;
        out.raw(&original_key)?;
        Ok(Claim {
            scope: out.finish(),
            zero: range.lower().is_zero() && range.upper().is_zero(),
        })
    })
}

/// Check fixed-width inventories before touching any caller-supplied coordinate record.
fn request_inventory(
    request: &Request,
    budget: &mut Budget,
    counts: &mut Counts,
) -> Result<u64, Fault> {
    let length = request.original.coordinates.len() as u64;
    budget.check(BudgetKind::MatrixEntries, length, AT)?;
    if length > 1 {
        return Err(invalid(InvalidCode::Expm1Claim));
    }
    budget.work(length, AT)?;
    let start = match &request.original.goal {
        super::super::EnclosureGoal::Width => 256,
        super::super::EnclosureGoal::AtMost(n)
        | super::super::EnclosureGoal::LessThan(n)
        | super::super::EnclosureGoal::Equal(n) => {
            256_u64.saturating_add(super::super::number_bytes(n))
        }
    };
    request
        .original
        .coordinates
        .iter()
        .try_fold(start, |bytes, coordinate| {
            count(&mut counts.coordinates, "coordinate inventory")?;
            Ok(bytes
                .saturating_add(super::super::interval_bytes(&coordinate.interval))
                .saturating_add(128))
        })
}

/// Fixed-width admission and prepaid traversal precede each direct proof size walk.
fn proof_inventory(
    proof: &Proof,
    policy: Policy,
    budget: &mut Budget,
    counts: &mut Counts,
) -> Result<u64, Fault> {
    parameters(proof.degree, proof.coefficient_bits, policy, budget)?;
    if proof.coefficients.len() != proof.degree as usize {
        return Err(invalid(InvalidCode::Expm1Coefficient));
    }
    if proof.stages.len() != (2 * proof.degree - 1) as usize {
        return Err(invalid(InvalidCode::Expm1Rounding));
    }
    budget.check(BudgetKind::ProofNodes, proof.coefficients.len() as u64, AT)?;
    budget.check(BudgetKind::ProofNodes, proof.stages.len() as u64, AT)?;
    budget.check(BudgetKind::WitnessBytes, proof.scope.len() as u64, AT)?;
    budget.work(proof.stages.len() as u64 + 6, AT)?;
    let mut bytes = (proof.scope.len() as u64)
        .saturating_add((proof.coefficients.len() as u64).saturating_mul(32))
        .saturating_add(512);
    for bound in [
        &proof.absolute_tail,
        &proof.relative_tail,
        &proof.import_ratio,
        &proof.central_ratio,
        &proof.endpoint_ratio,
        &proof.restoration_error,
    ] {
        count(&mut counts.proof_records, "proof inventory")?;
        bytes = bytes.saturating_add(super::super::number_bytes(bound));
    }
    for stage in &proof.stages {
        count(&mut counts.proof_records, "proof inventory")?;
        bytes = bytes
            .saturating_add(super::super::number_bytes(&stage.magnitude))
            .saturating_add(super::super::number_bytes(&stage.error));
    }
    Ok(bytes)
}

/// Prepay the copy, then transfer its reservation to the candidate at the ownership move.
fn clone_candidate(
    scope: &[u8],
    degree: u32,
    bits: u32,
    budget: &mut Budget,
    counts: &mut Counts,
) -> Result<Proof, Fault> {
    let scope = budget.with_retained_payload(scope.len() as u64, AT, |budget| {
        budget.work(scope.len() as u64, AT)?;
        Ok::<_, Fault>(scope.to_vec())
    })?;
    super::produce::candidate(scope, degree, bits, budget, counts)
}

#[cfg(test)]
mod tests {
    use super::{AT, Counts, clone_candidate};
    use crate::budget::{Budget, Limits};

    /// A moved candidate scope consumes one reservation beside the caller's original scope.
    /// Two scopes plus the degree-one workspace and 65,536 scratch bytes admit production;
    /// keeping the copy reservation active would require a third 131,072-byte scope.
    /// # Panics
    /// Panics if the moved scope is double counted before proposal construction.
    /// ´claim:cas:prepared-expm1-candidate-transfer´
    /// ´test:unit:prepared-expm1-candidate-transfer´
    #[test]
    fn prepared_expm1_candidate_transfer() -> Result<(), crate::Fault> {
        let scope = vec![0; 131_072];
        let mut budget = Budget::new(Limits {
            work: 2_000_000_000,
            integer_exponent: 4096,
            arithmetic_steps: 50_000_000,
            ..Limits::default()
        })?;
        let workspace = 38 * (8 * budget.limits().component_bits.div_ceil(8) + 128) + 64 + 1024;
        let mut limits = budget.limits().clone();
        limits.staged_payload_bytes = 2 * scope.len() as u64 + workspace + 65_536;
        budget = Budget::new(limits)?;
        let mut counts = Counts::default();
        let proof = budget.with_retained_payload(scope.len() as u64, AT, |budget| {
            clone_candidate(&scope, 1, 53, budget, &mut counts)
        })?;
        assert_eq!(counts.proposals, 1);
        assert_eq!(proof.scope, scope);
        Ok(())
    }
}
