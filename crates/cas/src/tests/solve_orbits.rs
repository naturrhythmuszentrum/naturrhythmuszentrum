// SPDX-License-Identifier: AGPL-3.0-only
//! Generic producer interactions, independent integer oracles and complete proof lifecycles.

use crate::limits::{Budget, BudgetKind, Limits};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::Variable;
use crate::solving::coefficients::model::{Element, Value};
use crate::solving::coefficients::scalar::Scalar;
use crate::solving::orbits::produce::{Orbit, enumerate};
use crate::solving::symmetric::algebra::Polynomial;
use crate::solving::symmetric::cursor::{Cursor, Seed};
use crate::solving::symmetric::reduction::{Reduction, reduce};
use crate::solving::symmetric::resolvent::{Resolvent, smaller};
use crate::solving::symmetric::seeds::generate;
use crate::testing::common::TestResult;
use crate::testing::solve_orbits::Fixture;
use crate::{Cas, FaultKind};
use std::collections::BTreeMap;

/// Small exact integer pairs in the independently interpreted cubic unity algebra.
type Pair = (i64, i64);
/// Independent sparse polynomials use dense exponent keys, separate from production forms.
type Oracle = BTreeMap<Vec<u32>, Pair>;

/// The test oracle uses integer distributivity modulo t²+t+1.
const fn pair_product(left: Pair, right: Pair) -> Pair {
    (
        left.0 * right.0 - left.1 * right.1,
        left.0 * right.1 + left.1 * right.0 - left.1 * right.1,
    )
}

/// Decode an exact integer coefficient for comparison, never to propose a production identity.
fn integer(value: &Element) -> TestResult<i64> {
    let Value::Scalar(Scalar::Rational(value)) = &value.value else {
        return Err("expected rational component".into());
    };
    let (numerator, denominator) = value
        .small_ratio(&mut Budget::new(Limits::blind_cubic())?)?
        .ok_or("large oracle integer")?;
    if denominator != 1 {
        return Err("noninteger oracle coefficient".into());
    }
    Ok(numerator)
}

/// Scratch results are decoded only after the producer has returned.
fn oracle(value: &Polynomial) -> TestResult<Oracle> {
    let mut result = Oracle::new();
    for term in &value.form.terms {
        let mut key = vec![0; value.variables.len()];
        for (variable, exponent) in &term.powers {
            let Variable::Slot(slot) = variable else {
                return Err("unexpected scalar atom".into());
            };
            *key.get_mut(usize::try_from(*slot)?)
                .ok_or("bad oracle slot")? = *exponent;
        }
        let pair = match &term.coefficient.value {
            Value::Tower(entries) if entries.len() <= 2 => (
                entries.first().map(integer).transpose()?.unwrap_or(0),
                entries.get(1).map(integer).transpose()?.unwrap_or(0),
            ),
            Value::Scalar(_) => (integer(&term.coefficient)?, 0),
            _ => return Err("unsupported oracle coefficient".into()),
        };
        result.insert(key, pair);
    }
    Ok(result)
}

/// Distribute every independent term pair, combining equal dense keys with ordinary integer arithmetic.
fn multiply(left: &Oracle, right: &Oracle) -> Oracle {
    let mut result = Oracle::new();
    for (first, a) in left {
        for (second, b) in right {
            let powers = first.iter().zip(second).map(|(x, y)| x + y).collect();
            let product = pair_product(*a, *b);
            let entry = result.entry(powers).or_insert((0, 0));
            entry.0 += product.0;
            entry.1 += product.1;
        }
    }
    result.retain(|_, coefficient| *coefficient != (0, 0));
    result
}

/// Independent addition also supports the sign needed by factor multiplication.
fn sum(left: &Oracle, right: &Oracle, sign: i64) -> Oracle {
    let mut result = left.clone();
    for (powers, coefficient) in right {
        let entry = result.entry(powers.clone()).or_insert((0, 0));
        entry.0 += sign * coefficient.0;
        entry.1 += sign * coefficient.1;
    }
    result.retain(|_, coefficient| *coefficient != (0, 0));
    result
}

