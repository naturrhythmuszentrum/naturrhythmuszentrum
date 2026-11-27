// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine timing admission from the retained original relation.

use super::{AT, Bounds, LcRelationError, LcSampleTimes, admit};
use crate::{Budget, LcArithmeticMode, LcFace, LcQuantity, LcStratum, contains_binary64};
use nrz_cas::Cas;
use nrz_cas::enclosure::lc_affine_fibre::{
    PiAffine, Receipt, TimingObservation, TimingRequest, TimingSourceKind, TimingSourceSlot,
    TimingWitness,
};
use nrz_cas::pi_bounds::PiBoundCertificate;
use std::cmp::Ordering;

/// Intersect one original rational support with the current effective support.
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

impl LcSampleTimes {
    /// Check exact live topology, reusing retained originals when unchanged.
    /// Then check supplied binary64 predicate enclosures.
    /// Equality predicates retain exact CAS evidence and require zero native endpoints.
    /// # Errors
    /// Refuses stale originals, incomplete predicate inventory, invalid native faces or resources.
    pub fn affine_timing_native64(
        &self,
        cas: &Cas,
        receipt: &Receipt,
        current: &TimingRequest,
        endpoints: &[[f64; 2]],
        mode: LcArithmeticMode,
        b: &mut Budget,
    ) -> Result<nrz_cas::enclosure::lc_affine_fibre::Topology, LcRelationError> {
        let lifts: Vec<_> = current.observations.iter().map(|item| item.lift).collect();
        let domains: Vec<_> = current
            .observations
            .iter()
            .map(|item| item.lift_domain)
            .collect();
        let expected = self.affine_timing_request(
            &lifts,
            &domains,
            &current.source_versions,
            current.pi.clone(),
            b,
        )?;
        if !expected.same_charged(current, b)? {
            return Err(LcRelationError::Projection);
        }
        let (topology, signs) = receipt.live_sign_predicates(cas, current, b)?;
        if signs.len() != endpoints.len() {
            return Err(LcRelationError::Observation);
        }
        for (predicate, endpoint) in signs.iter().zip(endpoints) {
            let zero = nrz_cas::Rational::zero();
            if predicate.required == Ordering::Equal {
                if predicate.image != [zero.clone(), zero] || *endpoint != [0.0, 0.0] {
                    return Err(LcRelationError::Containment);
                }
                continue;
            }
            let half = nrz_cas::Rational::from_ratio(1, 2, b)?;
            let stratum = if predicate.required == Ordering::Greater {
                LcStratum {
                    lower: Some(LcFace {
                        exact: half,
                        inward: 0.5,
                        open: true,
                    }),
                    upper: None,
                }
            } else {
                LcStratum {
                    lower: None,
                    upper: Some(LcFace {
                        exact: half.negated(b)?,
                        inward: -0.5,
                        open: true,
                    }),
                }
            };
            if !contains_binary64(
                *endpoint,
                [&predicate.image[0], &predicate.image[1]],
                &stratum,
                mode,
                b,
            )? {
                return Err(LcRelationError::Containment);
            }
        }
        Ok(match topology {
            std::borrow::Cow::Borrowed(retained) => retained.copy_charged(b)?,
            std::borrow::Cow::Owned(current) => current,
        })
    }

