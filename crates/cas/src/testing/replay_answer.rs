// SPDX-License-Identifier: AGPL-3.0-only
//! Syntax-only proposals, independent expected claims and per-application replay ledgers.

use std::cell::RefCell;

use crate::budget::{Budget, BudgetKind, BudgetUsage, Limits};
use crate::domain_model::Layer;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::proof_model::{Judgment, Statement};
use crate::rule::Primitive;
use crate::solving::{SolveForm, SolveStatement, StatementCoverage, StatementPresentation};
use crate::testing::common::TestResult;
use crate::wire_decode::Decoded;
use crate::wire_read::Reader;

/// Inclusive primitive-check deltas; the surrounding decoder and theorem comparisons remain charged.
#[derive(Clone, Debug)]
pub(crate) struct Obligation {
    pub(crate) rule: Primitive,
    pub(crate) before: BudgetUsage,
    pub(crate) after: BudgetUsage,
}

thread_local! {
    static LEDGERS: RefCell<Option<Vec<Obligation>>> = const { RefCell::new(None) };
}

/// Observation changes no mathematical state or budget counter.
pub(crate) fn record(
    rule: Primitive,
    before: &BudgetUsage,
    after: &BudgetUsage,
) -> Result<(), Fault> {
    LEDGERS.with(|cell| {
        let mut active = cell.try_borrow_mut().map_err(|_| observer_fault())?;
        if let Some(rows) = active.as_mut() {
            rows.try_reserve(1).map_err(|_| observer_fault())?;
            rows.push(Obligation {
                rule,
                before: *before,
                after: *after,
            });
        }
        Ok(())
    })
}

const fn observer_fault() -> Fault {
    Fault::invalid(InvalidCode::Rule, Location::new("test replay observer"))
}

/// Always disarm observation, including after an intentionally refused replay.
pub(crate) fn measured<T>(run: impl FnOnce() -> TestResult<T>) -> TestResult<(T, Vec<Obligation>)> {
    LEDGERS.with(|cell| -> TestResult {
        let mut active = cell.try_borrow_mut()?;
        assert!(active.is_none());
        *active = Some(Vec::new());
        Ok(())
    })?;
    let result = run();
    let rows = LEDGERS.with(|cell| -> TestResult<Vec<Obligation>> {
        cell.try_borrow_mut()?
            .take()
            .ok_or_else(|| "observer inactive".into())
    })?;
    Ok((result?, rows))
}

/// Admit only the proposed set's object DAG; there are no application or conclusion records.
pub(crate) fn proposal(source: &Cas, set: &Handle, receiver: &mut Cas) -> TestResult<Handle> {
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let bytes = crate::wire_build::syntax_key(source, source.checked(set)?, &mut budget)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut reader = Reader::new(&bytes, &mut budget)?;
    assert_eq!(reader.take(8)?, b"NRZSYN04");
    assert_eq!(reader.index()?, 0);
    let count = reader.count(BudgetKind::Vertices, 11, 512)?;
    let mut records = Vec::new();
    for _ in 0..count {
        records.push(syntax_record(receiver, &mut reader, &records)?);
    }
    let root = usize::try_from(reader.index()?)?;
    reader.finished()?;
    let Some(Decoded::Object(handle)) = records.get(root) else {
        return Err("proposal root is not syntax".into());
    };
    assert_eq!(receiver.state.proof_count(), 0);
    Ok(handle.clone())
}

fn syntax_record(
    cas: &mut Cas,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> TestResult<Decoded> {
    let tag = reader.byte()?;
    assert!(matches!(tag, 0..=8 | 13..=36));
    let length = reader.count(BudgetKind::WitnessBytes, 1, 2)?;
    let payload = reader.take(length)?;
    let mut part = Reader::new(payload, reader.budget)?;
    assert_eq!(part.index()?, 1);
    let layer = match part.byte()? {
        0 => Layer::Logic,
        1 => Layer::Pointwise,
        2 => Layer::Distribution,
        _ => return Err("proposal layer".into()),
    };
    let space = crate::wire_decode::optional(&mut part, records)?;
    let object = crate::wire_objects::decode(cas, tag, layer, space.as_ref(), &mut part, records)?;
    part.finished()?;
    Ok(Decoded::Object(object))
}

/// Source, context, region and requested form come from the independently rebuilt client request.
pub(crate) fn expected(
    fixture: &crate::testing::solve_blind::Fixture,
    problem: &Handle,
    set: &Handle,
    form: SolveForm,
) -> (Judgment, Judgment) {
    let coverage = Judgment {
        context: fixture.context.clone(),
        layer: Layer::Logic,
        space: None,
        residuals: Vec::new(),
        statement: Statement::Solve(Box::new(SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: fixture.region.clone(),
        }))),
    };
    let presentation = Judgment {
        statement: Statement::Solve(Box::new(SolveStatement::Presentation(
            StatementPresentation {
                set: set.clone(),
                form,
                region: fixture.region.clone(),
            },
        ))),
        ..coverage.clone()
    };
    (coverage, presentation)
}

/// The original replay oracle independently rebuilds both theorem keys and re-encodes the checked proof.
pub(crate) fn full_replay(
    cas: &mut Cas,
    bytes: &[u8],
    expected: &Judgment,
    budget: &mut Budget,
) -> Result<crate::proof_model::VerifiedWitness, Fault> {
    crate::wire_decode::magic(bytes)?;
    cas.proof_limits(budget, |cas, budget| {
        crate::proof_check::replay_budget(budget, |budget| {
            crate::proof_view::validate_judgment(cas, expected, budget)?;
            let expected = crate::wire_build::judgment_key(cas, expected, budget)?;
            cas.transaction(budget, |candidate, budget| {
                let witness = crate::wire_decode::inspect(candidate, bytes, budget)?;
                let canonical =
                    crate::wire::encode(candidate, candidate.checked(witness.proof())?, budget)?;
                if canonical != bytes {
                    return Err(Fault::invalid(
                        crate::error::InvalidCode::Encoding,
                        crate::Location::new("full replay oracle"),
                    ));
                }
                let actual =
                    crate::wire_build::judgment_key(candidate, witness.judgment(), budget)?;
                if actual != expected {
                    return Err(Fault::invalid(
                        crate::error::InvalidCode::EndpointMismatch,
                        crate::Location::new("typed proof admission"),
                    ));
                }
                Ok(witness)
            })
        })
    })
}
