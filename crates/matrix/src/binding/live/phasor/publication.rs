// SPDX-License-Identifier: AGPL-3.0-only
//! Complete output declarations and their original source closure precede one atomic heap publication.

pub(super) mod check;
use super::{AT, Heap, HeapError, PhasorError, PhasorEvaluation, PhasorProduct};
use crate::{
    CompanionAdapter, DependencyPolicy, Extent, ReadingCoverage, ReadingImport, ReadingRegion,
};
use nrz_bindings::readings::algebra::{Cas, limits::BudgetKind};
use nrz_bindings::{Budget, readings::PhasorResultDefinition};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Fully predeclared result family, scalar copy companions and original operation/source closure.
#[derive(Clone, Debug)]
pub struct PhasorPublication<P: Scalar>(Rc<Plan<P>>);

/// All exact metadata exists before numerical execution and cannot change between product and commit.
#[derive(Debug)]
struct Plan<P: Scalar> {
    /// Original checked multiplication and all live input versions.
    product: PhasorProduct<P>,
    /// Actual native product and fresh whole-family output slot.
    definition: PhasorResultDefinition,
    /// Shared exact graph containing every input and output declaration.
    graph: Rc<Cas>,
    /// Complete result value, vector and scalar-copy views with their overlap proofs.
    coverage: ReadingCoverage<usize>,
    /// Exactly one total and numerical-error parameter for each selected result copy.
    companions: Vec<CompanionAdapter<usize>>,
    /// Flattened original live-source closure; no recursive validation is needed.
    sources: Vec<super::super::selected::SelectedReading>,
    /// Retained metadata and evidence allowance beside evaluation and import scratch.
    bytes: u64,
}

impl<P: Scalar> PhasorPublication<P> {
    /// Check the complete fresh output family and all its companions before numerical access.
    ///
    /// Region zero contains the four block entries, region one their totals and region two
    /// their numerical errors. Output parameters use only checked scalar identity maps.
    /// # Errors
    /// Refuses reused or substituted operations, output roots, physical kinds, maps and incomplete views.
    pub fn new(
        product: PhasorProduct<P>,
        definition: PhasorResultDefinition,
        coverage: ReadingCoverage<usize>,
        companions: Vec<CompanionAdapter<usize>>,
        budget: &mut Budget,
    ) -> Result<Self, PhasorError> {
        product
            .correspondence()
            .check_operands(definition.correspondence().operands(), budget)?;
        if product.correspondence().original() != definition.operation()
            || product.correspondence().lowered() != definition.correspondence().lowered()
        {
            return Err(PhasorError::Identity);
        }
        check::coverage(definition.output(), &coverage, &companions, budget)?;
        let ancestors = sources(product.operands(), budget)?;
        let bytes = check::storage(&coverage, &companions, ancestors.len(), budget)?;
        let graph = Rc::clone(&product.operands()[0].whole().group.graph);
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Plan {
            product,
            definition,
            graph,
            coverage,
            companions,
            sources: ancestors,
            bytes,
        })))
    }
    /// Actual checked product, retaining every selected source and companion.
    #[must_use]
    pub fn product(&self) -> &PhasorProduct<P> {
        &self.0.product
    }
    /// Native operation and its fresh physical output interpretation.
    #[must_use]
    pub fn definition(&self) -> &PhasorResultDefinition {
        &self.0.definition
    }
    /// Every result view declared before execution, including the vector and all scalar copies.
    #[must_use]
    pub fn coverage(&self) -> &ReadingCoverage<usize> {
        &self.0.coverage
    }
    /// Complete ordered output parameter adapters, prepared before execution.
    #[must_use]
    pub fn companions(&self) -> &[CompanionAdapter<usize>] {
        &self.0.companions
    }
    /// Conservative retained metadata and exact evidence allowance.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// Complete atomically published family together with the numerical certificate that initialized it.
#[derive(Clone, Debug)]
pub struct PublishedPhasor<P: Scalar> {
    /// Every result reading in the predeclared order, all published together.
    readings: ReadingImport<P>,
    /// Checked actual native product with independent per-copy numerical coverage.
    evaluation: PhasorEvaluation<P>,
}
impl<P: Scalar> PublishedPhasor<P> {
    /// All phasor, vector and scalar views of the same initialized output family.
    #[must_use]
    pub const fn readings(&self) -> &ReadingImport<P> {
        &self.readings
    }
    /// Numerical proof tied to the original operation and every source version.
    #[must_use]
    pub const fn evaluation(&self) -> &PhasorEvaluation<P> {
        &self.evaluation
    }
}

impl Heap {
    /// Evaluate and publish a complete predeclared phasor product family in one heap commit.
    ///
    /// Values, deterministic totals, numerical errors and all views become available together.
    /// Every output retains the actual operation and the flattened original source closure.
    /// # Errors
    /// Refuses stale inputs, arithmetic, copy coverage, exact-resource or allocation failures atomically.
    pub fn publish_phasor_product<P: Scalar + 'static>(
        &mut self,
        publication: &PhasorPublication<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<PublishedPhasor<P>, PhasorError> {
        let plan = &publication.0;
        for group in self.symbols.groups() {
            for reading in group.coverage.readings() {
                budget.work(1, AT)?;
                if reading.reading().family().root()
                    == plan.definition.output().reading().family().root()
                {
                    return Err(PhasorError::Identity);
                }
            }
        }
        budget.with_retained_payload(plan.bytes, AT, |budget| {
            let evaluation = self.evaluate_phasor_product(&plan.product, context, budget)?;
            let regions = vec![
                ReadingRegion {
                    extent: Extent::new(2, 2)?,
                    values: evaluation.centers(self, context)?.to_vec(),
                },
                ReadingRegion {
                    extent: Extent::new(4, 1)?,
                    values: evaluation.totals(self, context)?.to_vec(),
                },
                ReadingRegion {
                    extent: Extent::new(4, 1)?,
                    values: evaluation.errors(self, context)?.to_vec(),
                },
            ];
            let provenance = super::super::import::ImportProvenance {
                definition: super::super::PhasorDefinition::Product(plan.definition.clone()),
                sources: plan.sources.clone(),
                coverage: evaluation.coverage(),
            };
            let readings = self.import_readings_with_provenance(
                Rc::clone(&plan.graph),
                plan.coverage.clone(),
                plan.companions.clone(),
                &regions,
                DependencyPolicy::WholeBase,
                Some(provenance),
                budget,
            )?;
            Ok(PublishedPhasor {
                readings,
                evaluation,
            })
        })
    }
}

/// Flatten and deduplicate existing source ancestors under the same finite caller ledger.
pub(super) fn sources<P: Scalar>(
    operands: &[super::PhasorOperand<P>; 2],
    budget: &mut Budget,
) -> Result<Vec<super::super::selected::SelectedReading>, PhasorError> {
    let mut result: Vec<super::super::selected::SelectedReading> = Vec::new();
    for operand in operands {
        for binding in std::iter::once(operand.whole()).chain(operand.copies()) {
            let selected = binding.selected();
            for source in std::iter::once(&selected).chain(&binding.group.antecedents) {
                let count = result
                    .len()
                    .checked_add(1)
                    .ok_or(HeapError::ExtentOverflow)?;
                super::super::admission::charge::<super::super::selected::SelectedReading>(
                    count, budget,
                )?;
                if !result.iter().any(|prior| {
                    Rc::ptr_eq(&prior.group, &source.group) && prior.index == source.index
                }) {
                    result.try_reserve(1).map_err(|_| HeapError::Capacity)?;
                    result.push(source.clone());
                }
            }
        }
    }
    Ok(result)
}
