// SPDX-License-Identifier: AGPL-3.0-only
//! Joint original timing-error and clock-phase inventory binding.

use super::{AT, LcRelationError, LcSharedClockPhase};
use crate::{Budget, LcSharedTiming};
use nrz_cas::Cas;
use nrz_cas::enclosure::lc_affine_fibre::{
    ClockPhaseSourceKind, PiAffine, TimingClockPhaseObservation, TimingClockPhaseReceipt,
    TimingClockPhaseRequest, TimingClockPhaseSourceKind, TimingClockPhaseSourceSlot,
    TimingClockPhaseWitness,
};
use nrz_cas::pi_bounds::PiBoundCertificate;

/// Map one original clock-phase source kind into the joint inventory.
const fn kind(value: ClockPhaseSourceKind) -> TimingClockPhaseSourceKind {
    match value {
        ClockPhaseSourceKind::Rate => TimingClockPhaseSourceKind::Rate,
        ClockPhaseSourceKind::Initial => TimingClockPhaseSourceKind::Initial,
        ClockPhaseSourceKind::Clock => TimingClockPhaseSourceKind::Clock,
        ClockPhaseSourceKind::Offset => TimingClockPhaseSourceKind::Offset,
        ClockPhaseSourceKind::Time(i) => TimingClockPhaseSourceKind::Time(i),
        ClockPhaseSourceKind::Measured(i) => TimingClockPhaseSourceKind::Measured(i),
        ClockPhaseSourceKind::Physical(i) => TimingClockPhaseSourceKind::Physical(i),
    }
}
impl LcSharedClockPhase {
    /// Bind the same physical relation to each original independent error and both phase layers.
    /// Versions cover original relation inputs, both shared offsets, all errors, then samples.
    /// # Errors
    /// Refuses different owners, unsupported declarations, missing versions or resources.
    pub fn affine_timing_clock_phase_request(
        &self,
        timing: &LcSharedTiming,
        lifts: &[i64],
        domains: &[[i64; 2]],
        versions: &[u64],
        pi: PiBoundCertificate,
        b: &mut Budget,
    ) -> Result<TimingClockPhaseRequest, LcRelationError> {
        let n = self.clock().nominal_times().len();
        let base = self.relation().inputs().len() + 2;
        if !self.relation().shares_allocation(timing.relation())
            || self.clock().nominal_times() != timing.nominal_times()
            || self.clock().offset() != timing.offset()
            || timing.errors().len() != n
            || versions.len() != base + 2 * n
        {
            return Err(LcRelationError::Projection);
        }
        let mut reduced = Vec::new();
        reduced
            .try_reserve_exact(base + n)
            .map_err(|_| LcRelationError::Capacity)?;
        reduced.extend_from_slice(&versions[..base]);
        reduced.extend_from_slice(&versions[base + n..]);
        let original = self.affine_clock_phase_request(lifts, domains, &reduced, pi, b)?;
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(n)
            .map_err(|_| LcRelationError::Capacity)?;
        for (item, error) in original.observations.into_iter().zip(timing.errors()) {
            let mut error_declarations = Vec::new();
            error_declarations
                .try_reserve_exact(1)
                .map_err(|_| LcRelationError::Capacity)?;
            error_declarations.push(error.clone());
            observations.push(TimingClockPhaseObservation {
                reported: item.reported,
                error: error.clone(),
                error_declarations,
                time_declarations: item.time_declarations,
                measured_declarations: item.measured_declarations,
                physical_declarations: item.physical_declarations,
                lift: item.lift,
                lift_domain: item.lift_domain,
            });
        }
        if observations
            .iter()
            .any(|item| item.time_declarations.is_empty())
        {
            return Err(LcRelationError::Projection);
        }
        let mut source_order = Vec::new();
        source_order
            .try_reserve_exact(base + 2 * n)
            .map_err(|_| LcRelationError::Capacity)?;
        for slot in original.source_order.iter().take(base) {
            source_order.push(TimingClockPhaseSourceSlot {
                kind: kind(slot.kind),
                declaration: slot.declaration,
            });
        }
        for i in 0..n {
            source_order.push(TimingClockPhaseSourceSlot {
                kind: TimingClockPhaseSourceKind::Error(i),
                declaration: 0,
            });
        }
        for slot in original.source_order.iter().skip(base) {
            source_order.push(TimingClockPhaseSourceSlot {
                kind: kind(slot.kind),
                declaration: slot.declaration,
            });
        }
        b.work(source_order.len() as u64, AT)?;
        let mut source_versions = Vec::new();
        source_versions
            .try_reserve_exact(versions.len())
            .map_err(|_| LcRelationError::Capacity)?;
        source_versions.extend_from_slice(versions);
        Ok(TimingClockPhaseRequest {
            rate: original.rate,
            rate_declarations: original.rate_declarations,
            initial: original.initial,
            initial_declarations: original.initial_declarations,
            clock: original.clock,
            clock_declarations: original.clock_declarations,
            offset: original.offset,
            offset_declarations: original.offset_declarations,
            observations,
            epoch: original.epoch,
            source_order,
            source_versions,
            pi: original.pi,
            revision: original.revision,
        })
    }

