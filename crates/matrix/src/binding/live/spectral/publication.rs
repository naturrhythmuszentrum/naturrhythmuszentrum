// SPDX-License-Identifier: AGPL-3.0-only
//! Complete predeclared conjugate results publish atomically with the original source closure.

mod check;
use super::{AT, LcSpectralEvaluation, LcSpectrum, SpectralError as Error, SpectralValue};
use crate::{
    CompanionAdapter, DependencyPolicy, Extent, Heap, HeapError, ReadingCoverage, ReadingImport,
    ReadingRegion,
};
use nrz_bindings::{
    Budget, CompanionKind,
    readings::{
        CompanionReadingKind, LcSpectralResultDefinition, RepresentationKind,
        algebra::{Cas, limits::BudgetKind},
    },
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Both roots, four Real components and every companion are fixed before numerical execution.
#[derive(Clone, Debug)]
pub struct LcSpectralPublication<P: Scalar>(Rc<Plan<P>>);
/// Shared complete numerical producer and immutable output inventory.
#[derive(Debug)]
struct Plan<P: Scalar> {
    /// Original scalar sources and exact lowering schedule.
    spectrum: LcSpectrum<P>,
    /// Checked original operation and six output interpretations.
    definition: LcSpectralResultDefinition,
    /// Shared exact graph, retained without copying its proof state.
    graph: Rc<Cas>,
    /// Complete value inventory and every shared-storage overlap proof.
    coverage: ReadingCoverage<usize>,
    /// Both own-target radius channels for each nonzero result.
    companions: Vec<CompanionAdapter<usize>>,
    /// Flattened original source closure, including earlier producers.
    sources: Vec<crate::binding::live::selected::SelectedReading>,
    /// Owned publication metadata allowance in bytes.
    bytes: u64,
}

impl<P: Scalar> LcSpectralPublication<P> {
    /// Check the complete result inventory, all signed maps and every own-target radius.
    /// # Errors
    /// Refuses changed operations, input reuse, incomplete views, companions and resource limits.
    pub fn new(
        spectrum: LcSpectrum<P>,
        definition: LcSpectralResultDefinition,
        coverage: ReadingCoverage<usize>,
        companions: Vec<CompanionAdapter<usize>>,
        budget: &mut Budget,
    ) -> Result<Self, Error> {
        if !spectrum.law().shares_allocation(definition.law()) {
            return Err(Error::Identity);
        }
        let root = definition.outputs()[0].reading().family().root();
        for input in spectrum.inputs() {
            budget.work(1, AT)?;
            if input.reading().family().root() == root {
                return Err(Error::Identity);
            }
        }
        check::coverage(&definition, &coverage, &companions, budget)?;
        let sources = sources(spectrum.inputs(), budget)?;
        let bytes = check::storage::<P>(&coverage, &companions, sources.len(), budget)?;
        let graph = Rc::clone(&spectrum.inputs()[0].group.graph);
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Plan {
            spectrum,
            definition,
            graph,
            coverage,
            companions,
            sources,
            bytes,
        })))
    }
    /// Original complete law and the actual L/C source bindings.
    #[must_use]
    pub fn spectrum(&self) -> &LcSpectrum<P> {
        &self.0.spectrum
    }
    /// Complete Complex roots, Real components and checked logical projections.
    #[must_use]
    pub fn definition(&self) -> &LcSpectralResultDefinition {
        &self.0.definition
    }
    /// Exact result inventory in its predeclared order.
    #[must_use]
    pub fn coverage(&self) -> &ReadingCoverage<usize> {
        &self.0.coverage
    }
    /// Each total and numerical radius retains its own exact target map.
    #[must_use]
    pub fn companions(&self) -> &[CompanionAdapter<usize>] {
        &self.0.companions
    }
    /// Owned publication metadata; the spectrum and result definition remain shared.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// One atomically initialized result family with the evidence that produced it.
