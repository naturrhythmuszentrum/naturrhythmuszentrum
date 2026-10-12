// SPDX-License-Identifier: AGPL-3.0-only
//! Normalized toy execution and atomic semantic publication, without certification.

use std::marker::PhantomData;

use crate::{Checked, Companion, Error, Handle, Heap, Maps, Plan, Scalar, ShapeId, Value};

/// Storage frozen for the life of a validated binding.
///
/// A live bound view prevents mutation of the underlying inputs.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::{resistor_fixture, solve};
/// let mut fixture = resistor_fixture::<f64>()?;
/// let plan = fixture.plan.check()?;
/// let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
/// let bound = binding.validate(&mut fixture.heap)?;
/// let view = bound.normalized()?;
/// fixture.heap.replace(fixture.inputs.source, vec![8.0])?;
/// let candidate = solve(&view)?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
///
/// ```
/// use nrz_descriptor_prototype::{resistor_fixture, solve};
/// let mut fixture = resistor_fixture::<f64>()?;
/// let plan = fixture.plan.check()?;
/// {
///     let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
///     let bound = binding.validate(&mut fixture.heap)?;
///     let candidate = solve(&bound.normalized()?)?;
/// }
/// fixture.heap.replace(fixture.inputs.source, vec![8.0])?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
#[derive(Debug)]
pub struct Bound<'h, 'p, P> {
    /// Exclusive borrow freezes all resolved versions and companion relationships.
    heap: &'h mut Heap<P>,
    /// Immutable declaration and original-expression evidence.
    plan: &'p Plan<Checked>,
    /// Validated coordinate maps, independent of logical shape.
    maps: Maps,
    /// Physical source, resistance and source variance copied under the borrow.
    coefficients: [P; 3],
    /// Unique candidate origin; equal shapes or maps never authorize substitution.
    id: u64,
}

impl<'h, 'p, P: Scalar> Bound<'h, 'p, P> {
    /// Construct after all binding checks; inaccessible outside the crate.
    pub(crate) const fn new(
        heap: &'h mut Heap<P>,
        plan: &'p Plan<Checked>,
        maps: Maps,
        coefficients: [P; 3],
        id: u64,
    ) -> Self {
        Self {
            heap,
            plan,
            maps,
            coefficients,
            id,
        }
    }

    /// Form a dimensionless view with explicit two-sided operator normalization.
    ///
    /// # Errors
    /// Refuses rounded reference underflow, overflow or nonfinite normalized data.
    pub fn normalized(&self) -> Result<NormalizedView<'_, P>, Error> {
        let [source, resistance, variance] = self.coefficients;
        let x = self
            .maps
            .solution
            .each_ref()
            .map(|map| P::rational(map.scale()));
        let y = self
            .maps
            .equations
            .each_ref()
            .map(|map| P::rational(map.scale()));
        if x.iter()
            .chain(&y)
            .any(|scale| !scale.finite() || *scale <= P::ZERO)
        {
            return Err(Error::Map);
        }
        let matrix = [
            [x[0] / y[0], -resistance * x[1] / y[0]],
            [x[0] / y[1], P::ZERO],
        ];
        let rhs = [P::ZERO, source / y[1]];
        let source_map = [P::ZERO, P::ONE];
        let variance = variance / y[1] / y[1];
        if !variance.finite()
            || matrix
                .iter()
                .flatten()
                .chain(&rhs)
                .chain(&source_map)
                .any(|value| !value.finite())
        {
            return Err(Error::Numerical);
        }
        Ok(NormalizedView {
            matrix,
            rhs,
            source_map,
            variance,
            binding: self.id,
            borrow: PhantomData,
        })
    }

    /// Restore and publish a candidate together with its full covariance atomically.
    ///
    /// The binding is consumed; a retained candidate cannot match another binding identity.
    /// This proves semantic admission only, not accuracy or statistical validity.
    ///
    /// # Errors
    /// Refuses foreign candidates, output contracts, CAS failures or nonfinite restoration.
    pub fn publish(
        self,
        candidate: &Candidate<P>,
        contract: OutputContract,
    ) -> Result<Published<P>, Error> {
        if candidate.binding != self.id {
            return Err(Error::Map);
        }
        let layout = self.plan.layout();
        if contract.solution != layout.solution || contract.covariance != layout.covariance {
            return Err(Error::Shape);
        }
        self.plan.publication_check(self.id)?;
        let center = [
            self.maps.solution[0].restore(candidate.center[0])?,
            self.maps.solution[1].restore(candidate.center[1])?,
        ];
        let scales = self
            .maps
            .solution
            .each_ref()
            .map(|map| P::rational(map.scale()));
        let mut covariance = candidate.covariance;
        for column in 0..2 {
            for row in 0..2 {
                let index = row + 2 * column;
                covariance[index] = covariance[index] * scales[row] * scales[column];
            }
        }
        let (value, companion) = self.heap.publish_pair(
            contract.solution,
            center.to_vec(),
            contract.covariance,
            covariance.to_vec(),
        )?;
        Ok(Published {
            value,
            companion,
            normalized_center: candidate.center,
            normalized_covariance: candidate.covariance,
        })
    }
}

