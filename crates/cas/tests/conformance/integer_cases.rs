// SPDX-License-Identifier: AGPL-3.0-only
//! Native Integer results have exact declared sorts and no scalar placement.

use super::decision_baseline::{Example, Fixture};
use crate::testing::builders::{admit, expression, expression_draft};
use crate::testing::common::{TestResult, real};
use crate::testing::readings::ledger;
use nrz_cas::domain::{CaseArmDraft, CasesDraft, RegionDraft, RegionExpr};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{AtomicSort, Draft, Operator, Sort};
use nrz_cas::{FaultKind, Handle, InvalidCode, Rational, Reference};

const fn integer() -> Sort {
    Sort::Value(AtomicSort::Integer)
}

fn literal(source: &mut Fixture, sort: Sort, n: i64, d: u64) -> TestResult<Handle> {
    expression(
        &mut source.cas,
        &source.context,
        Operator::Rational(Rational::from_ratio(n, d, &mut ledger()?)?),
        sort,
        &[],
        &mut ledger()?,
    )
}

fn drafts(source: &Fixture, sort: Sort, values: &[Handle]) -> TestResult<Vec<Draft>> {
    let mut result = Vec::new();
    for value in values {
        result.push(Draft::CaseArm(CaseArmDraft {
            header: Reference::Existing(source.header.clone()),
            scope: Vec::new(),
            placements: Vec::new(),
            region: Reference::Existing(source.ambient.clone()),
            value: Reference::Existing(value.clone()),
        }));
    }
    result.push(Draft::Cases(CasesDraft {
        sort,
        header: Reference::Existing(source.header.clone()),
        scope: Vec::new(),
        domain: Reference::Existing(source.ambient.clone()),
        arms: (0..values.len())
            .map(|index| u32::try_from(index).map(Reference::Local))
            .collect::<Result<Vec<_>, _>>()?,
    }));
    Ok(result)
}

/// Every native arm must agree with the explicit case sort, in either order.
/// ´test:integration:integer-case-declared-results´
#[test]
fn integer_case_declared_results() -> TestResult {
    let mut source = Fixture::new(Example::Integer)?;
    let count = literal(&mut source, integer(), 3, 1)?;
    let scalar = literal(&mut source, real(), 3, 1)?;
    for (sort, value) in [(integer(), &count), (real(), &scalar)] {
        let batch = drafts(&source, sort.clone(), std::slice::from_ref(value))?;
        let admitted = source.cas.admit(&batch, &mut ledger()?)?;
        let function = admitted.last().ok_or("missing case")?;
        assert_eq!(source.cas.expression(function)?.1, &sort);
    }
    for sort in [integer(), real()] {
        for values in [
            vec![count.clone(), scalar.clone()],
            vec![scalar.clone(), count.clone()],
        ] {
            let batch = drafts(&source, sort.clone(), &values)?;
            let before = source.cas.counts();
            let mut budget = ledger()?;
            let fault = source
                .cas
                .admit(&batch, &mut budget)
                .err()
                .ok_or("mixed case accepted")?;
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::SymbolSortMismatch)
            );
            assert_eq!(source.cas.counts(), before);
            assert!(budget.usage().work > 0);
        }
    }
    for (sort, value) in [(integer(), scalar), (real(), count)] {
        let batch = drafts(&source, sort, &[value])?;
        let before = source.cas.counts();
        let fault = source
            .cas
            .admit(&batch, &mut ledger()?)
            .err()
            .ok_or("wrong result accepted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::SymbolSortMismatch)
        );
        assert_eq!(source.cas.counts(), before);
    }
    Ok(())
}

/// Integer literals are exact integral values and do not acquire a registered scalar ambient.
/// ´test:integration:integer-case-literal-contract´
#[test]
fn integer_case_literal_contract() -> TestResult {
    let mut source = Fixture::new(Example::Integer)?;
    let count = literal(&mut source, integer(), 3, 1)?;
    assert_eq!(source.cas.scalar_ambient(&count)?, None);
    let before = source.cas.counts();
    assert!(literal(&mut source, integer(), 3, 2).is_err());
    assert_eq!(source.cas.counts(), before);
    let batch = drafts(&source, integer(), &[count])?;
    let admitted = source.cas.admit(&batch, &mut ledger()?)?;
    assert_eq!(
        source.cas.scalar_ambient(admitted.last().ok_or("case")?)?,
        None
    );
    Ok(())
}

