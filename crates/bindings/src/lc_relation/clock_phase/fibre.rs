// SPDX-License-Identifier: AGPL-3.0-only
//! Complete common-clock and common-phase original inventory binding.

use super::{AT, Bounds, LcRelationError, LcSharedClockPhase, admit};
use crate::{Budget, LcQuantity};
use nrz_cas::Cas;
use nrz_cas::enclosure::lc_affine_fibre::{
    ClockPhaseObservation, ClockPhaseReceipt, ClockPhaseRequest, ClockPhaseSourceKind,
    ClockPhaseSourceSlot, ClockPhaseWitness, PiAffine,
};
use nrz_cas::pi_bounds::PiBoundCertificate;
use std::cmp::Ordering;

/// Intersect one support with an existing effective support.
fn intersect(
    slot: &mut Option<Bounds>,
    next: &Bounds,
    b: &mut Budget,
) -> Result<(), LcRelationError> {
    let mut value = next.clone();
    if let Some(previous) = slot {
        if previous[0].compare(&value[0], b)? == Ordering::Greater {
            value[0].clone_from(&previous[0]);
        }
        if previous[1].compare(&value[1], b)? == Ordering::Less {
            value[1].clone_from(&previous[1]);
        }
    }
    if value[0].compare(&value[1], b)? == Ordering::Greater {
        return Err(LcRelationError::Containment);
    }
    *slot = Some(value);
    Ok(())
}