#[derive(Clone, Debug)]
pub struct PublishedLcSpectrum<P: Scalar> {
    /// All six atomically published live result readings.
    readings: ReadingImport<P>,
    /// Original numerical execution and arithmetic revision.
    evaluation: LcSpectralEvaluation<P>,
}
impl<P: Scalar + 'static> PublishedLcSpectrum<P> {
    /// Both roots and all four Real component symbols, including exact no-storage zeros.
    #[must_use]
    pub const fn readings(&self) -> &ReadingImport<P> {
        &self.readings
    }
    /// Complete original numerical proof and its retained source versions.
    #[must_use]
    pub const fn evaluation(&self) -> &LcSpectralEvaluation<P> {
        &self.evaluation
    }
    /// Execute one declared signed reading over its current stored coordinate and own radii.
    /// # Errors
    /// Refuses a stale result, companion, original source, arithmetic revision or absent view.
    pub fn reading(
        &self,
        index: usize,
        heap: &Heap,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<Vec<SpectralValue<P>>, Error> {
        self.validate(heap, context)?;
        let binding = self.readings.bindings().get(index).ok_or(Error::Identity)?;
        let count = binding.reading().entries().len();
        let output = count
            .checked_mul(std::mem::size_of::<SpectralValue<P>>())
            .ok_or(HeapError::ExtentOverflow)?;
        let scratch = output
            .checked_add((count * 2 + 1) * std::mem::size_of::<P>())
            .ok_or(HeapError::ExtentOverflow)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            u64::try_from(scratch).map_err(|_| HeapError::ExtentOverflow)?,
            AT,
        )?;
        budget.check(
            BudgetKind::OutputBytes,
            u64::try_from(output).map_err(|_| HeapError::ExtentOverflow)?,
            AT,
        )?;
        budget.work(1, AT)?;
        if binding.adapter().representation().kind() == RepresentationKind::None {
            return Ok(vec![SpectralValue {
                center: P::ZERO,
                total: P::ZERO,
                error: P::ZERO,
            }]);
        }
        let entries = heap.reading_entries(binding)?;
        let [center] = entries.as_slice() else {
            return Err(Error::Identity);
        };
        budget.work(
            u64::try_from(binding.companions().len() * 2).map_err(|_| HeapError::ExtentOverflow)?,
            AT,
        )?;
        let total = parameter(heap, binding, CompanionKind::DeterministicBound)?;
        let error = parameter(heap, binding, CompanionKind::NumericalError)?;
        let negative = index == 1 || index == 5;
        let center = if negative { -*center } else { *center };
        let mut values = Vec::new();
        values
            .try_reserve_exact(total.len())
            .map_err(|_| HeapError::Capacity)?;
        for (coordinate, (total, error)) in total.into_iter().zip(error).enumerate() {
            budget.work(1, AT)?;
            let center = if index < 2 && coordinate == 0 {
                P::ZERO
            } else {
                center
            };
            values.push(SpectralValue {
                center,
                total,
                error,
            });
        }
        Ok(values)
    }
    /// Full first-order covariance of both published roots, ordered `(Re+, Im+, Re-, Im-)`.
    ///
    /// The enclosure targets statistical parameters only, with `LinearizedMoments` assurance.
    /// Every output view and original source is revalidated before lending this result.
    /// # Errors
    /// Refuses changed output values or radii, original values or companions, and arithmetic revisions.
    pub fn root_covariance(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<Option<[[nrz_scalar::Enclosure<P>; 4]; 4]>, Error> {
        self.validate(heap, context)?;
        self.evaluation.root_covariance(heap, context)
    }
    /// Shared signed views have exact zero copy discrepancy because they read one stored value.
    /// # Errors
    /// Refuses any stale result, parameter, original source or arithmetic revision.
    pub fn shared_discrepancy(&self, heap: &Heap, context: &ArithmeticContext) -> Result<P, Error> {
        self.validate(heap, context)?;
        Ok(P::ZERO)
    }
    /// Revalidate every original source, produced view, parameter and arithmetic revision.
    fn validate(&self, heap: &Heap, context: &ArithmeticContext) -> Result<(), Error> {
        self.evaluation.validate(heap, context)?;
        for binding in self.readings.bindings() {
            heap.validate_reading(binding)?;
        }
        Ok(())
    }
}

