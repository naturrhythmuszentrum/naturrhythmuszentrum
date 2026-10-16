// SPDX-License-Identifier: AGPL-3.0-only
//! Deferred per-stratum presentation proofs, emitted only after complete coverage has replayed.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Handle;
use crate::proof_model::{ApplicationDraft, Certificate};
use crate::rule::Primitive;
use crate::solving::SolveForm;
use crate::solving::coefficients::storage::{invalid, reserve, vector};
use crate::solving::radicals::construction::Construction;

/// Exact parent frame and previously checked regional constructor used to close one local theorem.
pub(crate) struct Close {
    /// Context in which the closed theorem is used.
    pub(crate) parent: Handle,
    /// Parent's exact coefficient region.
    pub(crate) region: Handle,
    /// Local frame being discharged.
    pub(crate) frame: Handle,
    /// Global application index of the already staged meet constructor.
    pub(crate) constructor: u32,
}

/// One postorder node retains its exact solution set and already declared semantic frame.
struct Plan {
    /// Complete branch set at this node.
    set: Handle,
    /// Exact local context before any closure.
    context: Handle,
    /// Exact region of the local presentation.
    region: Handle,
    /// Earlier postorder nodes whose closed presentation covers this set.
    children: Vec<usize>,
    /// Root nodes need no enclosing context closure.
    close: Option<Close>,
}

/// Deferred presentation is preference evidence and never contaminates a valid coverage proof.
#[derive(Default)]
pub(crate) struct Docket {
    /// Finite postorder of the explored coefficient tree.
    plans: Vec<Plan>,
}

impl Docket {
    /// Record a completed coverage proposal without claiming its requested representation.
    pub(crate) fn add(
        &mut self,
        construction: &Construction,
        set: &Handle,
        children: Vec<usize>,
        budget: &mut Budget,
    ) -> Result<usize, Fault> {
        let index = self.plans.len();
        reserve(&mut self.plans, 1, budget)?;
        self.plans.push(Plan {
            set: set.clone(),
            context: construction.context.clone(),
            region: construction.region.clone(),
            children,
            close: None,
        });
        Ok(index)
    }

    /// The coverage traversal fixes the parent frame and constructor before presentation begins.
    pub(crate) fn close(&mut self, index: usize, close: Close) -> Result<(), Fault> {
        self.plans.get_mut(index).ok_or_else(invalid)?.close = Some(close);
        Ok(())
    }

    /// Emit leaves before their parents and name each closed child proof through compatibility slots.
    pub(crate) fn emit(
        &self,
        construction: &mut Construction,
        form: SolveForm,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut proofs = vector(self.plans.len(), budget)?;
        for plan in &self.plans {
            construction.context.clone_from(&plan.context);
            construction.region.clone_from(&plan.region);
            let mut compatibility = vector(plan.children.len(), budget)?;
            for child in &plan.children {
                compatibility.push(slot(
                    construction,
                    *proofs.get(*child).ok_or_else(invalid)?,
                    budget,
                )?);
            }
            crate::solving::radicals::proof::present(
                construction,
                &plan.set,
                form,
                compatibility,
                budget,
            )?;
            let mut proof = u32::try_from(
                construction
                    .steps
                    .len()
                    .checked_sub(1)
                    .ok_or_else(invalid)?,
            )
            .map_err(|_| invalid())?;
            if let Some(close) = &plan.close {
                proof = closed(construction, close, proof, budget)?;
            }
            proofs.push(proof);
        }
        Ok(())
    }
}

/// Certificates refer to ordered same-context premises, not global DAG indices.
fn slot(construction: &Construction, index: u32, budget: &mut Budget) -> Result<u32, Fault> {
    let index = usize::try_from(index).map_err(|_| invalid())?;
    if construction.steps.get(index).ok_or_else(invalid)?.context != construction.context {
        return Err(invalid());
    }
    let prefix = construction.steps.get(..index).ok_or_else(invalid)?;
    budget.work(crate::graph::size(prefix.len(), super::AT)?, super::AT)?;
    u32::try_from(
        prefix
            .iter()
            .filter(|step| step.context == construction.context)
            .count(),
    )
    .map_err(|_| invalid())
}

/// Close only the requested local membership frame; every residual remains checker-owned.
fn closed(
    construction: &mut Construction,
    close: &Close,
    proof: u32,
    budget: &mut Budget,
) -> Result<u32, Fault> {
    let conclusion = construction
        .steps
        .last()
        .ok_or_else(invalid)?
        .conclusion
        .clone();
    construction.context.clone_from(&close.parent);
    construction.region.clone_from(&close.region);
    let index = u32::try_from(construction.steps.len()).map_err(|_| invalid())?;
    reserve(&mut construction.steps, 1, budget)?;
    construction.steps.push(ApplicationDraft {
        rule: Primitive::ContextClose,
        context: close.parent.clone(),
        conclusion,
        premises: vec![proof, close.constructor],
        substitutions: Vec::new(),
        certificate: Certificate::Close {
            frame: close.frame.clone(),
        },
    });
    Ok(index)
}