    /// Reconstruct the complete supported original timing inventory in source order.
    /// Unsupported physical inputs refuse this isolated timing theorem.
    /// # Errors
    /// Refuses missing supports, incomplete source versions, invalid lift domains and resources.
    pub fn affine_timing_request(
        &self,
        lifts: &[i64],
        lift_domains: &[[i64; 2]],
        source_versions: &[u64],
        pi: PiBoundCertificate,
        b: &mut Budget,
    ) -> Result<TimingRequest, LcRelationError> {
        let n = self.time_boxes().len();
        if lifts.len() != n
            || lift_domains.len() != n
            || source_versions.len() != self.relation().inputs().len()
        {
            return Err(LcRelationError::Observation);
        }
        let mut rate = None;
        let mut initial = None;
        let mut rate_declarations = Vec::new();
        let mut initial_declarations = Vec::new();
        let mut time_declarations = vec![Vec::new(); n];
        let mut phase_declarations = vec![Vec::new(); n];
        let mut phases = vec![None; n];
        let mut source_order = Vec::new();
        source_order
            .try_reserve_exact(source_versions.len())
            .map_err(|_| LcRelationError::Capacity)?;
        for input in self.relation().inputs() {
            b.work(1, AT)?;
            let bounds = admit::bounds(input.interval(), b)?;
            let slot = match input.quantity() {
                LcQuantity::Rate | LcQuantity::AngularFrequency => {
                    let slot = TimingSourceSlot {
                        kind: TimingSourceKind::Rate,
                        declaration: rate_declarations.len(),
                    };
                    intersect(&mut rate, &bounds, b)?;
                    rate_declarations.push(bounds);
                    slot
                }
                LcQuantity::InitialPhase => {
                    let slot = TimingSourceSlot {
                        kind: TimingSourceKind::Initial,
                        declaration: initial_declarations.len(),
                    };
                    intersect(&mut initial, &bounds, b)?;
                    initial_declarations.push(bounds);
                    slot
                }
                LcQuantity::Time(index) if index < n => {
                    let slot = TimingSourceSlot {
                        kind: TimingSourceKind::Time(index),
                        declaration: time_declarations[index].len(),
                    };
                    time_declarations[index].push(bounds);
                    slot
                }
                LcQuantity::Phase(index) if index < n => {
                    let slot = TimingSourceSlot {
                        kind: TimingSourceKind::Phase(index),
                        declaration: phase_declarations[index].len(),
                    };
                    intersect(&mut phases[index], &bounds, b)?;
                    phase_declarations[index].push(bounds);
                    slot
                }
                _ => return Err(LcRelationError::Projection),
            };
            source_order.push(slot);
        }
        let rate = rate.ok_or(LcRelationError::Projection)?;
        let initial = initial.ok_or(LcRelationError::Projection)?;
        if rate != *self.rate_support() {
            return Err(LcRelationError::Projection);
        }
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(n)
            .map_err(|_| LcRelationError::Capacity)?;
        for (index, phase) in phases.into_iter().enumerate() {
            let phase = phase.ok_or(LcRelationError::Projection)?;
            if time_declarations[index].is_empty()
                || lift_domains[index][0] > lifts[index]
                || lift_domains[index][1] < lifts[index]
            {
                return Err(LcRelationError::CycleCover);
            }
            observations.push(TimingObservation {
                time: self.time_boxes()[index].clone(),
                phase,
                time_declarations: std::mem::take(&mut time_declarations[index]),
                phase_declarations: std::mem::take(&mut phase_declarations[index]),
                lift: lifts[index],
                lift_domain: lift_domains[index],
            });
        }
        Ok(TimingRequest {
            rate,
            initial,
            rate_declarations,
            initial_declarations,
            observations,
            epoch: self.relation().epoch().to_owned(),
            nominal: self.nominal_times().to_vec(),
            source_order,
            source_versions: source_versions.to_vec(),
            pi,
            revision: 1,
        })
    }

