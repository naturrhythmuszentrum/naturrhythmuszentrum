// SPDX-License-Identifier: AGPL-3.0-only
//! Matrix maps and LC similarity derivatives are checked against independent coefficient tables.

use crate::testing::common::{TestResult, real};
use crate::testing::jacobians::{coordinates, integer, lc, lc_coordinates, product, rectangle};
use crate::testing::map_inverses::{fixture, ledger};
use crate::testing::readings::matrix;
use nrz_cas::Handle;
use nrz_cas::jacobians::SourceCoordinates;
use nrz_cas::syntax::Operator;

fn linear(values: &[i64]) -> TestResult<(crate::testing::readings::Fixture, Handle, Handle)> {
    let mut case = fixture(&["x", "y"])?;
    let x = case.symbol("x")?;
    let y = case.symbol("y")?;
    let mut entries = Vec::new();
    for coefficients in values.as_chunks::<2>().0 {
        let first = integer(&mut case, coefficients[0])?;
        let second = integer(&mut case, coefficients[1])?;
        let left = case.term(Operator::Mul, real(), &[first, x.clone()])?;
        let right = case.term(Operator::Mul, real(), &[second, y.clone()])?;
        entries.push(case.term(Operator::Add, real(), &[left, right])?);
    }
    let original = case.term(
        Operator::Matrix {
            rows: 2,
            columns: 1,
        },
        matrix(2, 1),
        &entries,
    )?;
    let mut expected = Vec::new();
    for value in values {
        expected.push(integer(&mut case, *value)?);
    }
    let oracle = case.term(
        Operator::Matrix {
            rows: 2,
            columns: 2,
        },
        matrix(2, 2),
        &expected,
    )?;
    Ok((case, original, oracle))
}

/// Diagonal, triangular and general exact maps have their independent constant Jacobians.
/// ´test:integration:source-jacobian-linear-map-oracles´
#[test]
fn source_jacobian_linear_map_oracles() -> TestResult {
    for table in [[5, 0, 0, 10], [1, 2, 0, 1], [2, 3, 5, 7]] {
        let (mut case, original, oracle) = linear(&table)?;
        let mut contract = coordinates();
        contract.fixed.clear();
        let key =
            case.cas
                .source_jacobian_key(&case.context, &original, &contract, &mut ledger()?)?;
        let result = case.cas.source_jacobian(&key, &mut ledger()?)?;
        case.cas
            .prove_reading_equality(&case.context, result.matrix(), &oracle, &mut ledger()?)?;
        let bytes = case.cas.export_witness(result.witness(), &mut ledger()?)?;
        let (mut receiver, original, oracle) = linear(&table)?;
        let key = receiver.cas.source_jacobian_key(
            &receiver.context,
            &original,
            &contract,
            &mut ledger()?,
        )?;
        let rebuilt = receiver.cas.source_jacobian(&key, &mut ledger()?)?;
        receiver
            .cas
            .replay(&bytes, rebuilt.witness().judgment(), &mut ledger()?)?;
        receiver.cas.prove_reading_equality(
            &receiver.context,
            rebuilt.matrix(),
            &oracle,
            &mut ledger()?,
        )?;
    }
    Ok(())
}

/// The exact normalized LC operator has the two independently written rational sensitivities.
/// ´test:integration:source-jacobian-lc-operator-replays´
#[test]
fn source_jacobian_lc_operator_replays() -> TestResult {
    let (mut case, original, oracle) = lc(true)?;
    let key = case.cas.source_jacobian_key(
        &case.context,
        &original,
        &lc_coordinates(),
        &mut ledger()?,
    )?;
    let result = case.cas.source_jacobian(&key, &mut ledger()?)?;
    let equality =
        case.cas
            .prove_reading_equality(&case.context, result.matrix(), &oracle, &mut ledger()?)?;
    assert!(equality.is_discharged());
    assert_eq!(
        result
            .representation()
            .ok_or("original representation")?
            .hypotheses()
            .len(),
        2
    );
    let bytes = case.cas.export_witness(result.witness(), &mut ledger()?)?;
    let representation = case.cas.export_witness(
        result.representation().ok_or("representation")?,
        &mut ledger()?,
    )?;
    let (mut receiver, original, oracle) = lc(true)?;
    let key = receiver.cas.source_jacobian_key(
        &receiver.context,
        &original,
        &lc_coordinates(),
        &mut ledger()?,
    )?;
    let independently_built = receiver.cas.source_jacobian(&key, &mut ledger()?)?;
    receiver.cas.replay(
        &bytes,
        independently_built.witness().judgment(),
        &mut ledger()?,
    )?;
    receiver.cas.replay(
        &representation,
        independently_built
            .representation()
            .ok_or("representation")?
            .judgment(),
        &mut ledger()?,
    )?;
    receiver.cas.prove_reading_equality(
        &receiver.context,
        independently_built.matrix(),
        &oracle,
        &mut ledger()?,
    )?;
    Ok(())
}

/// A matrix with no entries does not traverse its unused literal axis.
/// ´test:integration:source-jacobian-empty-matrix-axis-is-bounded´
#[test]
fn source_jacobian_empty_matrix_axis_is_bounded() -> TestResult {
    let mut costs = Vec::new();
    for columns in [0, 2, u32::MAX] {
        let (mut case, _, _) = product()?;
        let original = rectangle(&mut case, 0, columns, &[])?;
        let key = case.cas.source_jacobian_key(
            &case.context,
            &original,
            &SourceCoordinates::default(),
            &mut ledger()?,
        )?;
        let mut budget = ledger()?;
        let result = case.cas.source_jacobian(&key, &mut budget)?;
        assert_eq!(
            case.cas.operands(result.matrix(), &mut ledger()?)?,
            [] as [Handle; 0]
        );
        costs.push(budget.usage().work);
    }
    // Definedness, empty expansion, representation checking/replay and empty
    // Jacobian checking/replay dominate these fixed costs. Coincident zero
    // rectangles skip 133 Work through canonical sharing; unused extent does not.
    assert_eq!(costs, [162_246, 162_379, 162_379]);
    Ok(())
}

/// Dropping the positive family also drops reciprocal premises and rejects both original and partial replay.
/// ´test:integration:source-jacobian-lc-replay-retains-premises´
#[test]
fn source_jacobian_lc_replay_retains_premises() -> TestResult {
    let (mut case, original, _) = lc(true)?;
    let key = case.cas.source_jacobian_key(
        &case.context,
        &original,
        &lc_coordinates(),
        &mut ledger()?,
    )?;
    let result = case.cas.source_jacobian(&key, &mut ledger()?)?;
    let bytes = case.cas.export_witness(result.witness(), &mut ledger()?)?;
    let mut target = result.witness().judgment().clone();
    crate::testing::jacobians::positive_family(&mut case, &[])?;
    target.context = case.context.clone();
    let before = case.cas.counts();
    assert!(case.cas.replay(&bytes, &target, &mut ledger()?).is_err());
    assert_eq!(case.cas.counts(), before);
    let changed = case.cas.source_jacobian_key(
        &case.context,
        &original,
        &lc_coordinates(),
        &mut ledger()?,
    )?;
    assert_ne!(key, changed);
    assert!(case.cas.source_jacobian(&changed, &mut ledger()?).is_err());
    Ok(())
}
