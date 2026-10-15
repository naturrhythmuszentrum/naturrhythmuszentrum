// SPDX-License-Identifier: AGPL-3.0-only
//! Small complete stores and transactions for admission and walk metering tests.

use super::common::{TestResult, default_budget, real};
use crate::test_api::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::test_api::proof::{
    ApplicationDraft, Certificate, DomainContract, Primitive, Statement, VerifiedWitness,
};
use crate::test_api::syntax::{Draft, Environment, ExpressionDraft, OccurrenceDraft, Operator};
use crate::test_api::{Cas, Handle, Rational, Reference, limits::Limits};

/// A complete four-record original tree, optionally preceded by unrelated source bytes.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Owning store used by each independently metered operation.
    pub(crate) cas: Cas,
    /// Scalar header shared by the fixture and later expressions.
    header: Handle,
    /// Existing literal syntax reused in a transaction.
    one: Handle,
    /// Parentless original occurrence used to exercise parent-map rollback.
    pub(crate) root: Handle,
}

impl Fixture {
    /// Keep the reachable tree identical while changing only unrelated stored source data.
    pub(crate) fn new(padding: usize) -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        if padding > 0 {
            cas.admit(
                &[Draft::Source {
                    key: "padding".into(),
                    text: " ".repeat(padding),
                }],
                &mut default_budget()?,
            )?;
        }
        Self::in_store(cas)
    }

    /// Keep an earlier checked witness in the same store as the rollback fixture.
    pub(crate) fn with_witness() -> TestResult<(Self, VerifiedWitness)> {
        let mut fixture = Self::new(0)?;
        let witness = fixture.cancellation()?;
        Ok((fixture, witness))
    }

    /// Admit a domain and context using only APIs available to both harnesses.
    fn proof_context(&mut self) -> TestResult<Handle> {
        let handles = self.cas.admit(
            &[
                Draft::Region(RegionDraft {
                    header: Reference::Existing(self.header.clone()),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Assignments,
                    expression: RegionExpr::Ambient,
                }),
                Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(self.header.clone()),
                    admissible: Reference::Local(0),
                    assumptions: Vec::new(),
                }),
            ],
            &mut default_budget()?,
        )?;
        handles
            .get(1)
            .cloned()
            .ok_or_else(|| "proof context missing".into())
    }

    /// Keep a checked cancellation witness without importing the public-only proof fixtures.
    fn cancellation(&mut self) -> TestResult<VerifiedWitness> {
        let context = self.proof_context()?;
        let draft = expression(
            Reference::Existing(self.header.clone()),
            Operator::Div,
            vec![Reference::Existing(self.one.clone()); 2],
        );
        let handles = self
            .cas
            .admit(std::slice::from_ref(&draft), &mut default_budget()?)?;
        let quotient = handles.first().ok_or("quotient missing")?;
        let step = ApplicationDraft {
            rule: Primitive::Cancel,
            context: context.clone(),
            conclusion: Statement::Equality {
                left: quotient.clone(),
                right: self.one.clone(),
                domain: self.cas.admissible_region(&context)?,
                domain_contract: DomainContract::DefinedAndEqual,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        };
        Ok(self.cas.check_proof(
            &context,
            std::slice::from_ref(&step),
            &mut default_budget()?,
        )?)
    }

    /// Add the small parentless tree to an independently prepared store.
    fn in_store(mut cas: Cas) -> TestResult<Self> {
        let handles = cas.admit(
            &[
                Draft::Header(HeaderDraft::Core(Environment::default())),
                Draft::Source {
                    key: "input".into(),
                    text: "1".into(),
                },
                expression(
                    Reference::Local(0),
                    Operator::Rational(Rational::one()),
                    Vec::new(),
                ),
                occurrence(Reference::Local(2), Reference::Local(1), Vec::new()),
            ],
            &mut default_budget()?,
        )?;
        Ok(Self {
            cas,
            header: handles.first().ok_or("header missing")?.clone(),
            one: handles.get(2).ok_or("literal missing")?.clone(),
            root: handles.get(3).ok_or("occurrence missing")?.clone(),
        })
    }

    /// Add a source, syntax, occurrence and parent links, then a large source and a final small sentinel.
    pub(crate) fn batch(&self) -> Vec<Draft> {
        vec![
            Draft::Source {
                key: "batch".into(),
                text: "1+1".into(),
            },
            expression(
                Reference::Existing(self.header.clone()),
                Operator::Add,
                vec![Reference::Existing(self.one.clone()); 2],
            ),
            occurrence(
                Reference::Existing(self.one.clone()),
                Reference::Local(0),
                Vec::new(),
            ),
            occurrence(
                Reference::Local(1),
                Reference::Local(0),
                vec![Reference::Existing(self.root.clone()), Reference::Local(2)],
            ),
            Draft::Source {
                key: "tail".into(),
                text: "x".repeat(1_000),
            },
            Draft::Source {
                key: "end".into(),
                text: String::new(),
            },
        ]
    }
}

/// Keep public draft construction explicit in both test layers.
const fn expression(header: Reference, operator: Operator, operands: Vec<Reference>) -> Draft {
    Draft::Expression(ExpressionDraft {
        space: None,
        operator,
        sort: real(),
        header,
        scope: Vec::new(),
        binding: None,
        operands,
    })
}

/// Original children remain distinct occurrences even when their syntax is shared.
const fn occurrence(expression: Reference, source: Reference, children: Vec<Reference>) -> Draft {
    Draft::Occurrence(OccurrenceDraft {
        expression,
        source: Some(source),
        span: None,
        children,
        provenance: None,
        binder_names: Vec::new(),
    })
}
