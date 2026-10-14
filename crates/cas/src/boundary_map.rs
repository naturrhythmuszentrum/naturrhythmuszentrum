// SPDX-License-Identifier: AGPL-3.0-only
//! Complete derivative assembly retains replayed cells and a separately proved defined region.

use crate::boundary_derivative::DerivativeJob;
use crate::boundary_model::{BoundaryCertificate, BoundaryPartition, CaseDerivative};
use crate::boundary_producer::{Runner, pending};
use crate::case_syntax::{self, invalid};
use crate::derivative_model::{DerivativeMap, PendingCell};
use crate::domain_model::CasesDraft;
use crate::error::{Fault, FaultKind, InvalidCode};
use crate::graph::{Draft, Handle, Reference};
use crate::proof_model::Statement;
use crate::rule::Primitive;

/// One level's complete or partial union, independently of a later derivative order.
struct Level {
    /// Checked derivative on the union of successfully proved cells.
    proof: Option<Handle>,
    /// Every nonempty requested cell has a derivative value.
    complete: bool,
    /// Exact unresolved cells.
    pending: Vec<PendingCell>,
    /// A missing premise or resource blocker, distinct from nonexistence.
    fault: Option<Fault>,
}

/// Proposed associations carry their exact checked derivative facts.
#[derive(Default)]
struct Cells {
    /// Ordered nonempty regions with derivative values.
    regions: Vec<Handle>,
    /// Matching actual derivative associations.
    arms: Vec<Handle>,
    /// Partition followed by successful ordered cell proofs.
    proofs: Vec<Handle>,
}

impl Runner<'_> {
    /// Certify finite geometry, then iterate only actual previously assembled derivatives.
    pub(crate) fn map(
        &mut self,
        job: &DerivativeJob,
        order: u32,
    ) -> Result<(DerivativeMap, Option<Fault>), Fault> {
        let (mut map, partition, proof) = self.start_map(job, order)?;
        let mut prefix = None;
        let mut current = job.function.clone();
        for level in 1..=order {
            if self.higher_order() {
                self.deferred = Some(Vec::new());
            }
            let output = self.level(job, &current, level, prefix.as_ref(), (&partition, &proof));
            if let Err(fault) = self.flush() {
                return Ok((map, Some(classify(fault))));
            }
            let output = match output {
                Ok(output) => output,
                Err(fault) => return Ok((map, Some(classify(fault)))),
            };
            map.pending = output.pending;
            let witness = output
                .proof
                .as_ref()
                .and_then(|id| self.facts.iter().find(|fact| fact.proof() == id))
                .cloned();
            if level == order {
                map.defined.clone_from(&witness);
            }
            if !output.complete {
                if level < order {
                    for region in &partition.cells {
                        map.pending.push(PendingCell {
                            region: region.clone(),
                            order,
                            blocker: pending(),
                        });
                    }
                }
                return Ok((map, output.fault.or_else(|| (level < order).then(pending))));
            }
            if level == order {
                map.complete = witness;
            }
            prefix = output.proof;
            let Statement::CaseDerivative(value) =
                self.statement(prefix.as_ref().ok_or_else(invalid)?)?
            else {
                return Err(invalid());
            };
            current = value.derivative;
        }
        Ok((map, None))
    }

    /// Retain complete geometry independently of derivative success.
    fn start_map(
        &mut self,
        job: &DerivativeJob,
        order: u32,
    ) -> Result<(DerivativeMap, BoundaryPartition, Handle), Fault> {
        let (partition, certificate) = crate::boundary_partition::build(
            self.cas,
            &job.function,
            &job.domain,
            &job.coordinate,
            self.generation,
        )?;
        let proof = self.push(
            Primitive::JetPolynomial,
            Statement::BoundaryPartition(Box::new(partition.clone())),
            &[],
            Some(certificate),
        )?;
        self.retain(&proof)?;
        let map = DerivativeMap {
            function: job.function.clone(),
            domain: job.domain.clone(),
            coordinate: job.coordinate.clone(),
            order,
            partition: self.facts.last().ok_or_else(invalid)?.clone(),
            facts: Vec::new(),
            defined: None,
            pending: Vec::new(),
            complete: None,
        };
        Ok((map, partition, proof))
    }

    /// Inspect each inhabited cell, retaining the precise region of every pending obligation.
    fn level(
        &mut self,
        job: &DerivativeJob,
        current: &Handle,
        order: u32,
        prefix: Option<&Handle>,
        geometry: (&BoundaryPartition, &Handle),
    ) -> Result<Level, Fault> {
        let (partition, proof) = geometry;
        let mut cells = Cells::new(proof, partition.cells.len())?;
        let mut result = Level::new(partition.cells.len())?;
        for (index, region) in partition.cells.iter().enumerate() {
            if result.stopped(region, order) {
                continue;
            }
            if crate::case_obligations::empty(self.cas, region.logical_id(), self.generation)? {
                continue;
            }
            let proposal = if index & 1 == 0 {
                self.interior(job, current, region, order, prefix)
            } else {
                self.point(
                    job,
                    current,
                    partition.boundaries.get(index / 2).ok_or_else(invalid)?,
                    order,
                    prefix,
                )
            };
            self.record_cell(&mut cells, &mut result, region, order, proposal)?;
        }
        if !cells.arms.is_empty()
            && !result
                .fault
                .is_some_and(|fault| matches!(fault.kind, FaultKind::Exhausted { .. }))
        {
            result.proof = Some(self.assemble_cells(job, order, &cells, result.complete)?);
        }
        Ok(result)
    }

    /// Record a checked value, a witnessed obstruction, or the exact unresolved cell.
    fn record_cell(
        &mut self,
        cells: &mut Cells,
        result: &mut Level,
        region: &Handle,
        order: u32,
        proposal: Result<Handle, Fault>,
    ) -> Result<(), Fault> {
        match proposal {
            Ok(checked) => {
                if let Some(value) = self.cell_result(&checked)? {
                    cells.arms.push(self.derivative_arm(region, &value)?);
                    cells.regions.push(region.clone());
                    cells.proofs.push(checked);
                } else {
                    result.complete = false;
                }
            }
            Err(fault) => {
                result.complete = false;
                let blocker = classify(fault);
                result.fault = Some(blocker);
                result.pending.push(PendingCell {
                    region: region.clone(),
                    order,
                    blocker,
                });
            }
        }
        Ok(())
    }

    /// Certify a complete derivative or its independently justified region of existence.
    fn assemble_cells(
        &mut self,
        job: &DerivativeJob,
        order: u32,
        cells: &Cells,
        complete: bool,
    ) -> Result<Handle, Fault> {
        let domain = if complete {
            job.domain.clone()
        } else {
            crate::boundary_regions::union(self.cas, &job.domain, &cells.regions, self.generation)?
        };
        let derivative = self.derivative_cases(&domain, &cells.arms)?;
        let proof = self.push(
            Primitive::CaseInterior,
            Statement::CaseDerivative(Box::new(CaseDerivative {
                function: job.function.clone(),
                derivative,
                coordinate: job.coordinate.clone(),
                order,
                domain,
            })),
            &cells.proofs,
            Some(BoundaryCertificate::Assemble),
        )?;
        self.retain(&proof)?;
        Ok(proof)
    }

    /// A nonexistence statement contributes no invented derivative value.
    fn cell_result(&mut self, proof: &Handle) -> Result<Option<Handle>, Fault> {
        Ok(match self.statement(proof)? {
            Statement::CaseDerivative(value) => Some(value.derivative),
            Statement::DerivativeAt(value) => Some(value.value),
            Statement::DerivativeDoesNotExist(_) => None,
            _ => return Err(invalid()),
        })
    }

    /// Preserve the exact region for each actual derivative association.
    fn derivative_arm(&mut self, region: &Handle, value: &Handle) -> Result<Handle, Fault> {
        let metadata = self.cas.state.region(region.logical_id())?.clone();
        crate::case_ordered::arm(
            self.cas,
            metadata.header,
            &metadata.scope,
            region.logical_id(),
            value.logical_id(),
            self.generation,
        )
    }

    /// Build derivative syntax from the witnessed association family.
    fn derivative_cases(&mut self, domain: &Handle, arms: &[Handle]) -> Result<Handle, Fault> {
        let metadata = self.cas.state.region(domain.logical_id())?.clone();
        case_syntax::admit(
            self.cas,
            Draft::Cases(CasesDraft {
                header: Reference::Existing(self.cas.handle(metadata.header)),
                scope: metadata.scope,
                domain: Reference::Existing(domain.clone()),
                arms: arms.iter().cloned().map(Reference::Existing).collect(),
            }),
            self.generation,
        )
    }
}