/// Dimensionless numerical access lent only by a validated binding.
#[derive(Debug)]
pub struct NormalizedView<'a, P> {
    /// Two-by-two normalized matrix in explicit row arrays.
    matrix: [[P; 2]; 2],
    /// Two normalized right-hand-side coordinates.
    rhs: [P; 2],
    /// Dimensionless dependence of the normalized RHS on the normalized source.
    source_map: [P; 2],
    /// Source covariance normalized by the paired equation reference.
    variance: P,
    /// Binding that owns the normalization contract.
    binding: u64,
    /// Keep copied numeric arrays within the validated borrow's lifetime.
    borrow: PhantomData<&'a ()>,
}

impl<P> NormalizedView<'_, P> {
    /// Dimensionless operator coefficients.
    #[must_use]
    pub const fn matrix(&self) -> &[[P; 2]; 2] {
        &self.matrix
    }

    /// Dimensionless right-hand side.
    #[must_use]
    pub const fn rhs(&self) -> &[P; 2] {
        &self.rhs
    }
}

/// Unpublished, binding-specific numerical output with private coefficients.
///
/// A candidate is not a published value.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::{Candidate, Published};
/// fn misuse(candidate: Candidate<f64>) -> Published<f64> { candidate }
/// ```
///
/// ```
/// use nrz_descriptor_prototype::{resistor_fixture, solve, OutputContract, Published};
/// let mut fixture = resistor_fixture::<f64>()?;
/// let plan = fixture.plan.check()?;
/// let contract = OutputContract::from(plan.layout());
/// let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
/// let bound = binding.validate(&mut fixture.heap)?;
/// let candidate = solve(&bound.normalized()?)?;
/// let published: Published<f64> = bound.publish(&candidate, contract)?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
#[derive(Debug)]
pub struct Candidate<P> {
    /// Owning binding and map identity.
    binding: u64,
    /// Normalized centers, inaccessible before publication.
    center: [P; 2],
    /// Full normalized covariance in logical column-major order.
    covariance: [P; 4],
}

/// Proposed public result contracts must still agree with checked output shapes.
#[derive(Clone, Copy, Debug)]
pub struct OutputContract {
    /// Heterogeneous center-vector shape.
    pub solution: ShapeId,
    /// Paired covariance shape.
    pub covariance: ShapeId,
}

impl From<crate::Layout> for OutputContract {
    fn from(layout: crate::Layout) -> Self {
        Self {
            solution: layout.solution,
            covariance: layout.covariance,
        }
    }
}

/// Atomically admitted result identities; no numerical certificate is implied.
#[derive(Debug)]
pub struct Published<P> {
    /// Published center value.
    value: Handle<Value, P>,
    /// Published covariance with both targets bound to the new value version.
    companion: Handle<Companion, P>,
    /// Normalized center retained for comparison after publication succeeds.
    normalized_center: [P; 2],
    /// Normalized covariance retained for comparison after publication succeeds.
    normalized_covariance: [P; 4],
}

