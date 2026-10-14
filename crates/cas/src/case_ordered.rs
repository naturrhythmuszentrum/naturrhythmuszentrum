// SPDX-License-Identifier: AGPL-3.0-only
//! Checked ordered-source recurrence without implicit guard fallthrough.

use crate::budget::Budget;
use crate::case_model::CaseCertificate;
use crate::case_syntax::{self, invalid};
use crate::domain_model::{CaseArmDraft, CasesDraft, RegionExpr};
use crate::error::Fault;
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference};
use crate::proof_model::{CheckedApplication, ScopedConditions};
use crate::source_model::{SourceForm, SourceFormKind};

/// Reconstructed output and the source-order association evidence.
pub(crate) struct Construction {
    /// Native unordered case resulting from all original clauses.
    pub(crate) result: Handle,
    /// Exact reached-region and association map.
    pub(crate) certificate: CaseCertificate,
    /// Reached guard and branch requirements, including still-open predicates.
    pub(crate) conditions: ScopedConditions,
}

/// One source-order recurrence under reconstruction.
struct Build {
    /// Region where the next original selector is reached.
    reached: LogicalId,
    /// Reconstructed ordered roles.
    certificate: CaseCertificate,
    /// Requirements under their actual reached guards.
    conditions: ScopedConditions,
}

/// Reconstruct the source signature and every association from immutable syntax.
pub(crate) fn construct(
    cas: &mut Cas,
    context: LogicalId,
    source: LogicalId,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Construction, Fault> {
    let form = cas.state.source_form(source)?.clone();
    let mut build = Build {
        reached: form.domain,
        conditions: ScopedConditions::default(),
        certificate: CaseCertificate {
            regions: vec![cas.handle(form.domain)],
            ..CaseCertificate::default()
        },
    };
    for clause in &form.clauses {
        build.clause(cas, context, &form, *clause, premises, budget)?;
    }
    if let Some(value) = form.otherwise {
        crate::case_obligations::defined(
            cas,
            context,
            build.reached,
            value,
            premises,
            &mut build.conditions,
            budget,
        )?;
        build.certificate.arms.push(arm(
            cas,
            form.header,
            &form.scope,
            build.reached,
            value,
            budget,
        )?);
    }
    build.finish(cas, &form, budget)
}

impl Build {
    /// Retain every original branch's value requirements, including empty guarded ones.
    fn clause(
        &mut self,
        cas: &mut Cas,
        context: LogicalId,
        form: &SourceForm,
        clause: (LogicalId, LogicalId),
        premises: &[CheckedApplication],
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let (selector, value) = clause;
        let effective = if form.kind == SourceFormKind::OrderedCases {
            self.guard(cas, context, selector, premises, budget)?
        } else {
            selector
        };
        let local = case_syntax::meet(cas, form.domain, &[form.domain, effective], budget)?;
        crate::case_obligations::defined(
            cas,
            context,
            local,
            value,
            premises,
            &mut self.conditions,
            budget,
        )?;
        self.certificate.arms.push(arm(
            cas,
            form.header,
            &form.scope,
            effective,
            value,
            budget,
        )?);
        Ok(())
    }

    /// Defined-and-false recurrence never treats an undefined guard as a false guard.
    fn guard(
        &mut self,
        cas: &mut Cas,
        context: LogicalId,
        selector: LogicalId,
        premises: &[CheckedApplication],
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let expression = case_syntax::guard_expression(cas, selector, budget)?;
        crate::case_obligations::defined(
            cas,
            context,
            self.reached,
            expression,
            premises,
            &mut self.conditions,
            budget,
        )?;
        let effective = case_syntax::guarded(cas, self.reached, selector, budget)?;
        let otherwise = RegionExpr::GuardFalse {
            base: Reference::Existing(cas.handle(self.reached)),
            predicate: Reference::Existing(cas.handle(selector)),
        };
        self.reached = case_syntax::region(cas, self.reached, otherwise, budget)?;
        self.certificate.regions.push(cas.handle(self.reached));
        Ok(effective)
    }

    /// Native association order has no first-match meaning after the source recurrence is fixed.
    fn finish(
        self,
        cas: &mut Cas,
        form: &SourceForm,
        budget: &mut Budget,
    ) -> Result<Construction, Fault> {
        if self.certificate.arms.is_empty() {
            return Err(invalid());
        }
        let draft = Draft::Cases(CasesDraft {
            header: Reference::Existing(cas.handle(form.header)),
            scope: form.scope.clone(),
            domain: Reference::Existing(cas.handle(form.domain)),
            arms: self
                .certificate
                .arms
                .iter()
                .cloned()
                .map(Reference::Existing)
                .collect(),
        });
        let result = case_syntax::admit(cas, draft, budget)?;
        Ok(Construction {
            result,
            certificate: self.certificate,
            conditions: self.conditions,
        })
    }
}

/// Retain one exact region/value association, with no source-order priority in native syntax.
pub(crate) fn arm(
    cas: &mut Cas,
    header: LogicalId,
    scope: &crate::model::Scope,
    region: LogicalId,
    value: LogicalId,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let draft = Draft::CaseArm(CaseArmDraft {
        header: Reference::Existing(cas.handle(header)),
        scope: scope.clone(),
        region: Reference::Existing(cas.handle(region)),
        value: Reference::Existing(cas.handle(value)),
    });
    case_syntax::admit(cas, draft, budget)
}