    /// Restore all timing errors and both phase layers from one checked current point.
    /// # Errors
    /// Refuses changed originals, stale sources, outside points or resources.
    pub fn affine_timing_clock_phase_witness(
        &self,
        timing: &LcSharedTiming,
        cas: &Cas,
        receipt: &TimingClockPhaseReceipt,
        current: &TimingClockPhaseRequest,
        point: &[PiAffine],
        b: &mut Budget,
    ) -> Result<TimingClockPhaseWitness, LcRelationError> {
        let lifts: Vec<_> = current.observations.iter().map(|item| item.lift).collect();
        let domains: Vec<_> = current
            .observations
            .iter()
            .map(|item| item.lift_domain)
            .collect();
        let expected = self.affine_timing_clock_phase_request(
            timing,
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

impl LcSharedTiming {
    /// Bind this timing owner through the complete clock-phase original relation.
    /// # Errors
    /// Refuses different original owners, missing declarations or resources.
    pub fn affine_timing_clock_phase_request(
        &self,
        phase: &LcSharedClockPhase,
        lifts: &[i64],
        domains: &[[i64; 2]],
        versions: &[u64],
        pi: PiBoundCertificate,
        b: &mut Budget,
    ) -> Result<TimingClockPhaseRequest, LcRelationError> {
        phase.affine_timing_clock_phase_request(self, lifts, domains, versions, pi, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LcInput, LcInterval, LcQuantity, LcRelation, LcSharedClock};
    use nrz_cas::{Rational, limits::Limits};
    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
    fn limits() -> Limits {
        Limits {
            work: 20_000_000_000,
            replay_work: 20_000_000_000,
            arithmetic_steps: 20_000_000_000,
            witness_bytes: 32_000_000,
            output_bytes: 32_000_000,
            staged_payload_bytes: 64_000_000,
            matrix_entries: 1_000_000,
            ..Limits::blind_cubic()
        }
    }
    fn q(n: i64, d: u64, b: &mut Budget) -> Result<Rational, nrz_cas::Fault> {
        Rational::from_ratio(n, d, b)
    }
    fn interval(low: (i64, u64), high: (i64, u64), b: &mut Budget) -> TestResult<LcInterval> {
        Ok(LcInterval::new(
            Some(q(low.0, low.1, b)?.into()),
            Some(q(high.0, high.1, b)?.into()),
            b,
        )?)
    }
    fn pi(b: &mut Budget) -> Result<PiBoundCertificate, nrz_cas::Fault> {
        Ok(PiBoundCertificate {
            version: 1,
            terms: [2, 1],
            sums: [q(74, 375, b)?, q(1, 239, b)?],
            lower: q(128, 41, b)?,
            upper: q(640, 203, b)?,
        })
    }
    #[test]
    fn lc_affine_timing_clock_phase_binding_originals() -> TestResult {
        let b = &mut Budget::new(limits())?;
        let first_observation = 0;
        let second_observation = 1;
        let declarations = [
            (LcQuantity::Rate, (1, 1), (2, 1)),
            (LcQuantity::InitialPhase, (0, 1), (1, 1)),
            (LcQuantity::Time(0), (5, 4), (7, 4)),
            (LcQuantity::Phase(first_observation), (1, 1), (5, 1)),
            (LcQuantity::Time(1), (9, 4), (11, 4)),
            (LcQuantity::Phase(second_observation), (2, 1), (6, 1)),
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
        let relation = LcRelation::new("joint epoch".into(), 2, inputs, b)?;
        let nominal = vec![q(1, 1, b)?, q(2, 1, b)?];
        let errors = [interval((0, 1), (1, 4), b)?, interval((0, 1), (1, 4), b)?];
        let timing = LcSharedTiming::new(
            relation.clone(),
            nominal.clone(),
            &errors,
            &interval((0, 1), (1, 1), b)?,
            b,
        )?;
        let clock = LcSharedClock::new(relation, nominal, &interval((0, 1), (1, 1), b)?, b)?;
        let phase = LcSharedClockPhase::new(
            clock,
            vec![interval((0, 1), (5, 1), b)?, interval((0, 1), (6, 1), b)?],
            &interval((0, 1), (1, 1), b)?,
            b,
        )?;
        let versions = [1; 12];
        let request = phase.affine_timing_clock_phase_request(
            &timing,
            &[0, 0],
            &[[0, 0]; 2],
            &versions,
            pi(b)?,
            b,
        )?;
        assert_eq!(
            request.source_order[8].kind,
            TimingClockPhaseSourceKind::Error(0)
        );
        assert_eq!(
            request.source_order[9].kind,
            TimingClockPhaseSourceKind::Error(1)
        );
        assert_eq!(request.observations[0].error[1], q(1, 4, b)?);
        assert_eq!(request.observations[1].error[1], q(1, 4, b)?);
        assert_eq!(request.observations[0].time_declarations.len(), 1);
        assert_eq!(request.observations[1].time_declarations.len(), 1);
        let from_timing = timing.affine_timing_clock_phase_request(
            &phase,
            &[0, 0],
            &[[0, 0]; 2],
            &versions,
            pi(b)?,
            b,
        )?;
        assert!(request.same_charged(&from_timing, b)?);
        assert!(matches!(
            phase.affine_timing_clock_phase_request(
                &timing,
                &[0, 0],
                &[[0, 0]; 2],
                &[1; 11],
                pi(b)?,
                b
            ),
            Err(LcRelationError::Projection)
        ));
        Ok(())
    }
}