fn bound_case(source: &mut Fixture) -> TestResult<Handle> {
    let scope = vec![vec![AtomicSort::Integer]];
    let value = admit(
        &mut source.cas,
        expression_draft(
            Operator::Bound { depth: 0, slot: 0 },
            integer(),
            Reference::Existing(source.header.clone()),
            scope.clone(),
            None,
            &[],
        ),
        &mut ledger()?,
    )?;
    let region = admit(
        &mut source.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(source.header.clone()),
            scope: scope.clone(),
            ambient: source.kind.clone(),
            expression: RegionExpr::Ambient,
        }),
        &mut ledger()?,
    )?;
    let batch = [
        Draft::CaseArm(CaseArmDraft {
            header: Reference::Existing(source.header.clone()),
            scope: scope.clone(),
            placements: Vec::new(),
            region: Reference::Existing(region.clone()),
            value: Reference::Existing(value),
        }),
        Draft::Cases(CasesDraft {
            sort: integer(),
            header: Reference::Existing(source.header.clone()),
            scope,
            domain: Reference::Existing(region),
            arms: vec![Reference::Local(0)],
        }),
    ];
    let admitted = source.cas.admit(&batch, &mut ledger()?)?;
    admitted.last().cloned().ok_or_else(|| "case".into())
}

/// Instantiation preserves explicit Integer case results and refuses a Real slot replacement.
/// ´test:integration:integer-case-bound-result´
#[test]
fn integer_case_bound_result() -> TestResult {
    let mut source = Fixture::new(Example::Integer)?;
    let function = bound_case(&mut source)?;
    assert_eq!(source.cas.expression(&function)?.1, &integer());
    assert_eq!(
        source.cas.expression_scope(&function)?,
        &vec![vec![AtomicSort::Integer]]
    );
    let three = literal(&mut source, integer(), 3, 1)?;
    let instantiated = source
        .cas
        .instantiate_bound(&function, &[three], &mut ledger()?)?;
    assert_eq!(source.cas.expression(&instantiated)?.1, &integer());
    assert_eq!(
        source.cas.expression_scope(&instantiated)?.as_slice(),
        [] as [Vec<AtomicSort>; 0]
    );
    let scalar = literal(&mut source, real(), 3, 1)?;
    let before = source.cas.counts();
    assert!(
        source
            .cas
            .instantiate_bound(&function, &[scalar], &mut ledger()?)
            .is_err()
    );
    assert_eq!(source.cas.counts(), before);
    Ok(())
}

/// Native Integer admission has literal cumulative Work and storage ceilings.
/// ´test:integration:integer-case-admission-boundaries´
#[test]
fn integer_case_admission_boundaries() -> TestResult {
    let mut source = Fixture::new(Example::Integer)?;
    let count = literal(&mut source, integer(), 3, 1)?;
    let batch = drafts(&source, integer(), &[count])?;
    let before = source.cas.counts();
    let mut budget = ledger()?;
    source.cas.admit(&batch, &mut budget)?;
    assert_eq!(budget.usage().work, 1_522);
    assert_eq!(
        source.cas.counts().payload_bytes - before.payload_bytes,
        965
    );
    for (kind, amount) in [
        (BudgetKind::Work, 1_522),
        (BudgetKind::StagedPayloadBytes, 4_368),
        (BudgetKind::PayloadBytes, 3_311),
    ] {
        assert!(admission_attempt(kind, amount, 0)?);
        assert!(!admission_attempt(kind, amount - 1, 0)?);
    }
    assert!(admission_attempt(BudgetKind::Work, 1_522 + 37, 37)?);
    assert!(!admission_attempt(BudgetKind::Work, 1_522 + 36, 37)?);
    Ok(())
}

fn admission_attempt(kind: BudgetKind, amount: u64, prior: u64) -> TestResult<bool> {
    let mut source = Fixture::new(Example::Integer)?;
    let count = literal(&mut source, integer(), 3, 1)?;
    let batch = drafts(&source, integer(), &[count])?;
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
        BudgetKind::PayloadBytes => limits.payload_bytes = amount,
        _ => return Err("admission resource".into()),
    }
    let mut budget = Budget::new(limits)?;
    budget.work(
        prior,
        nrz_cas::Location::new("prior integer case admission"),
    )?;
    let before = source.cas.counts();
    match source.cas.admit(&batch, &mut budget) {
        Ok(_) => Ok(true),
        Err(fault) => {
            assert_eq!(source.cas.counts(), before);
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
            Ok(false)
        }
    }
}

/// A caller-owned named result cannot bypass the bounded draft payload measurement.
/// ´test:integration:integer-case-result-name-budget´
#[test]
fn integer_case_result_name_budget() -> TestResult {
    let mut source = Fixture::new(Example::Integer)?;
    let count = literal(&mut source, integer(), 3, 1)?;
    let sort = Sort::Value(AtomicSort::Scalar(nrz_cas::syntax::Field::Named(
        "n".repeat(257),
    )));
    let batch = drafts(&source, sort, &[count])?;
    let before = source.cas.counts();
    let fault = source
        .cas
        .admit(&batch, &mut ledger()?)
        .err()
        .ok_or("unmetered sort name")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::NameBytes,
            ..
        }
    ));
    assert_eq!(source.cas.counts(), before);
    Ok(())
}