/// Elementary functions are independently enumerated by subsets, without the production recurrence.
fn elementary_oracle(width: usize) -> Vec<Oracle> {
    let mut result = vec![Oracle::new(); width];
    for mask in 1_usize..(1 << width) {
        let powers: Vec<u32> = (0..width)
            .map(|slot| u32::from(mask & (1 << slot) != 0))
            .collect();
        let degree = powers.iter().filter(|power| **power == 1).count();
        result[degree - 1].insert(powers, (1, 0));
    }
    result
}

/// Expand the returned elementary expression using independent subset polynomials and multiplication.
fn expansion_oracle(expression: &Oracle, width: usize) -> Oracle {
    let elementary = elementary_oracle(width);
    let mut result = Oracle::new();
    for (powers, coefficient) in expression {
        let mut term = Oracle::from([(vec![0; width], *coefficient)]);
        for (slot, power) in powers.iter().enumerate() {
            for _ in 0..*power {
                term = multiply(&term, &elementary[slot]);
            }
        }
        result = sum(&result, &term, 1);
    }
    result
}

/// Multiply every supplied orbit factor independently, retaining each original factor.
fn factor_oracle(orbit: &Orbit) -> TestResult<Vec<Oracle>> {
    let width = orbit
        .members
        .first()
        .ok_or("missing orbit")?
        .variables
        .len();
    let mut coefficients = vec![Oracle::from([(vec![0; width], (1, 0))])];
    for member in &orbit.members {
        let value = oracle(member)?;
        let mut next = vec![Oracle::new(); coefficients.len() + 1];
        for (index, coefficient) in coefficients.iter().enumerate() {
            next[index] = sum(&next[index], &multiply(coefficient, &value), -1);
            next[index + 1] = sum(&next[index + 1], coefficient, 1);
        }
        coefficients = next;
    }
    Ok(coefficients)
}

/// Candidate order is deterministic, lazy, and governed by n/gcd(n,k), including nonprime degrees.
/// ´test:crate:symmetric-candidate-cursor´
#[test]
fn symmetric_candidate_cursor() -> TestResult {
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut cursor = Cursor::new(3, &mut budget)?;
    let mut repeat = Cursor::new(3, &mut budget)?;
    for index in 1..3 {
        for power in 1..=3 {
            let next = cursor.next(&mut budget)?;
            assert_eq!(next, Some(Seed::Cyclic { index, power }));
            assert_eq!(repeat.next(&mut budget)?, next);
        }
    }
    for partition in [
        vec![0, 0, 0],
        vec![0, 0, 1],
        vec![0, 1, 0],
        vec![0, 1, 1],
        vec![0, 1, 2],
    ] {
        let next = cursor.next(&mut budget)?;
        assert_eq!(next, Some(Seed::Partition(partition)));
        assert_eq!(repeat.next(&mut budget)?, next);
    }
    assert_eq!(cursor.next(&mut budget)?, None);
    let mut composite = Cursor::new(4, &mut budget)?;
    let mut powers = Vec::new();
    while let Some(Seed::Cyclic { index, power }) = composite.next(&mut budget)? {
        if index == 2 {
            powers.push(power);
        }
    }
    assert_eq!(powers, [1, 2]);
    assert!(budget.usage().gcd_steps > 0);
    Ok(())
}

/// Build one generated orbit with the requested unity orientation under a single shared ledger.
fn cyclic_orbit(
    fixture: &Fixture,
    orientation: u32,
    power: u32,
    budget: &mut Budget,
) -> TestResult<Orbit> {
    let engine = Engine {
        state: &fixture.base.cas.state,
        domain: &fixture.frame.domain,
    };
    let unity = engine.pow(&fixture.unity, orientation, budget)?;
    let seed = generate(
        &fixture.frame,
        &Seed::Cyclic { index: 1, power },
        Some(&unity),
        &fixture.base.cas.state,
        budget,
    )?;
    Ok(enumerate(&seed, &fixture.base.cas.state, budget)?)
}