impl<P: Scalar> Published<P> {
    /// The center value's typed handle.
    #[must_use]
    pub const fn value(&self) -> Handle<Value, P> {
        self.value
    }

    /// The covariance's typed handle.
    #[must_use]
    pub const fn companion(&self) -> Handle<Companion, P> {
        self.companion
    }

    /// The admitted normalized center, for the comparison fixture.
    #[must_use]
    pub const fn normalized_center(&self) -> &[P; 2] {
        &self.normalized_center
    }

    /// The admitted normalized covariance, for the comparison fixture.
    #[must_use]
    pub const fn normalized_covariance(&self) -> &[P; 4] {
        &self.normalized_covariance
    }
}

/// Solve only normalized coordinates and propagate the single shared source linearly.
///
/// Physical coordinates cannot be supplied by choosing the same scalar precision.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::{difference_view, resistor_fixture, solve};
/// let fixture = resistor_fixture::<f64>()?;
/// let physical = difference_view(&fixture.heap, fixture.inputs.source)?;
/// let candidate = solve(&physical)?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
///
/// ```
/// use nrz_descriptor_prototype::{resistor_fixture, solve};
/// let mut fixture = resistor_fixture::<f64>()?;
/// let plan = fixture.plan.check()?;
/// let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
/// let bound = binding.validate(&mut fixture.heap)?;
/// let candidate = solve(&bound.normalized()?)?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
///
/// # Errors
/// Refuses zero or nonfinite determinants and nonfinite outputs.
pub fn solve<P: Scalar>(view: &NormalizedView<'_, P>) -> Result<Candidate<P>, Error> {
    let [[a, b], [c, d]] = view.matrix;
    let determinant = a * d - b * c;
    if determinant == P::ZERO || !determinant.finite() {
        return Err(Error::Numerical);
    }
    let apply = |rhs: [P; 2]| {
        [
            (d * rhs[0] - b * rhs[1]) / determinant,
            (a * rhs[1] - c * rhs[0]) / determinant,
        ]
    };
    let center = apply(view.rhs);
    let sensitivity = apply(view.source_map);
    let covariance = [
        sensitivity[0] * sensitivity[0] * view.variance,
        sensitivity[1] * sensitivity[0] * view.variance,
        sensitivity[0] * sensitivity[1] * view.variance,
        sensitivity[1] * sensitivity[1] * view.variance,
    ];
    if center
        .iter()
        .chain(&covariance)
        .any(|value| !value.finite())
    {
        return Err(Error::Numerical);
    }
    Ok(Candidate {
        binding: view.binding,
        center,
        covariance,
    })
}

#[cfg(test)]
mod tests {
    //! Exercise publication against corrupt internal numerical candidates.

    use crate::{Error, OutputContract, resistor_fixture, solve};

    /// Publication checks both arrays even if an internal kernel supplied invalid data.
    ///
    /// ´test:unit:publication-refuses-nonfinite-candidates´
    #[test]
    fn publication_refuses_nonfinite_candidates() -> Result<(), Error> {
        for corrupt_center in [true, false] {
            let mut fixture = resistor_fixture::<f64>()?;
            let plan = fixture.plan.check()?;
            let count = fixture.heap.live_count();
            let contract = OutputContract::from(plan.layout());
            let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
            let bound = binding.validate(&mut fixture.heap)?;
            let mut candidate = solve(&bound.normalized()?)?;
            if corrupt_center {
                candidate.center[0] = f64::INFINITY;
            } else {
                candidate.covariance[0] = f64::NAN;
            }
            assert!(matches!(
                bound.publish(&candidate, contract),
                Err(Error::Numerical)
            ));
            assert_eq!(fixture.heap.live_count(), count);
        }
        Ok(())
    }
}
