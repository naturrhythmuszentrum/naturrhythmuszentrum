// SPDX-License-Identifier: AGPL-3.0-only
//! Public distribution fixtures construct syntax and replay proofs without computing expectations.

use super::budgets::derivation_budget as budget;
use super::builders::admit;
use super::cases;
use super::common::{TestResult, real};
use crate::test_api::domain::{ContextDraft, Layer, SpaceDraft};
use crate::test_api::proof::{DistributionStatement, Statement};
use crate::test_api::request::{ActionMode, SearchResult, Task};
use crate::test_api::syntax::{Draft, ExpressionDraft, Field, Operator, Sort};
use crate::test_api::{Cas, Handle, Reference};

/// One exact real distribution space over the existing source-aware fixture.
pub(crate) struct Fixture {
    /// Scalar syntax, context and original source.
    pub(crate) base: cases::Fixture,
    /// Exact open real space.
    pub(crate) space: Handle,
}

impl Fixture {
    /// Construct the whole real axis without asserting integrability or differentiation.
    pub(crate) fn new() -> TestResult<Self> {
        let mut base = cases::Fixture::new()?;
        let space = admit(
            &mut base.cas,
            Draft::Space(SpaceDraft {
                header: Reference::Existing(base.header.clone()),
                domain: Reference::Existing(base.domain.clone()),
                coordinate: "x".into(),
                field: Field::Real,
                test_family: "CompactSmooth".into(),
            }),
            &mut budget()?,
        )?;
        Ok(Self { base, space })
    }

    /// Construct an explicit distribution, test or action node.
    pub(crate) fn node(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        let sort = match operator {
            Operator::TestDerivative { .. } | Operator::TestMultiply => Sort::Test,
            Operator::Pair
            | Operator::PairNearSupport
            | Operator::TestApply
            | Operator::RegularAction
            | Operator::Neg
            | Operator::Add
            | Operator::Sub
            | Operator::Mul => real(),
            _ => Sort::Distribution,
        };
        admit(
            &mut self.base.cas,
            Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.base.header.clone()),
                space: Some(Reference::Existing(self.space.clone())),
                scope: Vec::new(),
                sort,
                operator,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
                binding: None,
            }),
            &mut budget()?,
        )
    }

    /// Declare one ordinary compact smooth test with its exact earlier space binding.
    pub(crate) fn test(&mut self) -> TestResult<Handle> {
        let (space, header, test) = super::case_spaces::space(
            &mut self.base.cas,
            &self.base.header,
            &self.base.domain,
            "x",
        )?;
        assert_eq!(space, self.space);
        let context = admit(
            &mut self.base.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(self.base.domain.clone()),
                assumptions: Vec::new(),
            }),
            &mut budget()?,
        )?;
        self.base.header = header;
        self.base.context = context;
        Ok(test)
    }

    /// Delta syntax has an independently selected rational location and derivative order.
    pub(crate) fn delta(&mut self, point: &str, order: u32) -> TestResult<Handle> {
        let point = self.base.number(point)?;
        self.node(Operator::Delta { order }, &[point])
    }

    /// Request explicit checked regular embedding.
    pub(crate) fn embed(&mut self, function: &Handle) -> TestResult<(Handle, SearchResult)> {
        let request = self.base.request(
            Task::EmbedRegular {
                space: self.space.clone(),
            },
            std::slice::from_ref(function),
        )?;
        let result = self.base.proved(&request)?;
        let DistributionStatement::Embedding { embedding, .. } = statement(&result)? else {
            return Err("embedding theorem".into());
        };
        Ok((embedding.clone(), result))
    }

    /// Request an actual distribution derivative on the exact space interval.
    pub(crate) fn derivative(&mut self, input: &Handle, order: u32) -> TestResult<SearchResult> {
        let request = self.base.request(
            Task::Differentiate {
                coordinate: "x".into(),
                domain: self.base.domain.clone(),
                layer: Layer::Distribution,
                order,
            },
            std::slice::from_ref(input),
        )?;
        self.base.proved(&request)
    }

    /// Explicit supported polynomial action with the entire axis as smooth neighborhood.
    pub(crate) fn pair(&mut self, input: &Handle, function: &Handle) -> TestResult<SearchResult> {
        let request = self.base.request(
            Task::Pair {
                space: self.space.clone(),
                action_mode: ActionMode::NearSupport {
                    neighborhood: self.base.domain.clone(),
                },
            },
            &[input.clone(), function.clone()],
        )?;
        self.base.proved(&request)
    }

    /// Exact coefficient construction uses hand-specified rational data.
    pub(crate) fn scale(&mut self, coefficient: &str, input: &Handle) -> TestResult<Handle> {
        let factor = self.base.number(coefficient)?;
        self.node(Operator::DistributionScale, &[factor, input.clone()])
    }
}

/// Borrow only the public typed statement.
pub(crate) fn statement(result: &SearchResult) -> TestResult<&DistributionStatement> {
    let Statement::Distribution(value) = &result.witness.judgment().statement else {
        return Err("distribution statement".into());
    };
    Ok(value)
}

/// Extract an output without computing any expected mathematical answer.
pub(crate) fn output(result: &SearchResult) -> TestResult<Handle> {
    Ok(match statement(result)? {
        DistributionStatement::Embedding { embedding, .. } => embedding,
        DistributionStatement::Derivative { derivative, .. } => derivative,
        DistributionStatement::Equality { right, .. } => right,
        DistributionStatement::Pairing { value, .. } => value,
        _ => return Err("value-producing distribution theorem".into()),
    }
    .clone())
}

/// Every accepted flow is checked again in an empty store with its complete expected theorem.
pub(crate) fn replay(fixture: &Fixture, result: &SearchResult) -> TestResult<Vec<u8>> {
    let bytes = fixture
        .base
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let imported = receiver.inspect(&bytes, &mut budget()?)?;
    assert_eq!(imported.judgment().layer, Layer::Distribution);
    assert!(imported.judgment().space.is_some());
    let expected = imported.judgment().clone();
    receiver.replay(&bytes, &expected, &mut budget()?)?;
    assert_eq!(receiver.export_witness(&imported, &mut budget()?)?, bytes);
    Ok(bytes)
}