    /// Restore every timestamp from a checked current original timing inventory.
    /// # Errors
    /// Refuses stale relation data, changed incidence or an outside projected point.
    pub fn affine_timing_witness(
        &self,
        cas: &Cas,
        receipt: &Receipt,
        current: &TimingRequest,
        point: &[PiAffine; 2],
        b: &mut Budget,
    ) -> Result<TimingWitness, LcRelationError> {
        let lifts: Vec<_> = current.observations.iter().map(|item| item.lift).collect();
        let domains: Vec<_> = current
            .observations
            .iter()
            .map(|item| item.lift_domain)
            .collect();
        let expected = self.affine_timing_request(
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
    use crate::LcRelation;
    use nrz_cas::Rational;
    use nrz_cas::limits::Limits;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn q(value: i64, b: &mut Budget) -> Result<Rational, nrz_cas::Fault> {
        Rational::from_ratio(value, 1, b)
    }

    fn pi(b: &mut Budget) -> Result<PiBoundCertificate, nrz_cas::Fault> {
        Ok(PiBoundCertificate {
            version: 1,
            terms: [2, 1],
            sums: [
                Rational::from_ratio(74, 375, b)?,
                Rational::from_ratio(1, 239, b)?,
            ],
            lower: Rational::from_ratio(128, 41, b)?,
            upper: Rational::from_ratio(640, 203, b)?,
        })
    }

    fn exact_integer_images() -> Vec<i64> {
        // Independent integer row oracle for these ten declared strips.
        // Each determinant is 0 or ±1, so every solved coordinate and residual is integral.
        let rows: [(i64, i64, i64); 10] = [
            (-1, 0, -1),
            (1, 0, 2),
            (0, -1, 0),
            (0, 1, 0),
            (-1, -1, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 1, 0),
        ];
        let mut expected_values = vec![1, 1, 0, 1, 0, 0, 0, 0];
        for (i, &(a, phase, bound)) in rows.iter().enumerate() {
            for &(other_a, other_phase, other_bound) in &rows[i + 1..] {
                let det = a * other_phase - other_a * phase;
                expected_values.push(det);
                if det != 0 {
                    let rate = (bound * other_phase - other_bound * phase) / det;
                    let initial = (other_bound * a - bound * other_a) / det;
                    for &(row_a, row_phase, row_bound) in &rows {
                        expected_values.push(row_a * rate + row_phase * initial - row_bound);
                    }
                }
            }
        }
        expected_values
    }

    /// Build the exact two-observation original shared by fibre and ray controls.
    fn timing_fixture() -> Result<LcSampleTimes, Box<dyn std::error::Error>> {
        let mut b = Budget::new(Limits::default())?;
        let first = 0;
        let second = 1;
        let declarations = [
            (LcQuantity::InitialPhase, [0, 0]),
            (LcQuantity::Rate, [1, 2]),
            (LcQuantity::Time(first), [0, 1]),
            (LcQuantity::Phase(first), [0, 0]),
            (LcQuantity::Time(first), [0, 0]),
            (LcQuantity::Time(second), [0, 0]),
            (LcQuantity::Phase(second), [0, 0]),
        ];
        let inputs = declarations
            .into_iter()
            .map(|(kind, [lo, hi])| admit::input(kind, [q(lo, &mut b)?, q(hi, &mut b)?], &mut b))
            .collect::<Result<Vec<_>, _>>()?;
        let relation = LcRelation::new("epoch".into(), 2, inputs, &mut b)?;
        Ok(LcSampleTimes::new(
            relation,
            vec![Rational::zero(); 2],
            &mut b,
        )?)
    }

    /// Bind independent timing originals to the complete actual-time future ray.
    #[test]
    fn lc_future_ray_timing_binding() -> TestResult {
        let timing = timing_fixture()?;
        let mut cas = Cas::new(Limits::default())?;
        let mut ray_budget = Budget::new(Limits::blind_cubic())?;
        let ray_pi = pi(&mut ray_budget)?;
        let ray = timing.affine_future_ray(
            &mut cas,
            &[vec![0, 0]],
            &[[0, 0]; 2],
            &[0],
            &[1; 7],
            &ray_pi,
            q(1, &mut ray_budget)?,
            PiAffine::rational(q(1, &mut ray_budget)?),
            false,
            &mut ray_budget,
        )?;
        assert!(ray.original().shares_allocation(timing.relation()));
        assert_eq!(ray.proposal(&cas, &mut ray_budget)?.vertices.len(), 2);
        let _horizon = ray.horizon(&cas, &mut ray_budget)?;
        Ok(())
    }

    #[test]
    fn lc_affine_timing_binding_originals() -> TestResult {
        let timing = timing_fixture()?;
        let first = 0;
        let mut b = Budget::new(Limits::blind_cubic())?;
        let request =
            timing.affine_timing_request(&[0, 0], &[[0, 0]; 2], &[1; 7], pi(&mut b)?, &mut b)?;
        assert_eq!(request.source_order[4].kind, TimingSourceKind::Time(first));
        assert_eq!(request.source_order[4].declaration, 1);
        let cas = Cas::new(Limits::default())?;
        let mut b = Budget::new(Limits::blind_cubic())?;
        let receipt = cas.construct_lc_affine_timing(&request, &mut b)?;
        let mut b = Budget::new(Limits::blind_cubic())?;
        let vertex = receipt.topology(&cas, &mut b)?.vertices[0].point.clone();
        let witness = timing.affine_timing_witness(&cas, &receipt, &request, &vertex, &mut b)?;
        assert_eq!(witness.timestamps.len(), 2);
        let expected_values = exact_integer_images();
        let (_, predicates) = receipt.live_sign_predicates(&cas, &request, &mut b)?;
        assert_eq!(predicates.len(), expected_values.len());
        let mut endpoints = Vec::new();
        for (predicate, expected) in predicates.iter().zip(expected_values) {
            let exact = q(expected, &mut b)?;
            assert_eq!(predicate.image, [exact.clone(), exact]);
            let value = f64::from(i32::try_from(expected)?);
            endpoints.push([value, value]);
        }
        assert_eq!(
            timing
                .affine_timing_native64(
                    &cas,
                    &receipt,
                    &request,
                    &endpoints,
                    LcArithmeticMode::REQUIRED,
                    &mut b
                )?
                .dimension,
            nrz_cas::enclosure::lc_affine_fibre::Dimension::Segment
        );
        let mut bad_endpoints = endpoints.clone();
        bad_endpoints[0] = [0.0, 2.0];
        assert!(
            timing
                .affine_timing_native64(
                    &cas,
                    &receipt,
                    &request,
                    &bad_endpoints,
                    LcArithmeticMode::REQUIRED,
                    &mut b
                )
                .is_err()
        );
        let mut missing = request.clone();
        missing.source_order.pop();
        assert!(
            timing
                .affine_timing_witness(
                    &cas,
                    &receipt,
                    &missing,
                    &[
                        PiAffine::rational(Rational::one()),
                        PiAffine::rational(Rational::zero())
                    ],
                    &mut b
                )
                .is_err()
        );
        Ok(())
    }
}