/// Unsupported analytic fragments are pending premises; opaque geometry remains undecided.
pub(crate) const fn classify(mut fault: Fault) -> Fault {
    if let FaultKind::Invalid(code) = &mut fault.kind {
        *code = match *code {
            InvalidCode::UnsupportedFragment
            | InvalidCode::PolynomialFragment
            | InvalidCode::CoefficientField
            | InvalidCode::RefutedDomain => InvalidCode::PendingPremise,
            other => other,
        };
    }
    fault
}

impl Cells {
    /// Reserve the already bounded partition size before retaining any cell result.
    fn new(proof: &Handle, count: usize) -> Result<Self, Fault> {
        let mut result = Self {
            proofs: vec![proof.clone()],
            ..Self::default()
        };
        result
            .regions
            .try_reserve(count)
            .map_err(|_| allocation())?;
        result.arms.try_reserve(count).map_err(|_| allocation())?;
        result.proofs.try_reserve(count).map_err(|_| allocation())?;
        Ok(result)
    }
}

impl Level {
    /// Pending cells use the same finite size bound as the complete partition.
    fn new(count: usize) -> Result<Self, Fault> {
        let mut pending = Vec::new();
        pending.try_reserve(count).map_err(|_| allocation())?;
        Ok(Self {
            proof: None,
            complete: true,
            pending,
            fault: None,
        })
    }

    /// Exhaustion preserves all subsequent cells as explicitly unattempted obligations.
    fn stopped(&mut self, region: &Handle, order: u32) -> bool {
        if let Some(blocker) = self.fault
            && matches!(blocker.kind, FaultKind::Exhausted { .. })
        {
            self.pending.push(PendingCell {
                region: region.clone(),
                order,
                blocker,
            });
            true
        } else {
            false
        }
    }
}

/// Allocation failure never becomes an incomplete mathematical theorem.
const fn allocation() -> Fault {
    Fault::allocation(crate::error::Location::new("derivative cells"))
}
