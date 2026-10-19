// SPDX-License-Identifier: AGPL-3.0-only
//! Public case fixtures construct syntax only; expected mathematical values come from tests.

use super::budgets::{derivation_budget as budget, derivation_request_limits};
use super::builders::{self, admit};
use super::common::{TestResult, real};
use crate::test_api::domain::{
    AmbientDomain, BoundaryDraft, CaseArmDraft, CasesDraft, ContextDraft, HeaderDraft, RegionDraft,
    RegionExpr,
};
use crate::test_api::proof::Statement;
use crate::test_api::request::{CoveredInput, Outcome, Request, SearchResult, Task};
use crate::test_api::source::{OriginalForm, SourceClauseDraft, SourceFormDraft, SourceFormKind};
use crate::test_api::syntax::{Draft, Field, Operator, PredicateKind};
use crate::test_api::{Cas, Handle, Reference};

/// One real axis with an original source and an explicit ambient root context.
pub(crate) struct Fixture {
    /// Graph under test.
    pub(crate) cas: Cas,
    /// Actual declaration signature.
    pub(crate) header: Handle,
    /// Root proof context.
    pub(crate) context: Handle,
    /// Whole real axis.
    pub(crate) domain: Handle,
    /// Original source provenance.
    pub(crate) source: Handle,
    /// Declared real coordinate.
    pub(crate) x: Handle,
    /// Exact zero.
    pub(crate) zero: Handle,
    /// Exact one.
    pub(crate) one: Handle,
}

impl Fixture {
    /// Build the exact semantic and original-source roots before exercising any producer.
    pub(crate) fn new() -> TestResult<Self> {
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let (header, domain, context) = roots(&mut cas)?;
        let source = admit(
            &mut cas,
            Draft::Source {
                key: "cases-source".into(),
                text: "original case clauses".into(),
            },
            &mut budget()?,
        )?;
        let x = builders::expression(
            &mut cas,
            &context,
            Operator::Free("x".into()),
            real(),
            &[],
            &mut budget()?,
        )?;
        let zero = builders::number(&mut cas, &context, real(), "0", budget)?;
        let one = builders::number(&mut cas, &context, real(), "1", budget)?;
        Ok(Self {
            cas,
            header,
            context,
            domain,
            source,
            x,
            zero,
            one,
        })
    }

    /// Construct an exact scalar expression with ordered strict operands.
    pub(crate) fn term(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        builders::expression(
            &mut self.cas,
            &self.context,
            operator,
            real(),
            operands,
            &mut budget()?,
        )
    }

    /// Exact rational literal, without calling a mathematical producer.
    pub(crate) fn number(&mut self, text: &str) -> TestResult<Handle> {
        builders::number(&mut self.cas, &self.context, real(), text, budget)
    }

