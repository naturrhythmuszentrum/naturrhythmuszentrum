// SPDX-License-Identifier: AGPL-3.0-only
//! Original spectral evidence, physical input bindings and independent numerical oracles.

#[path = "declarations.rs"]
mod declarations;
#[path = "exact.rs"]
mod exact;
#[path = "inputs.rs"]
mod inputs;
#[path = "source/mod.rs"]
mod source;

use nrz_bindings::{
    Budget, Limits,
    readings::{
        CoordinateNormalization, LcSpectralLaw, RootCoverage, RootProblem,
        algebra::{Cas, Handle, request::Outcome},
    },
};
use nrz_matrix::{Heap, HeapLimits, LcSpectrum, ReadingBinding};
use nrz_scalar::{LiteralLimits, Scalar};
use std::rc::Rc;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub(crate) fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(Limits::default())?)
}

/// Prepared graph and both original source readings, retained independently of heap contents.
pub(crate) struct Fixture {
    pub(crate) graph: Rc<Cas>,
    pub(crate) law: LcSpectralLaw,
    pub(crate) input_construction_work: u64,
    pub(crate) coverage: nrz_matrix::ReadingCoverage<usize>,
    pub(crate) companions: Vec<nrz_matrix::CompanionAdapter<usize>>,
}

impl Fixture {
    /// All exact evidence and complete input declarations precede numerical input access.
    pub(crate) fn new(references: [&str; 3]) -> TestResult<Self> {
        let (mut source, request) = source::original([1, 1], &mut source::ledger()?)?;
        let parameters = [
            source.real.clone(),
            source::expression(
                &mut source,
                nrz_bindings::readings::algebra::syntax::Field::Real,
                nrz_bindings::readings::algebra::syntax::Operator::Free("C".into()),
                &[],
                &mut ledger()?,
            )?,
        ];
        let Outcome::Solved { answer, .. } = source.cas.run(&request) else {
            return Err("spectral roots".into());
        };
        let witness = answer.coverage.ok_or("spectral coverage")?;
        let bytes = source.cas.export_witness(&witness, &mut ledger()?)?;
        let problem = RootProblem::check(&mut source.cas, request, &mut ledger()?)?;
        let coverage = RootCoverage::replay(
            &mut source.cas,
            &problem,
            answer.set,
            &bytes,
            &mut ledger()?,
        )?;
        let mut construction = ledger()?;
        let context = source::real::context(&mut source, &mut construction)?;
        let state = [
            exact::meaning(
                "voltage",
                [2, 1, -3, -1, 0, 0, 0],
                references[0],
                &mut construction,
            )?,
            exact::meaning(
                "current",
                [0, 0, 0, 1, 0, 0, 0],
                references[1],
                &mut construction,
            )?,
        ];
        let state =
            CoordinateNormalization::new(&mut source.cas, &context, &state, &mut construction)
                .map_err(|error| format!("state reference inversion: {error:?}"))?;
        let time = exact::meaning(
            "duration",
            [0, 0, 1, 0, 0, 0, 0],
            references[2],
            &mut construction,
        )?;
        let recipe = exact::recipe(&mut source, &context, &state, &time, &mut construction)?;
        let law = LcSpectralLaw::check(
            &mut source.cas,
            coverage,
            parameters,
            recipe,
            state,
            time,
            &mut ledger()?,
        )
        .map_err(|error| format!("closed spectral law: {error:?}"))?;
        let mut declarations = ledger()?;
        let (coverage, companions) =
            inputs::declarations(&mut source, &context, &law, &mut declarations)?;
        Ok(Self {
            graph: Rc::new(source.cas),
            law,
            input_construction_work: declarations.usage().work,
            coverage,
            companions,
        })
    }
    /// Original input parameters are actual companion regions, not fixture tolerances.
    pub(crate) fn import<P: Scalar + 'static>(
        &self,
        values: [[P; 3]; 2],
    ) -> TestResult<(Heap, [ReadingBinding<P>; 2])> {
        self.import_with_budget(values, &mut ledger()?)
    }
    /// Storage admission uses the same explicit caller ledger as its surrounding transaction.
    pub(crate) fn import_with_budget<P: Scalar + 'static>(
        &self,
        values: [[P; 3]; 2],
        budget: &mut Budget,
    ) -> TestResult<(Heap, [ReadingBinding<P>; 2])> {
        let mut heap = Heap::new(HeapLimits::default())?;
        let bindings = inputs::import(&mut heap, self, values, budget)?;
        Ok((heap, bindings))
    }
    pub(crate) fn plan<P: Scalar>(
        &self,
        bindings: [ReadingBinding<P>; 2],
        budget: &mut Budget,
    ) -> TestResult<LcSpectrum<P>> {
        Ok(LcSpectrum::new(
            &self.graph,
            self.law.clone(),
            bindings,
            LiteralLimits::default(),
            budget,
        )?)
    }
}

/// Matrix-valued storage and scalar exact inputs retain the same original declaration frame.
fn symbol(
    source: &mut source::Fixture,
    key: &str,
    matrix: bool,
    budget: &mut Budget,
) -> TestResult<Handle> {
    use nrz_bindings::readings::algebra::{
        Reference,
        syntax::{AtomicSort, Draft, ExpressionDraft, Extent, Field, Operator, Sort},
    };
    let sort = if matrix {
        Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(1),
            columns: Extent::Literal(1),
        })
    } else {
        Sort::Value(AtomicSort::Scalar(Field::Real))
    };
    Ok(source
        .cas
        .admit(
            &[Draft::Expression(ExpressionDraft {
                operator: Operator::Free(key.into()),
                sort,
                header: Reference::Existing(source.header.clone()),
                scope: Vec::new(),
                binding: None,
                space: None,
                operands: Vec::new(),
            })],
            budget,
        )?
        .pop()
        .ok_or("spectral symbol")?)
}