/// Equal/larger orbits never recurse, and all generated character orbits independently replay.
/// ´test:crate:generated-orbits-and-degree-descent´
#[test]
fn generated_orbits_and_degree_descent() -> TestResult {
    let mut fixture = Fixture::new(3)?;
    for orientation in [1, 2] {
        for (power, count) in [(1, 6), (2, 6), (3, 2)] {
            let mut budget = Budget::new(Limits::blind_cubic())?;
            let orbit = cyclic_orbit(&fixture, orientation, power, &mut budget)?;
            assert_eq!(orbit.members.len(), count);
            assert!(budget.usage().coefficient_ops > 0);
            let before = budget.usage();
            let target = smaller(&orbit, 3, 0, &fixture.base.cas.state, &mut budget)?;
            if count >= 3 {
                assert!(target.is_none());
                assert_eq!(budget.usage().coefficient_ops, before.coefficient_ops);
                assert_eq!(budget.usage().symmetric_steps, 0);
                assert_eq!(budget.usage().resolvent_depth, 0);
            } else {
                assert_eq!(
                    target.ok_or("smaller target missing")?.coefficients.len(),
                    3
                );
            }
            let step =
                orbit.application(&mut fixture.base.cas, &fixture.base.context, &mut budget)?;
            let witness = fixture.base.cas.check_proof(
                &fixture.base.context,
                std::slice::from_ref(&step),
                &mut Budget::new(Limits::blind_cubic())?,
            )?;
            empty_replay(&fixture.base.cas, &witness)?;
        }
    }
    Ok(())
}