impl Heap {
    /// Publish both conjugate roots, their exact components and all radii in one transaction.
    /// # Errors
    /// Refuses reused output roots, stale sources, failed numerical guards and caller limits atomically.
    pub fn publish_lc_spectrum<P: Scalar + 'static>(
        &mut self,
        publication: &LcSpectralPublication<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<PublishedLcSpectrum<P>, Error> {
        let plan = &publication.0;
        let root = plan.definition.outputs()[0].reading().family().root();
        for group in self.symbols.groups() {
            for adapter in group.coverage.readings() {
                budget.work(1, AT)?;
                if adapter.reading().family().root() == root {
                    return Err(Error::Identity);
                }
            }
        }
        let owned = check::produced(plan)?;
        budget.check(BudgetKind::OutputBytes, owned, AT)?;
        budget.with_retained_payload(plan.bytes, AT, |budget| {
            let evaluation = self.evaluate_lc_spectrum(&plan.spectrum, context, budget)?;
            budget.with_retained_payload(owned, AT, |budget| {
                commit(self, plan, evaluation, context, budget)
            })
        })
    }
}

/// All metadata, output radii and region payloads are prepared before the existing atomic heap commit.
fn commit<P: Scalar + 'static>(
    heap: &mut Heap,
    plan: &Plan<P>,
    evaluation: LcSpectralEvaluation<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<PublishedLcSpectrum<P>, Error> {
    let rate = evaluation.imaginary_rates(heap, context)?[0];
    let regions = vec![
        ReadingRegion {
            extent: Extent::new(1, 1)?,
            values: vec![rate.center],
        },
        ReadingRegion {
            extent: Extent::new(6, 1)?,
            values: vec![
                P::ZERO,
                rate.total,
                P::ZERO,
                rate.total,
                rate.total,
                rate.total,
            ],
        },
        ReadingRegion {
            extent: Extent::new(6, 1)?,
            values: vec![
                P::ZERO,
                rate.error,
                P::ZERO,
                rate.error,
                rate.error,
                rate.error,
            ],
        },
    ];
    let provenance = crate::binding::live::import::ImportProvenance {
        definition: crate::binding::live::ResultDefinition::Spectral(plan.definition.clone()),
        sources: plan.sources.clone(),
        dependencies: DependencyPolicy::WholeBase,
        coverage: evaluation.coverage(),
    };
    let readings = heap.import_readings_with_provenance(
        Rc::clone(&plan.graph),
        plan.coverage.clone(),
        plan.companions.clone(),
        &regions,
        DependencyPolicy::WholeBase,
        Some(provenance),
        budget,
    )?;
    Ok(PublishedLcSpectrum {
        readings,
        evaluation,
    })
}

/// Every original source and any earlier producer ancestor remains in the flattened closure.
fn sources<P: Scalar>(
    inputs: &[crate::ReadingBinding<P>; 2],
    budget: &mut Budget,
) -> Result<Vec<crate::binding::live::selected::SelectedReading>, Error> {
    let mut result: Vec<crate::binding::live::selected::SelectedReading> = Vec::new();
    for input in inputs {
        let selected = input.selected();
        for source in std::iter::once(&selected).chain(&input.group.antecedents) {
            let count = result
                .len()
                .checked_add(1)
                .ok_or(HeapError::ExtentOverflow)?;
            crate::binding::live::admission::charge::<
                crate::binding::live::selected::SelectedReading,
            >(count, budget)?;
            if !result
                .iter()
                .any(|prior| Rc::ptr_eq(&prior.group, &source.group) && prior.index == source.index)
            {
                result.try_reserve(1).map_err(|_| HeapError::Capacity)?;
                result.push(source.clone());
            }
        }
    }
    Ok(result)
}

/// Read the parameter version attached to this exact result, never a cached radius from evaluation.
fn parameter<P: Scalar + 'static>(
    heap: &Heap,
    binding: &crate::ReadingBinding<P>,
    kind: CompanionKind,
) -> Result<Vec<P>, Error> {
    for (index, companion) in binding.companions().iter().enumerate() {
        if companion.companion().kind() == CompanionReadingKind::Radius(kind)
            && companion
                .targets()
                .iter()
                .any(|target| target.reading() == binding.reading())
        {
            return Ok(heap.reading_companion_entries(binding, index)?);
        }
    }
    Err(Error::Identity)
}