    /// Typed selector retains each scalar argument.
    pub(crate) fn predicate(
        &mut self,
        kind: PredicateKind,
        arguments: &[Handle],
    ) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: arguments.iter().cloned().map(Reference::Existing).collect(),
            },
            &mut budget()?,
        )
    }

    /// Build defined-and-true or defined-and-false membership on the complete axis.
    pub(crate) fn region(&mut self, predicate: &Handle, truth: bool) -> TestResult<Handle> {
        let base = Reference::Existing(self.domain.clone());
        let predicate = Reference::Existing(predicate.clone());
        let expression = if truth {
            RegionExpr::GuardTrue { base, predicate }
        } else {
            RegionExpr::GuardFalse { base, predicate }
        };
        admit(
            &mut self.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                expression,
            }),
            &mut budget()?,
        )
    }

    /// Admit unordered associations, making no claim that they agree or cover their domain.
    pub(crate) fn cases(&mut self, pairs: &[(Handle, Handle)]) -> TestResult<Handle> {
        let mut arms = Vec::new();
        for (region, value) in pairs {
            arms.push(Reference::Existing(admit(
                &mut self.cas,
                Draft::CaseArm(CaseArmDraft {
                    placements: Vec::new(),
                    header: Reference::Existing(self.header.clone()),
                    scope: Vec::new(),
                    region: Reference::Existing(region.clone()),
                    value: Reference::Existing(value.clone()),
                }),
                &mut budget()?,
            )?));
        }
        admit(
            &mut self.cas,
            Draft::Cases(CasesDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                domain: Reference::Existing(self.domain.clone()),
                arms,
            }),
            &mut budget()?,
        )
    }

    /// Preserve the independently supplied clause order in an original occurrence tree.
    pub(crate) fn ordered(
        &mut self,
        pairs: &[(Handle, Handle)],
        otherwise: Option<Handle>,
    ) -> TestResult<OriginalForm> {
        let clauses = pairs
            .iter()
            .map(|(selector, value)| SourceClauseDraft {
                selector: Reference::Existing(selector.clone()),
                value: Reference::Existing(value.clone()),
            })
            .collect();
        Ok(crate::test_api::source::ordered_cases(
            &mut self.cas,
            SourceFormDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind: SourceFormKind::OrderedCases,
                domain: Reference::Existing(self.domain.clone()),
                clauses,
                otherwise: otherwise.map(Reference::Existing),
            },
            &self.source,
            &mut budget()?,
        )?)
    }

    /// Complete original coverage is a required input to elaboration.
    pub(crate) fn elaborate_request(&self, form: &OriginalForm) -> TestResult<Request> {
        let receipt = self.cas.walk_original(&form.root, &mut budget()?)?.receipt;
        Ok(Request {
            task: Task::ElaborateCases,
            context: self.context.clone(),
            inputs: vec![CoveredInput {
                root: form.root.clone(),
                receipt,
            }],
            premises: Vec::new(),
            limits: derivation_request_limits(),
        })
    }

    /// Completely walk each generated input before forming the task.
    pub(crate) fn request(&mut self, task: Task, values: &[Handle]) -> TestResult<Request> {
        builders::request(&mut self.cas, &self.context, task, values)
    }

    /// Surface every unexpected bounded outcome as a test failure with its exact diagnostics.
    pub(crate) fn proved(&mut self, request: &Request) -> TestResult<SearchResult> {
        match self.cas.run(request) {
            Outcome::Proved { result, .. } => Ok(result),
            other => Err(format!("expected discharged case theorem, got {other:?}").into()),
        }
    }

    /// Extract only the typed elaboration endpoint, never an expected numerical value.
    pub(crate) fn elaborate(&mut self, form: &OriginalForm) -> TestResult<(Handle, SearchResult)> {
        let request = self.elaborate_request(form)?;
        let result = self.proved(&request)?;
        let Statement::Elaboration {
            result: expression, ..
        } = &result.witness.judgment().statement
        else {
            return Err("expected elaboration statement".into());
        };
        Ok((expression.clone(), result))
    }

    /// A boundary stores only the shared location, with no function ownership assumption.
    pub(crate) fn boundary(&mut self, point: &Handle) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Boundary(BoundaryDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                location: Reference::Existing(point.clone()),
            }),
            &mut budget()?,
        )
    }
}

/// Build an explicit real-axis header, domain and root context without mathematical production.
fn roots(cas: &mut Cas) -> TestResult<(Handle, Handle, Handle)> {
    let header = admit(
        cas,
        Draft::Header(HeaderDraft::Core(builders::environment(
            Field::Real,
            &["x"],
        ))),
        &mut budget()?,
    )?;
    let domain = admit(
        cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Ambient,
        }),
        &mut budget()?,
    )?;
    let context = admit(
        cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header.clone()),
            admissible: Reference::Existing(domain.clone()),
            assumptions: Vec::new(),
        }),
        &mut budget()?,
    )?;
    Ok((header, domain, context))
}