/// Fresh inspection has no preloaded objects or lemmas; canonical re-export verifies the whole proof closure.
fn empty_replay(source: &Cas, witness: &crate::proof::VerifiedWitness) -> TestResult {
    let bytes = source.export_witness(witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut empty = Cas::new(Limits::blind_cubic())?;
    let imported = empty.inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    assert_eq!(imported.hypotheses(), []);
    assert_eq!(imported.judgment().residuals, []);
    let replayed = empty.replay(
        &bytes,
        imported.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(replayed.judgment(), imported.judgment());
    assert_eq!(
        empty.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// The supplied depressed coefficient substitution is introduced only after the generic producer returns.
fn depressed_images(fixture: &Fixture) -> TestResult<Vec<Polynomial>> {
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let state = &fixture.base.cas.state;
    Ok(vec![
        fixture.frame.identity(false, state, &mut budget)?,
        fixture.frame.variable(0, state, &mut budget)?,
        fixture
            .frame
            .variable(1, state, &mut budget)?
            .neg(state, &mut budget)?,
    ])
}

/// Each returned coefficient expands to the independent orbit product and has fresh symmetry/substitution replay.
fn certify_coefficients(fixture: &mut Fixture, orbit: &Orbit, resolvent: &Resolvent) -> TestResult {
    let product = factor_oracle(orbit)?;
    let images = depressed_images(fixture)?;
    let expected = [
        Oracle::from([(vec![3, 0, 0], (-27, 0))]),
        Oracle::from([(vec![0, 1, 0], (27, 0))]),
        Oracle::from([(vec![0, 0, 0], (1, 0))]),
    ];
    for ((reduction, independent), substituted) in
        resolvent.coefficients.iter().zip(&product).zip(&expected)
    {
        assert_eq!(oracle(&reduction.source)?, *independent);
        assert_eq!(
            expansion_oracle(&oracle(&reduction.expression)?, 3),
            *independent
        );
        let certified = fixture.certify(reduction, &images)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let actual = Polynomial::read(&fixture.base.cas, &certified.substituted, &mut budget)?;
        let expression = Polynomial::read(&fixture.base.cas, &certified.expression, &mut budget)?;
        assert_eq!(oracle(&actual)?, *substituted);
        assert_eq!(oracle(&expression)?, oracle(&reduction.expression)?);
        empty_replay(&fixture.base.cas, &certified.witness)?;
    }
    Ok(())
}

/// Both orientations yield the same supplied lower-degree target with every finite proof independently replayed.
/// ´test:crate:symmetric-resolvent-empty-store-lifecycle´
#[test]
fn symmetric_resolvent_empty_store_lifecycle() -> TestResult {
    for orientation in [1, 2] {
        let mut fixture = Fixture::new(3)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let orbit = cyclic_orbit(&fixture, orientation, 3, &mut budget)?;
        let resolvent = smaller(&orbit, 3, 0, &fixture.base.cas.state, &mut budget)?
            .ok_or("smaller resolvent missing")?;
        assert_eq!(resolvent.depth, 1);
        certify_coefficients(&mut fixture, &orbit, &resolvent)?;
    }
    Ok(())
}

/// Cross-character pairing is reduced from a generated product, not loaded as an invariant recipe.
/// ´test:crate:symmetric-pairing-independent-expansion´
#[test]
fn symmetric_pairing_independent_expansion() -> TestResult {
    let mut fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let left = generate(
        &fixture.frame,
        &Seed::Cyclic { index: 1, power: 1 },
        Some(&fixture.unity),
        &fixture.base.cas.state,
        &mut budget,
    )?;
    let right = generate(
        &fixture.frame,
        &Seed::Cyclic { index: 2, power: 1 },
        Some(&fixture.unity),
        &fixture.base.cas.state,
        &mut budget,
    )?;
    let product = left.mul(&right, &fixture.base.cas.state, &mut budget)?;
    let reduction = reduce(&product, &fixture.base.cas.state, &mut budget)?;
    let independent = multiply(&oracle(&left)?, &oracle(&right)?);
    let expected = Oracle::from([(vec![2, 0, 0], (1, 0)), (vec![0, 1, 0], (-3, 0))]);
    assert_eq!(oracle(&product)?, independent);
    assert_eq!(oracle(&reduction.expression)?, expected);
    assert_eq!(expansion_oracle(&expected, 3), independent);
    let images = depressed_images(&fixture)?;
    let certified = fixture.certify(&reduction, &images)?;
    empty_replay(&fixture.base.cas, &certified.witness)?;
    Ok(())
}

/// A partition of four roots generates three pairings and a cubic resolvent by the same generic machinery.
/// ´test:crate:partition-resolvent-independent-expansion´
#[test]
fn partition_resolvent_independent_expansion() -> TestResult {
    let fixture = Fixture::new(4)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut cursor = Cursor::new(4, &mut budget)?;
    let sought = Seed::Partition(vec![0, 0, 1, 1]);
    loop {
        if cursor.next(&mut budget)?.ok_or("partition missing")? == sought {
            break;
        }
    }
    let seed = generate(
        &fixture.frame,
        &sought,
        None,
        &fixture.base.cas.state,
        &mut budget,
    )?;
    let orbit = enumerate(&seed, &fixture.base.cas.state, &mut budget)?;
    assert_eq!(orbit.members.len(), 3);
    let resolvent = smaller(&orbit, 4, 0, &fixture.base.cas.state, &mut budget)?
        .ok_or("quartic target missing")?;
    assert_eq!(resolvent.coefficients.len(), 4);
    let expected = factor_oracle(&orbit)?;
    for (coefficient, independent) in resolvent.coefficients.iter().zip(&expected) {
        assert_eq!(oracle(&coefficient.source)?, *independent);
        assert_eq!(
            expansion_oracle(&oracle(&coefficient.expression)?, 4),
            *independent
        );
    }
    Ok(())
}

/// Finite symmetric work succeeds at measured N and refuses at N-1; nonsymmetric inputs never get a reduction.
/// ´test:crate:symmetric-step-boundary-and-refusal´
#[test]
fn symmetric_step_boundary_and_refusal() -> TestResult {
    let fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let first = fixture
        .frame
        .variable(0, &fixture.base.cas.state, &mut budget)?;
    assert!(reduce(&first, &fixture.base.cas.state, &mut budget).is_err());
    let orbit = cyclic_orbit(&fixture, 1, 3, &mut budget)?;
    let resolvent =
        smaller(&orbit, 3, 0, &fixture.base.cas.state, &mut budget)?.ok_or("target missing")?;
    let source = &resolvent
        .coefficients
        .first()
        .ok_or("constant missing")?
        .source;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    reduce(source, &fixture.base.cas.state, &mut measured)?;
    let count = measured.usage().symmetric_steps;
    assert!(count > 0);
    for (limit, succeeds) in [(count, true), (count - 1, false)] {
        let mut bounded = Budget::new(Limits {
            symmetric_steps: limit,
            ..Limits::blind_cubic()
        })?;
        let result = reduce(source, &fixture.base.cas.state, &mut bounded);
        if succeeds {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result.err().ok_or("short symmetric budget accepted")?.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::SymmetricSteps,
                    ..
                }
            ));
        }
    }
    Ok(())
}

/// Missing map existence and false reductions fail the independent checker without publishing staged syntax.
/// ´test:crate:symmetric-proof-atomic-refusal´
#[test]
fn symmetric_proof_atomic_refusal() -> TestResult {
    let mut fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let orbit = cyclic_orbit(&fixture, 1, 3, &mut budget)?;
    let resolvent =
        smaller(&orbit, 3, 0, &fixture.base.cas.state, &mut budget)?.ok_or("target missing")?;
    let reduction = resolvent.coefficients.first().ok_or("constant missing")?;
    let changed = Reduction {
        source: reduction
            .source
            .copy(&fixture.base.cas.state, &mut budget)?,
        expression: reduction
            .expression
            .identity(false, &fixture.base.cas.state, &mut budget)?,
        basis: reduction
            .basis
            .iter()
            .map(|value| value.copy(&fixture.base.cas.state, &mut budget))
            .collect::<Result<_, _>>()?,
    };
    let images = depressed_images(&fixture)?;
    let before = fixture.base.cas.counts();
    assert!(fixture.certify(&changed, &images).is_err());
    assert_eq!(fixture.base.cas.counts(), before);
    fixture.without_existence();
    assert!(fixture.certify(reduction, &images).is_err());
    assert_eq!(fixture.base.cas.counts(), before);
    Ok(())
}

/// Mixed homogeneous components reduce in decreasing degree, while zero needs no subtraction.
/// ´test:crate:symmetric-mixed-degrees-and-zero´
#[test]
fn symmetric_mixed_degrees_and_zero() -> TestResult {
    let fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let state = &fixture.base.cas.state;
    let basis =
        crate::solving::symmetric::reduction::elementary(&fixture.frame, state, &mut budget)?;
    let cube = basis
        .first()
        .ok_or("first elementary missing")?
        .pow(3, state, &mut budget)?;
    let mixed = cube
        .add(
            basis.get(1).ok_or("second elementary missing")?,
            state,
            &mut budget,
        )?
        .add(
            &fixture.frame.identity(true, state, &mut budget)?,
            state,
            &mut budget,
        )?;
    let reduced = reduce(&mixed, state, &mut budget)?;
    assert_eq!(
        oracle(&reduced.expression)?,
        Oracle::from([
            (vec![3, 0, 0], (1, 0)),
            (vec![0, 1, 0], (1, 0)),
            (vec![0, 0, 0], (1, 0))
        ])
    );
    assert_eq!(
        expansion_oracle(&oracle(&reduced.expression)?, 3),
        oracle(&mixed)?
    );
    let before = budget.usage().symmetric_steps;
    let zero = reduce(&fixture.frame, state, &mut budget)?;
    assert_eq!(zero.expression.form.terms.len(), 0);
    assert_eq!(budget.usage().symmetric_steps, before);
    assert!(
        generate(
            &fixture.frame,
            &Seed::Partition(vec![0, 2, 0]),
            None,
            state,
            &mut budget
        )
        .is_err()
    );
    assert!(
        generate(
            &fixture.frame,
            &Seed::Cyclic { index: 1, power: 4 },
            Some(&fixture.unity),
            state,
            &mut budget
        )
        .is_err()
    );
    Ok(())
}

/// Both recursive peak ceilings stop before coefficient expansion, with their exact fault kinds.
/// ´test:crate:resolvent-depth-and-degree-boundaries´
#[test]
fn resolvent_depth_and_degree_boundaries() -> TestResult {
    let fixture = Fixture::new(3)?;
    let orbit = cyclic_orbit(&fixture, 1, 3, &mut Budget::new(Limits::blind_cubic())?)?;
    for (kind, limit) in [
        (BudgetKind::ResolventDegree, 1),
        (BudgetKind::ResolventDepth, 0),
    ] {
        let mut limits = Limits::blind_cubic();
        if kind == BudgetKind::ResolventDegree {
            limits.resolvent_degree = limit;
        } else {
            limits.resolvent_depth = limit;
        }
        let mut budget = Budget::new(limits)?;
        let failure = smaller(&orbit, 3, 0, &fixture.base.cas.state, &mut budget)
            .err()
            .ok_or("short recursive limit accepted")?;
        assert!(matches!(failure.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
        assert_eq!(budget.usage().coefficient_ops, 0);
    }
    let mut exact = Budget::new(Limits {
        resolvent_degree: 2,
        resolvent_depth: 1,
        ..Limits::blind_cubic()
    })?;
    assert!(smaller(&orbit, 3, 0, &fixture.base.cas.state, &mut exact)?.is_some());
    assert_eq!(exact.usage().resolvent_degree, 2);
    assert_eq!(exact.usage().resolvent_depth, 1);
    Ok(())
}

/// Submission and independent replay each use their own measured identity boundary and atomic publication.
/// ´test:crate:symmetric-identity-budget-boundaries´
#[test]
fn symmetric_identity_budget_boundaries() -> TestResult {
    let mut fixture = Fixture::new(3)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let orbit = cyclic_orbit(&fixture, 1, 3, &mut budget)?;
    let resolvent =
        smaller(&orbit, 3, 0, &fixture.base.cas.state, &mut budget)?.ok_or("target missing")?;
    let reduction = resolvent
        .coefficients
        .get(1)
        .ok_or("linear coefficient missing")?;
    let images = depressed_images(&fixture)?;
    let mut generation = Budget::new(Limits::blind_cubic())?;
    let mut checker = Budget::new(Limits::blind_cubic())?;
    fixture.certify_with(reduction, &images, &mut generation, &mut checker)?;
    assert_eq!(generation.usage().solve_identities, 2);
    assert_eq!(checker.usage().solve_identities, 2);
    for replay in [false, true] {
        for (limit, succeeds) in [(2, true), (1, false)] {
            let bounded = Limits {
                solve_identities: limit,
                ..Limits::blind_cubic()
            };
            let mut generation = Budget::new(if replay {
                Limits::blind_cubic()
            } else {
                bounded.clone()
            })?;
            let mut checker = Budget::new(if replay {
                bounded
            } else {
                Limits::blind_cubic()
            })?;
            let before = fixture.base.cas.counts();
            let result = fixture.certify_with(reduction, &images, &mut generation, &mut checker);
            if succeeds {
                assert!(result.is_ok());
            } else {
                let error = result.err().ok_or("short identity limit accepted")?;
                let fault = error
                    .downcast_ref::<crate::Fault>()
                    .ok_or("wrong error type")?;
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::SolveIdentities,
                        ..
                    }
                ));
                assert_eq!(fixture.base.cas.counts(), before);
            }
        }
    }
    Ok(())
}