impl LcSharedClockPhase {
    /// Reconstruct every original declaration and both phase layers in source order.
    /// Versions cover original relation inputs, the clock and phase offsets, then samples.
    /// # Errors
    /// Refuses unsupported originals, incomplete versions/lifts and exhausted resources.
    #[expect(
        clippy::too_many_lines,
        reason = "one ordered original-inventory traversal"
    )]
    pub fn affine_clock_phase_request(
        &self,
        lifts: &[i64],
        domains: &[[i64; 2]],
        versions: &[u64],
        pi: PiBoundCertificate,
        b: &mut Budget,
    ) -> Result<ClockPhaseRequest, LcRelationError> {
        let n = self.clock().nominal_times().len();
        if lifts.len() != n
            || domains.len() != n
            || self.samples().len() != n
            || versions.len() != self.relation().inputs().len() + 2 + n
        {
            return Err(LcRelationError::Observation);
        }
        let mut rate = None;
        let mut initial = None;
        let mut rate_declarations = Vec::new();
        let mut initial_declarations = Vec::new();
        let mut clock_declarations = Vec::new();
        let mut offset_declarations = Vec::new();
        let mut time_declarations = vec![Vec::new(); n];
        let mut measured_declarations = vec![Vec::new(); n];
        let mut physical_declarations = vec![Vec::new(); n];
        let mut source_order = Vec::new();
        source_order
            .try_reserve_exact(versions.len())
            .map_err(|_| LcRelationError::Capacity)?;
        for input in self.relation().inputs() {
            b.work(1, AT)?;
            let bounds = admit::bounds(input.interval(), b)?;
            let slot = match input.quantity() {
                LcQuantity::Rate | LcQuantity::AngularFrequency => {
                    let slot = ClockPhaseSourceSlot {
                        kind: ClockPhaseSourceKind::Rate,
                        declaration: rate_declarations.len(),
                    };
                    intersect(&mut rate, &bounds, b)?;
                    rate_declarations.push(bounds);
                    slot
                }
                LcQuantity::InitialPhase => {
                    let slot = ClockPhaseSourceSlot {
                        kind: ClockPhaseSourceKind::Initial,
                        declaration: initial_declarations.len(),
                    };
                    intersect(&mut initial, &bounds, b)?;
                    initial_declarations.push(bounds);
                    slot
                }
                LcQuantity::Time(index) if index < n => {
                    let slot = ClockPhaseSourceSlot {
                        kind: ClockPhaseSourceKind::Time(index),
                        declaration: time_declarations[index].len(),
                    };
                    time_declarations[index].push(bounds);
                    slot
                }
                LcQuantity::Phase(index) if index < n => {
                    let slot = ClockPhaseSourceSlot {
                        kind: ClockPhaseSourceKind::Physical(index),
                        declaration: physical_declarations[index].len(),
                    };
                    physical_declarations[index].push(bounds);
                    slot
                }
                _ => return Err(LcRelationError::Projection),
            };
            source_order.push(slot);
        }
        let clock = self.clock().offset().clone();
        clock_declarations.push(clock.clone());
        source_order.push(ClockPhaseSourceSlot {
            kind: ClockPhaseSourceKind::Clock,
            declaration: 0,
        });
        let offset = self.offset().clone();
        offset_declarations.push(offset.clone());
        source_order.push(ClockPhaseSourceSlot {
            kind: ClockPhaseSourceKind::Offset,
            declaration: 0,
        });
        for (index, sample) in self.samples().iter().enumerate() {
            let bounds = admit::bounds(sample, b)?;
            measured_declarations[index].push(bounds);
            source_order.push(ClockPhaseSourceSlot {
                kind: ClockPhaseSourceKind::Measured(index),
                declaration: 0,
            });
        }
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(n)
            .map_err(|_| LcRelationError::Capacity)?;
        for index in 0..n {
            if physical_declarations[index].is_empty()
                || domains[index][0] > lifts[index]
                || domains[index][1] < lifts[index]
            {
                return Err(LcRelationError::CycleCover);
            }
            observations.push(ClockPhaseObservation {
                reported: self.clock().nominal_times()[index].clone(),
                time_declarations: std::mem::take(&mut time_declarations[index]),
                measured_declarations: std::mem::take(&mut measured_declarations[index]),
                physical_declarations: std::mem::take(&mut physical_declarations[index]),
                lift: lifts[index],
                lift_domain: domains[index],
            });
        }
        let rate = rate.ok_or(LcRelationError::Projection)?;
        let initial = initial.ok_or(LcRelationError::Projection)?;
        Ok(ClockPhaseRequest {
            rate,
            rate_declarations,
            initial,
            initial_declarations,
            clock,
            clock_declarations,
            offset,
            offset_declarations,
            observations,
            epoch: self.relation().epoch().to_owned(),
            source_order,
            source_versions: versions.to_vec(),
            pi,
            revision: 1,
        })
    }

    /// Restore a joint physical tuple from a checked current branch point.
    /// # Errors
    /// Refuses stale originals, changed strata, outside points and exhausted resources.
    pub fn affine_clock_phase_witness(
        &self,
        cas: &Cas,
        receipt: &ClockPhaseReceipt,
        current: &ClockPhaseRequest,
        point: &[PiAffine; 4],
        b: &mut Budget,
    ) -> Result<ClockPhaseWitness, LcRelationError> {
        let lifts: Vec<_> = current.observations.iter().map(|item| item.lift).collect();
        let domains: Vec<_> = current
            .observations
            .iter()
            .map(|item| item.lift_domain)
            .collect();
        let expected = self.affine_clock_phase_request(
            &lifts,
            &domains,
            &current.source_versions,
            current.pi.clone(),
            b,
        )?;
        if !expected.same_charged(current, b)? {
            return Err(LcRelationError::Projection);
        }
        Ok(receipt.restore(cas, current, point, b)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LcInput, LcInterval, LcRelation, LcSharedClock};
    use nrz_cas::{Rational, limits::Limits};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    /// The 18-row bindings inventory needs a caller ledger wider than blind cubic.
    fn limits() -> Limits {
        Limits {
            output_bytes: 16_777_216,
            staged_payload_bytes: 67_108_864,
            work: 10_000_000_000,
            arithmetic_steps: 10_000_000_000,
            ..Limits::blind_cubic()
        }
    }

    /// Convert a small rational source endpoint.
    fn q(n: i64, d: u64, b: &mut Budget) -> Result<Rational, nrz_cas::Fault> {
        Rational::from_ratio(n, d, b)
    }

    /// Build one exact original interval.
    fn interval(lo: (i64, u64), hi: (i64, u64), b: &mut Budget) -> TestResult<LcInterval> {
        Ok(LcInterval::new(
            Some(q(lo.0, lo.1, b)?.into()),
            Some(q(hi.0, hi.1, b)?.into()),
            b,
        )?)
    }

    /// Build the bounded original physical relation and both measured layers.
    fn phase(extra: bool) -> TestResult<LcSharedClockPhase> {
        let b = &mut Budget::new(limits())?;
        let first = 0;
        let second = 1;
        let declarations = [
            (LcQuantity::Rate, (1, 1), (2, 1)),
            (LcQuantity::InitialPhase, (0, 1), (1, 1)),
            (LcQuantity::Time(first), (5, 4), (3, 2)),
            (LcQuantity::Phase(first), (1, 1), (4, 1)),
            (LcQuantity::Phase(second), (2, 1), (6, 1)),
        ];
        let mut inputs = Vec::new();
        for (kind, low, high) in declarations {
            inputs.push(LcInput::new(
                kind,
                &kind.descriptor()?,
                interval(low, high, b)?,
                b,
            )?);
        }
        if extra {
            let kind = LcQuantity::Energy;
            inputs.push(LcInput::new(
                kind,
                &kind.descriptor()?,
                interval((1, 1), (1, 1), b)?,
                b,
            )?);
        }
        let relation = LcRelation::new("joint epoch".into(), 2, inputs, b)?;
        let clock = LcSharedClock::new(
            relation,
            vec![q(1, 1, b)?, q(2, 1, b)?],
            &interval((0, 1), (1, 1), b)?,
            b,
        )?;
        Ok(LcSharedClockPhase::new(
            clock,
            vec![interval((0, 1), (4, 1), b)?, interval((0, 1), (6, 1), b)?],
            &interval((0, 1), (1, 1), b)?,
            b,
        )?)
    }

    /// Bind the complete original request, restore a nonzero offset, and refuse energy.
    #[test]
    fn lc_affine_clock_phase_binding_entry_points() -> TestResult {
        let question = phase(false)?;
        let b = &mut Budget::new(limits())?;
        let pi = PiBoundCertificate {
            version: 1,
            terms: [2, 1],
            sums: [q(74, 375, b)?, q(1, 239, b)?],
            lower: q(128, 41, b)?,
            upper: q(640, 203, b)?,
        };
        let request =
            question.affine_clock_phase_request(&[0, 0], &[[0, 0]; 2], &[1; 9], pi.clone(), b)?;
        assert_eq!(request.source_order.len(), 9);
        let cas = Cas::new(limits())?;
        let receipt = cas.construct_lc_affine_clock_phase(&request, b)?;
        let point = [2, 1, 1, 1].map(|v| PiAffine::rational(q(v, 1, b).expect("integer")));
        let witness = question.affine_clock_phase_witness(&cas, &receipt, &request, &point, b)?;
        assert_eq!(witness.phase_offset, PiAffine::rational(q(1, 1, b)?));
        assert_eq!(witness.physical[0], PiAffine::rational(q(4, 1, b)?));
        let unsupported = phase(true)?;
        assert!(matches!(
            unsupported.affine_clock_phase_request(&[0, 0], &[[0, 0]; 2], &[1; 10], pi, b),
            Err(LcRelationError::Projection)
        ));
        Ok(())
    }
}
