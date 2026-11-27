// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`retained_dyadic_power_pairing`] | matrix | The returned original authority, native schedule, domain and exact use stay paired. |
//! | [`retained_dyadic_power_inventory`] | matrix | Charged inventory distinguishes unique header, checked evidence, key buffers and each use. |
//! | [`retained_dyadic_power_resources`] | matrix | Construction and further uses preserve cumulative accounting and complete admission. |
//! | [`retained_dyadic_power_lifetime`] | matrix | Separately funded uses keep the same graph alive through release and heap death. |
//! | [`retained_dyadic_power_observation`] | matrix | Native reuse neither prepares another authority nor spends exact Work. |

mod bytes;
pub(in crate::loan) mod fixture;
pub(in crate::loan) mod observation;

use crate::HeapLimits;
use nrz_bindings::{Budget, readings::algebra::Rational};
use nrz_scalar::{ArithmeticContext, DyadicPowerFailure};
use std::cmp::Ordering;

/// Test failures retain their concrete source or numerical cause.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Decode one finite positive binary64 endpoint as an exact rational for golden comparison.
fn represented(value: f64, budget: &mut Budget) -> TestResult<Rational> {
    let bits = value.to_bits();
    let field = (bits >> 52) & 2047;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (mantissa, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction + (1_u64 << 52), i64::try_from(field)? - 1075)
    };
    let exact = Rational::from_ratio(i64::try_from(mantissa)?, 1, budget)?;
    let two = Rational::from_ratio(2, 1, budget)?;
    Ok(exact.mul(&two.pow(exponent, budget)?, budget)?)
}

/// Actual returned authority and numerical results share one private owner and original domain.
/// # Panics
/// Panics only if original identities or retained native readings cease to agree.
/// ´claim:matrix:retained-dyadic-power-pairing´
/// ´test:unit:retained-dyadic-power-pairing´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one selected control keeps its paired golden fixtures together"
)]
fn retained_dyadic_power_pairing() -> TestResult {
    let (heap, mut account, law) = fixture::prepare()?;
    let judgment = law.authority()?.judgment();
    assert_eq!(
        judgment.request().goal,
        nrz_bindings::readings::algebra::enclosure::EnclosureGoal::Width
    );
    assert_eq!(law.domain()?, [1.0, 2.0]);
    let arithmetic = ArithmeticContext::new()?;
    let (mut guard_cas, guard_target, guard_context) = fixture::original(1, 2)?;
    let mut guard_budget = Budget::new(fixture::limits())?;
    let mut guard_work = 0;
    let guard_authority = nrz_bindings::DyadicPowerPreparation::prepare(
        &mut guard_cas,
        &guard_target,
        &guard_context,
        [1.0, 2.0],
        fixture::policy(),
        &mut guard_budget,
        &mut |_| false,
        &mut guard_work,
    )?;
    crate::prepared_dyadic_power::guards::mismatches(
        &guard_cas,
        &guard_target,
        &guard_context,
        &guard_authority,
        &arithmetic,
        fixture::scalar_limits(),
        &mut Budget::new(fixture::limits())?,
    )?;
    let one = law.evaluate(1.0, &arithmetic)?;
    assert_eq!(one.reading().endpoints, [1.0, 1.0]);
    assert!(std::ptr::eq(one.authority(), law.authority()?));
    let two = law.evaluate(2.0, &arithmetic)?.reading();
    assert!(two.endpoints[0] * two.endpoints[0] <= 2.0);
    assert!(two.endpoints[1] * two.endpoints[1] >= 2.0);
    assert!(matches!(
        law.evaluate(0.5, &arithmetic),
        Err(DyadicPowerFailure::OutsideDomain)
    ));
    let (narrow_cas, narrow_target, narrow_context) = fixture::original(1, 2)?;
    let mut narrow_heap = crate::Heap::new(HeapLimits::default())?;
    let mut narrow_account = crate::ExecutionAccount::new(Budget::new(fixture::limits())?)?;
    let narrow = narrow_heap
        .prepare_retained_dyadic_power(
            narrow_cas,
            &narrow_target,
            &narrow_context,
            [0.5, 1.0],
            fixture::policy(),
            fixture::scalar_limits(),
            &arithmetic,
            &mut |_| false,
            &mut narrow_account,
        )
        .map_err(|(error, _)| error)?;
    assert_eq!(narrow.domain()?, [0.5, 1.0]);
    assert!(matches!(
        narrow.evaluate(1.5, &arithmetic),
        Err(DyadicPowerFailure::OutsideDomain)
    ));
    let other = crate::Heap::new(HeapLimits::default())?;
    assert!(
        other
            .retained_dyadic_power_inventory(&law, &mut account)
            .is_err()
    );
    let inventory = heap.retained_dyadic_power_inventory(&law, &mut account)?;
    assert_ne!(inventory.owner_identity, 0);
    assert_eq!(inventory.uses, 1);
    let (cas, target, context) = fixture::original(1, 8)?;
    let mut other_heap = crate::Heap::new(HeapLimits::default())?;
    let mut other_account = crate::ExecutionAccount::new(Budget::new(fixture::limits())?)?;
    let other_law = other_heap
        .prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            [1.0, 2.0],
            fixture::policy(),
            fixture::scalar_limits(),
            &arithmetic,
            &mut |_| false,
            &mut other_account,
        )
        .map_err(|(error, _)| error)?;
    assert_ne!(
        law.frozen()?.parts.authority_key,
        other_law.frozen()?.parts.authority_key
    );
    assert_ne!(
        law.frozen()?.parts.lowering_key,
        other_law.frozen()?.parts.lowering_key
    );
    let output = other_law.evaluate(2.0, &arithmetic)?.reading();
    let corpus = nrz_docgen::golden::read(
        include_bytes!("../../../../../data/golden/dyadic-power.json").as_slice(),
        nrz_docgen::golden::Limits::default(),
        &mut Budget::new(fixture::threshold_limits())?,
    )?;
    let row = corpus.value()["entries"]
        .as_array()
        .ok_or("golden entries")?
        .iter()
        .find(|row| row["id"] == "power-upper")
        .ok_or("power-upper")?;
    assert_eq!(row["exponent"], "1/8");
    assert_eq!(row["input"]["point"], "2");
    let pair = row["golden"]["point"].as_array().ok_or("golden pair")?;
    let mut exact = Budget::new(fixture::limits())?;
    let low = Rational::parse(pair[0].as_str().ok_or("golden lower")?, &mut exact)?;
    let high = Rational::parse(pair[1].as_str().ok_or("golden upper")?, &mut exact)?;
    assert_ne!(
        represented(output.endpoints[0], &mut exact)?.compare(&low, &mut exact)?,
        Ordering::Greater
    );
    assert_ne!(
        high.compare(&represented(output.endpoints[1], &mut exact)?, &mut exact)?,
        Ordering::Greater
    );
    let normal = f64::MIN_POSITIVE;
    let next = f64::from_bits(normal.to_bits() + 1);
    for (id, numerator, denominator, point, exponent) in [
        ("power-normal-near-one", 255, 256, normal, "255/256"),
        ("power-normal-next-near-one", 255, 256, next, "255/256"),
        ("power-normal-identity", 1, 1, normal, "1"),
    ] {
        let (cas, target, context) = fixture::threshold_original(numerator, denominator)?;
        let mut point_heap = crate::Heap::new(HeapLimits::default())?;
        let mut point_account =
            crate::ExecutionAccount::new(Budget::new(fixture::threshold_limits())?)?;
        let point_law = point_heap
            .prepare_retained_dyadic_power(
                cas,
                &target,
                &context,
                [normal, next],
                fixture::policy(),
                fixture::scalar_limits(),
                &arithmetic,
                &mut |_| false,
                &mut point_account,
            )
            .map_err(|(error, _)| error)?;
        let reading = point_law.evaluate(point, &arithmetic)?.reading();
        let row = corpus.value()["entries"]
            .as_array()
            .ok_or("golden entries")?
            .iter()
            .find(|row| row["id"] == id)
            .ok_or("normal golden row")?;
        assert_eq!(row["exponent"], exponent);
        let mut exact = Budget::new(fixture::threshold_limits())?;
        let input = Rational::parse(
            row["input"]["point"].as_str().ok_or("normal input")?,
            &mut exact,
        )?;
        assert_eq!(represented(point, &mut exact)?, input);
        let pair = row["golden"]["point"].as_array().ok_or("normal pair")?;
        let golden_low = Rational::parse(pair[0].as_str().ok_or("normal low")?, &mut exact)?;
        let golden_high = Rational::parse(pair[1].as_str().ok_or("normal high")?, &mut exact)?;
        let lower = represented(reading.endpoints[0], &mut exact)?;
        let upper = represented(reading.endpoints[1], &mut exact)?;
        assert_ne!(lower.compare(&golden_low, &mut exact)?, Ordering::Greater);
        assert_ne!(golden_high.compare(&upper, &mut exact)?, Ordering::Greater);
        assert!(reading.center.is_finite());
        assert!(reading.total_radius.is_finite() && reading.total_radius >= 0.0);
        assert_eq!(reading.numerical_error, reading.total_radius);
        let center = represented(reading.center, &mut exact)?;
        let radius = represented(reading.total_radius, &mut exact)?;
        assert_ne!(
            center
                .sub(&radius, &mut exact)?
                .compare(&golden_low, &mut exact)?,
            Ordering::Greater
        );
        assert_ne!(
            golden_high.compare(&center.add(&radius, &mut exact)?, &mut exact)?,
            Ordering::Greater
        );
        assert!(matches!(
            point_law.evaluate(f64::from_bits(normal.to_bits() - 1), &arithmetic),
            Err(DyadicPowerFailure::BelowNormalOperand)
        ));
    }
    let (default_cas, default_target, default_context) = fixture::original(255, 256)?;
    let mut default_heap = crate::Heap::new(HeapLimits::default())?;
    let mut default_account = crate::ExecutionAccount::new(Budget::new(fixture::limits())?)?;
    let (default_error, default_returned) = default_heap
        .prepare_retained_dyadic_power(
            default_cas,
            &default_target,
            &default_context,
            [normal, next],
            fixture::policy(),
            fixture::scalar_limits(),
            &arithmetic,
            &mut |_| false,
            &mut default_account,
        )
        .expect_err("normal-threshold preparation needs the isolated exponent cap");
    default_returned.expression(&default_target, &mut Budget::new(fixture::limits())?)?;
    let crate::RetainedDyadicPowerError::Preparation(crate::PreparedDyadicPowerError::Bindings(
        nrz_bindings::DyadicPowerPreparationError::Exact(default_fault),
    )) = default_error
    else {
        return Err("default-cap threshold refusal phase".into());
    };
    assert_eq!(
        default_fault.kind,
        nrz_bindings::readings::algebra::FaultKind::Exhausted {
            reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
            budget: nrz_bindings::readings::algebra::limits::BudgetKind::IntegerExponent,
            limit: 256,
            needed: 1074,
        }
    );
    Ok(())
}

/// The owner and each use have separately visible retained byte classes.
/// # Panics
/// Panics only if a retained class disappears, duplicates, or loses its charged use.
/// ´claim:matrix:retained-dyadic-power-inventory´
/// ´test:unit:retained-dyadic-power-inventory´
#[test]
fn retained_dyadic_power_inventory() -> TestResult {
    let (mut heap, mut account, law) = fixture::prepare()?;
    let before_inspection = account.usage().work;
    let first = heap.retained_dyadic_power_inventory(&law, &mut account)?;
    assert!(account.usage().work > before_inspection);
    let [h, u, row] = bytes::headers();
    let e = bytes::evidence(law.authority()?)?;
    let parts = &law.frozen()?.parts;
    let mut encoding_budget = Budget::new(fixture::limits())?;
    let endpoint_bytes = [
        law.authority()?
            .judgment()
            .interval()
            .lower()
            .canonical_bytes(&mut encoding_budget)?
            .len(),
        law.authority()?
            .judgment()
            .interval()
            .upper()
            .canonical_bytes(&mut encoding_budget)?
            .len(),
    ];
    let authority_length = b"NRZ-DYADIC-AUTHORITY".len()
        + 4
        + 3 * 8
        + parts.authority.proof_bytes().len()
        + endpoint_bytes.into_iter().sum::<usize>()
        + 8
        + 4
        + 2 * 8
        + 2 * 4
        + 1;
    let lowering_length = b"NRZ-DYADIC-LOWERING".len() + 4 + 8 + authority_length + 2 + 1;
    assert_eq!(parts.authority_key.len(), authority_length);
    assert_eq!(parts.lowering_key.len(), lowering_length);
    assert!(parts.authority_key.capacity() >= authority_length);
    assert!(parts.lowering_key.capacity() >= lowering_length);
    let keys = parts.authority.proof_capacity()
        + parts.authority_key.capacity()
        + parts.lowering_key.capacity();
    assert_eq!(
        [
            first.owner_bytes,
            first.evidence_bytes,
            first.key_bytes,
            first.use_bytes
        ],
        [h, e, keys, u]
    );
    assert_eq!([h, u, row], [1808, 160, 264]);
    assert_eq!(e, 2549);
    assert_eq!(keys, 3474);
    assert_eq!(h + e + keys + u + 2 * row, 8519);
    assert!(first.graph_bytes > 0);
    assert!(first.use_bytes > 0);
    assert_eq!(first.uses, 1);
    assert_eq!(
        first.evidence_bytes as u64,
        law.authority()?.retained_bytes()
    );
    let second = heap.retain_retained_dyadic_power(&law, &mut account)?;
    let both = heap.retained_dyadic_power_inventory(&second, &mut account)?;
    assert_eq!(both.owner_identity, first.owner_identity);
    assert_eq!(both.owner_bytes, first.owner_bytes);
    assert_eq!(both.evidence_bytes, first.evidence_bytes);
    assert_eq!(both.key_bytes, first.key_bytes);
    assert_eq!(both.uses, 2);
    assert_eq!(both.use_bytes, first.use_bytes * 2);
    Ok(())
}

/// Constructor and further-use receipts stay cumulative on the same original account.
/// # Panics
/// Panics only if cumulative receipts, exact or one-below Work, or use authority moves.
/// ´claim:matrix:retained-dyadic-power-resources´
/// ´test:unit:retained-dyadic-power-resources´
#[expect(
    clippy::too_many_lines,
    reason = "the selected control compares cumulative receipts and exact resource boundaries"
)]
#[test]
fn retained_dyadic_power_resources() -> TestResult {
    /// Existing caller reservation used by the independent use boundary.
    const PREFIX: u64 = 1_000_000;
    /// Unrelated active reservation beside the retained law.
    const UNRELATED: u64 = 64;
    /// Exact resource scope for this isolated boundary.
    const AT: nrz_bindings::readings::algebra::Location =
        nrz_bindings::readings::algebra::Location::new("retained dyadic use boundary fixture");
    public_constructor_boundary()?;
    public_use_boundary()?;
    let [h, u, row] = bytes::headers();
    for warm in [false, true] {
        let mut heap = crate::Heap::new(HeapLimits::default())?;
        let mut account =
            crate::ExecutionAccount::new(nrz_bindings::Budget::new(fixture::limits())?)?;
        let initial = heap.resource_bytes()?;
        let law = fixture::prepare_on(&mut heap, &mut account, warm)?;
        let expected_e = bytes::evidence(law.authority()?)?;
        let actual_e = usize::try_from(law.authority()?.retained_bytes())?;
        assert_eq!(actual_e, expected_e);
        let parts = &law.frozen()?.parts;
        let keys = parts.authority.proof_capacity()
            + parts.authority_key.capacity()
            + parts.lowering_key.capacity();
        assert_eq!(
            heap.resource_bytes()? - initial,
            h + expected_e + keys + u + 2 * row
        );
        let constructor_work = account.usage().work;
        let before = heap.retained_dyadic_power_inventory(&law, &mut account)?;
        let inspected_work = account.usage().work;
        assert!(constructor_work > 0 && inspected_work > constructor_work);
        let old_capacity = heap.loans.debits.capacity();
        assert_eq!(old_capacity, 2);
        let staged_use = u + row + 2 * old_capacity * row;
        assert_eq!(staged_use, 1480);
        for short in [0, 1] {
            let mut boundary = Budget::new(nrz_bindings::Limits {
                staged_payload_bytes: PREFIX + UNRELATED + u64::try_from(staged_use - short)?,
                ..fixture::limits()
            })?;
            let injection = crate::loan::fault::Injection::after(usize::MAX);
            let trial = boundary.with_retained_payload(PREFIX, AT, |budget| {
                budget.with_retained_payload(UNRELATED, AT, |budget| {
                    super::admission::Plan::new(&heap, account.identity, None, 7, budget)
                })
            });
            if short == 0 {
                assert!(trial.is_ok());
            } else {
                let crate::LoanError::Budget(fault) = trial.err().ok_or("one-byte-short use")?
                else {
                    return Err("typed use boundary refusal".into());
                };
                assert_eq!(
                    fault.kind,
                    nrz_bindings::readings::algebra::FaultKind::Exhausted {
                        reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                        budget:
                            nrz_bindings::readings::algebra::limits::BudgetKind::StagedPayloadBytes,
                        limit: u64::try_from(staged_use - 1)?,
                        needed: u64::try_from(staged_use)?,
                    }
                );
            }
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            assert_eq!(heap.loans.debits.iter().flatten().count(), 2);
            drop(injection);
        }
        let extra = heap.retain_retained_dyadic_power(&law, &mut account)?;
        assert_eq!(
            [constructor_work, inspected_work, account.usage().work],
            if warm {
                [169_039, 169_045, 169_061]
            } else {
                [127_549, 127_555, 127_571]
            }
        );
        assert!(account.usage().work > inspected_work);
        let after = heap.retained_dyadic_power_inventory(&extra, &mut account)?;
        assert_eq!(
            (before.owner_bytes, before.evidence_bytes, before.key_bytes),
            (after.owner_bytes, after.evidence_bytes, after.key_bytes)
        );
        assert_eq!(after.uses, 2);
        assert_eq!(
            heap.resource_bytes()? - initial,
            h + expected_e + keys + 2 * u + heap.loans.debits.capacity() * row
        );
    }
    for (warm, exact) in [(false, 127_549), (true, 169_039)] {
        for short in [0, 1] {
            let mut heap = crate::Heap::new(HeapLimits::default())?;
            let mut account = crate::ExecutionAccount::new(Budget::new(nrz_bindings::Limits {
                work: exact - short,
                ..fixture::limits()
            })?)?;
            let result = fixture::prepare_on(&mut heap, &mut account, warm);
            if short == 0 {
                let law = result?;
                assert!(law.authority()?.judgment().retained_bytes() > 0);
                assert_eq!(account.usage().work, exact);
            } else {
                let error = result.err().ok_or("one Work short must refuse")?;
                let Some(crate::RetainedDyadicPowerError::Ownership(crate::LoanError::Budget(
                    fault,
                ))) = error.downcast_ref::<crate::RetainedDyadicPowerError>()
                else {
                    return Err("one Work short refusal phase".into());
                };
                assert_eq!(
                    fault.kind,
                    nrz_bindings::readings::algebra::FaultKind::Exhausted {
                        reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                        budget: nrz_bindings::readings::algebra::limits::BudgetKind::Work,
                        limit: exact - 1,
                        needed: exact,
                    }
                );
                assert_eq!(account.usage().work, exact - 9);
            }
        }
    }
    complete_owner_boundary()?;
    public_preparation_refusals()?;
    Ok(())
}

/// Independently enumerate a public owner peak with a live graph and growing debit table.
/// # Panics
/// Panics only if the measured exact or one-below public outcome is internally inconsistent.
fn public_constructor_boundary() -> TestResult {
    let (mut producing, target, context) = warm_original()?;
    let mut probe_work = 0;
    let parts = crate::prepared_dyadic_power::owned_parts(
        &mut producing,
        &target,
        &context,
        [1.0, 2.0],
        fixture::policy(),
        fixture::scalar_limits(),
        &ArithmeticContext::new()?,
        &mut Budget::new(fixture::limits())?,
        &mut |_| false,
        &mut probe_work,
    )?;
    let evidence = bytes::evidence(&parts.authority)?;
    let keys = parts.key_bytes()?;
    let [header, use_header, row] = bytes::headers();
    let occupied = 1025_usize;
    let managed = header + evidence + keys + use_header + 2 * row + 2 * occupied * row;
    let graph = producing.counts().payload_bytes;
    let boundary = graph + u64::try_from(managed)?;
    drop((parts, producing));
    for short in [0, 1] {
        let mut heap = crate::Heap::new(HeapLimits::default())?;
        let mut account = crate::ExecutionAccount::new(Budget::new(nrz_bindings::Limits {
            staged_payload_bytes: boundary - short,
            ..fixture::limits()
        })?)?;
        let (seed, seed_target, seed_context) = warm_original()?;
        let first = heap
            .prepare_retained_dyadic_power(
                seed,
                &seed_target,
                &seed_context,
                [1.0, 2.0],
                fixture::policy(),
                fixture::scalar_limits(),
                &ArithmeticContext::new()?,
                &mut |_| false,
                &mut account,
            )
            .map_err(|(error, _)| error)?;
        let mut uses = Vec::new();
        for _ in 0..1023 {
            uses.push(heap.retain_retained_dyadic_power(&first, &mut account)?);
        }
        assert_eq!(heap.loans.debits.capacity(), occupied);
        let (cas, target, context) = warm_original()?;
        let before = heap.resource_bytes()?;
        let result = heap.prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            [1.0, 2.0],
            fixture::policy(),
            fixture::scalar_limits(),
            &ArithmeticContext::new()?,
            &mut |_| false,
            &mut account,
        );
        if short == 0 {
            let law = result.map_err(|(error, _)| error)?;
            assert_eq!(
                heap.resource_bytes()? - before,
                header + evidence + keys + use_header + 2 * row
            );
            assert!(law.authority()?.retained_bytes() > 0);
        } else {
            let (error, returned) = result.expect_err("one below public owner peak");
            let crate::RetainedDyadicPowerError::Ownership(crate::LoanError::Budget(fault)) = error
            else {
                return Err(format!("{error:?}").into());
            };
            assert_eq!(
                fault.kind,
                nrz_bindings::readings::algebra::FaultKind::Exhausted {
                    reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                    budget: nrz_bindings::readings::algebra::limits::BudgetKind::StagedPayloadBytes,
                    limit: u64::try_from(managed - 1)?,
                    needed: u64::try_from(managed)?,
                }
            );
            returned.expression(&target, &mut Budget::new(fixture::limits())?)?;
            assert_eq!(heap.resource_bytes()?, before);
        }
        drop(uses);
    }
    Ok(())
}

/// Public further use exhausts the replacement peak after real debit occupancy.
fn public_use_boundary() -> TestResult {
    use nrz_bindings::readings::algebra::{Exhaustion, FaultKind, limits::BudgetKind};
    let [_, use_header, row] = bytes::headers();
    let occupied = 1025_usize;
    let staged = use_header + row + 2 * occupied * row;
    for short in [0, 1] {
        let mut heap = crate::Heap::new(HeapLimits::default())?;
        let mut account = crate::ExecutionAccount::new(Budget::new(nrz_bindings::Limits {
            staged_payload_bytes: u64::try_from(staged - short)?,
            ..fixture::limits()
        })?)?;
        let law = fixture::prepare_on(&mut heap, &mut account, false)?;
        let mut uses = Vec::new();
        for _ in 0..1023 {
            uses.push(heap.retain_retained_dyadic_power(&law, &mut account)?);
        }
        assert_eq!(heap.loans.debits.capacity(), occupied);
        let before = heap.resource_bytes()?;
        let result = heap.retain_retained_dyadic_power(&law, &mut account);
        if short == 0 {
            let extra = result?;
            assert_eq!(heap.resource_bytes()? - before, use_header + row);
            drop(extra);
        } else {
            let crate::LoanError::Budget(fault) = result.expect_err("one below public use peak")
            else {
                return Err("typed public use refusal".into());
            };
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: Exhaustion::Limit,
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: u64::try_from(staged - 1)?,
                    needed: u64::try_from(staged)?,
                }
            );
            assert_eq!(heap.resource_bytes()?, before);
        }
        drop(uses);
    }
    Ok(())
}

/// Seed an ordinary checked enclosure on its own ledger without owning a result bundle.
fn warm_original() -> TestResult<(
    nrz_bindings::readings::algebra::Cas,
    nrz_bindings::readings::algebra::Handle,
    nrz_bindings::readings::algebra::Handle,
)> {
    let (mut cas, target, context) = fixture::original(1, 2)?;
    let mut work = 0;
    let _seed = nrz_bindings::DyadicPowerPreparation::prepare(
        &mut cas,
        &target,
        &context,
        [1.0, 2.0],
        fixture::policy(),
        &mut Budget::new(fixture::limits())?,
        &mut |_| false,
        &mut work,
    )?;
    assert!(work > 0);
    Ok((cas, target, context))
}

/// Public early refusals return their graph and exactly the reached inspection prefix.
/// # Panics
/// Panics only if a numerical, carrier or zero-Work cause or returned graph changes.
#[expect(
    clippy::too_many_lines,
    reason = "the selected control compares every public refusal prefix"
)]
fn public_preparation_refusals() -> TestResult {
    use crate::{PreparedDyadicPowerError, RetainedDyadicPowerError};
    use nrz_bindings::readings::algebra::{Location, limits::BudgetKind};
    use nrz_bindings::{DyadicPowerPreparation, DyadicPowerPreparationError};
    /// Fixed location for the public refusal prefix fixture.
    const AT: Location = Location::new("dyadic public refusal prefix fixture");
    for (numerator, denominator) in [(0, 1), (1, 3)] {
        let (cas, target, context) = fixture::original(numerator, denominator)?;
        let before = cas.counts();
        let (oracle, oracle_target, oracle_context) = fixture::original(numerator, denominator)?;
        let mut oracle_budget = Budget::new(fixture::limits())?;
        oracle_budget.work(7, AT)?;
        drop(DyadicPowerPreparation::original_exponent(
            &oracle,
            &oracle_target,
            &oracle_context,
            &mut oracle_budget,
        ));
        let mut budget = Budget::new(fixture::limits())?;
        budget.work(7, AT)?;
        let mut account = crate::ExecutionAccount::new(budget)?;
        let mut heap = crate::Heap::new(HeapLimits::default())?;
        let initial = heap.resource_bytes()?;
        let (error, returned) = heap
            .prepare_retained_dyadic_power(
                cas,
                &target,
                &context,
                [1.0, 2.0],
                fixture::policy(),
                fixture::scalar_limits(),
                &ArithmeticContext::new()?,
                &mut |_| false,
                &mut account,
            )
            .expect_err("early public preparation refusal");
        if numerator == 0 {
            assert!(matches!(
                error,
                RetainedDyadicPowerError::Preparation(PreparedDyadicPowerError::Native(
                    DyadicPowerFailure::OutsideDomain
                ))
            ));
        } else {
            assert!(matches!(
                error,
                RetainedDyadicPowerError::Preparation(PreparedDyadicPowerError::Native(
                    DyadicPowerFailure::NonDyadicExponent
                ))
            ));
        }
        returned.expression(&target, &mut Budget::new(fixture::limits())?)?;
        returned.context_header(&context, &mut Budget::new(fixture::limits())?)?;
        assert_eq!(returned.counts(), before);
        assert_eq!(account.usage().work, oracle_budget.usage().work);
        assert!(heap.loans.debits.iter().all(Option::is_none));
        assert_eq!(heap.resource_bytes()?, initial);
    }

    let mut rational_budget = Budget::new(fixture::limits())?;
    let tiny = Rational::from_ratio(1, 1, &mut rational_budget)?.div(
        &Rational::from_ratio(2, 1, &mut rational_budget)?.pow(64, &mut rational_budget)?,
        &mut rational_budget,
    )?;
    let (cas, target, context) = fixture::original_rational(tiny.clone())?;
    let before = cas.counts();
    let (oracle, oracle_target, oracle_context) = fixture::original_rational(tiny)?;
    let mut oracle_budget = Budget::new(fixture::limits())?;
    oracle_budget.work(7, AT)?;
    assert!(matches!(
        DyadicPowerPreparation::original_exponent(
            &oracle,
            &oracle_target,
            &oracle_context,
            &mut oracle_budget,
        ),
        Err(DyadicPowerPreparationError::ExponentCarrierLimit)
    ));
    let mut budget = Budget::new(fixture::limits())?;
    budget.work(7, AT)?;
    let mut account = crate::ExecutionAccount::new(budget)?;
    let mut heap = crate::Heap::new(HeapLimits::default())?;
    let (error, returned) = heap
        .prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            [1.0, 2.0],
            fixture::policy(),
            fixture::scalar_limits(),
            &ArithmeticContext::new()?,
            &mut |_| false,
            &mut account,
        )
        .expect_err("carrier refusal");
    assert!(matches!(
        error,
        RetainedDyadicPowerError::Preparation(PreparedDyadicPowerError::Bindings(
            DyadicPowerPreparationError::ExponentCarrierLimit
        ))
    ));
    returned.expression(&target, &mut Budget::new(fixture::limits())?)?;
    returned.context_header(&context, &mut Budget::new(fixture::limits())?)?;
    assert_eq!(returned.counts(), before);
    assert_eq!(account.usage().work, oracle_budget.usage().work);
    assert!(heap.loans.debits.iter().all(Option::is_none));

    let (cas, target, context) = fixture::original(1, 2)?;
    let before = cas.counts();
    let mut account = crate::ExecutionAccount::new(Budget::new(nrz_bindings::Limits {
        work: 0,
        ..fixture::limits()
    })?)?;
    let mut heap = crate::Heap::new(HeapLimits::default())?;
    let (error, returned) = heap
        .prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            [1.0, 2.0],
            fixture::policy(),
            fixture::scalar_limits(),
            &ArithmeticContext::new()?,
            &mut |_| false,
            &mut account,
        )
        .expect_err("zero Work refusal");
    let RetainedDyadicPowerError::Preparation(PreparedDyadicPowerError::Bindings(
        DyadicPowerPreparationError::Exact(fault),
    )) = error
    else {
        return Err("zero Work typed cause".into());
    };
    assert!(matches!(
        fault.kind,
        nrz_bindings::readings::algebra::FaultKind::Exhausted {
            budget: BudgetKind::Work,
            limit: 0,
            needed: 1,
            ..
        }
    ));
    assert_eq!(account.usage().work, 0);
    returned.expression(&target, &mut Budget::new(fixture::limits())?)?;
    returned.context_header(&context, &mut Budget::new(fixture::limits())?)?;
    assert_eq!(returned.counts(), before);
    assert!(heap.loans.debits.iter().all(Option::is_none));

    let (cas, target, context) = fixture::original(1, 2)?;
    let before = cas.counts();
    let (mut oracle, oracle_target, oracle_context) = fixture::original(1, 2)?;
    let mut oracle_budget = Budget::new(fixture::limits())?;
    oracle_budget.work(7, AT)?;
    let mut oracle_work = 0;
    let oracle_error = DyadicPowerPreparation::prepare(
        &mut oracle,
        &oracle_target,
        &oracle_context,
        [1.0, 2.0],
        fixture::policy(),
        &mut oracle_budget,
        &mut |_| true,
        &mut oracle_work,
    )
    .expect_err("immediately interrupted public bindings attempt");
    let DyadicPowerPreparationError::Stopped(oracle_report) = oracle_error else {
        return Err("typed interruption report".into());
    };
    assert!(matches!(
        oracle_report.outcome,
        nrz_bindings::readings::algebra::enclosure::EnclosureOutcome::Interrupted
    ));
    assert_eq!(oracle_report.completed_rounds, 0);
    assert!(oracle_report.checked.is_none());
    assert!(oracle_report.resource_fault.is_none());
    let mut budget = Budget::new(fixture::limits())?;
    budget.work(7, AT)?;
    let mut account = crate::ExecutionAccount::new(budget)?;
    let mut heap = crate::Heap::new(HeapLimits::default())?;
    let (error, returned) = heap
        .prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            [1.0, 2.0],
            fixture::policy(),
            fixture::scalar_limits(),
            &ArithmeticContext::new()?,
            &mut |_| true,
            &mut account,
        )
        .expect_err("immediate interruption");
    let RetainedDyadicPowerError::Preparation(PreparedDyadicPowerError::Bindings(
        DyadicPowerPreparationError::Stopped(report),
    )) = error
    else {
        return Err("typed public interruption".into());
    };
    assert!(matches!(
        report.outcome,
        nrz_bindings::readings::algebra::enclosure::EnclosureOutcome::Interrupted
    ));
    assert_eq!(report.completed_rounds, 0);
    assert!(report.checked.is_none() && report.resource_fault.is_none());
    assert_eq!(report.work, oracle_report.work);
    let (prelude, prelude_target, prelude_context) = fixture::original(1, 2)?;
    let mut prelude_budget = Budget::new(fixture::limits())?;
    let _first = DyadicPowerPreparation::original_exponent(
        &prelude,
        &prelude_target,
        &prelude_context,
        &mut prelude_budget,
    )?;
    let operands = prelude.operands(&prelude_target, &mut prelude_budget)?;
    let exponent = operands.get(1).ok_or("original exponent operand")?;
    let (nrz_bindings::readings::algebra::syntax::Operator::Rational(original), _) =
        prelude.expression(exponent, &mut prelude_budget)?
    else {
        return Err("original rational exponent".into());
    };
    let exact = Rational::from_ratio(1, 1, &mut prelude_budget)?.div(
        &Rational::from_ratio(2, 1, &mut prelude_budget)?.pow(1, &mut prelude_budget)?,
        &mut prelude_budget,
    )?;
    assert_eq!(
        exact.compare(original, &mut prelude_budget)?,
        Ordering::Equal
    );
    assert_eq!(prelude_budget.usage().work, 3306);
    assert_eq!(
        account.usage().work,
        oracle_budget.usage().work + prelude_budget.usage().work
    );
    returned.expression(&target, &mut Budget::new(fixture::limits())?)?;
    returned.context_header(&context, &mut Budget::new(fixture::limits())?)?;
    assert_eq!(returned.counts(), before);
    assert!(heap.loans.debits.iter().all(Option::is_none));
    Ok(())
}

/// Independent fixture bytes place the actual owner shell at its complete staged boundary.
/// # Panics
/// Panics only if the independently counted owner peak, refusal or allocation prefix moves.
fn complete_owner_boundary() -> TestResult {
    use crate::loan::fault::Injection;
    use nrz_bindings::readings::algebra::{Exhaustion, FaultKind, Location, limits::BudgetKind};
    /// Persistent caller reservation outside the owner peak.
    const PREFIX: u64 = 1_000_000;
    /// Unrelated retained reservation alongside the producing graph.
    const UNRELATED: u64 = 64;
    /// Stable fault location for the combined boundary fixture.
    const AT: Location = Location::new("retained dyadic owner boundary fixture");
    let (mut cas, target, context) = fixture::original(1, 2)?;
    let mut setup = Budget::new(fixture::limits())?;
    let mut work = 0;
    let prepared = nrz_bindings::DyadicPowerPreparation::prepare(
        &mut cas,
        &target,
        &context,
        [1.0, 2.0],
        fixture::policy(),
        &mut setup,
        &mut |_| false,
        &mut work,
    )?;
    assert_eq!(bytes::evidence(&prepared)?, 2549);
    let graph = cas.counts().payload_bytes;
    let judgments = cas.retained_judgment_bytes();
    let [header, use_header, row] = bytes::headers();
    let staged = u64::try_from(header + 3474 + 2549 + use_header + 2 * row)?;
    assert_eq!(staged, 8519);
    let boundary = PREFIX + UNRELATED + graph + staged;
    for short in [0, 1] {
        let mut budget = Budget::new(nrz_bindings::Limits {
            staged_payload_bytes: boundary - short,
            ..fixture::limits()
        })?;
        let mut receipt = 0;
        let registered = nrz_bindings::DyadicPowerPreparation::prepare(
            &mut cas,
            &target,
            &context,
            [1.0, 2.0],
            fixture::policy(),
            &mut budget,
            &mut |_| false,
            &mut receipt,
        )?;
        assert_eq!(registered.retained_bytes(), 2549);
        assert_eq!(cas.retained_judgment_bytes(), judgments);
        let mut heap = crate::Heap::new(HeapLimits::default())?;
        let seal = super::Seal {
            domain: [1.0_f64.to_bits(), 2.0_f64.to_bits()],
            exponent: (1, 1),
            revisions: [1, ArithmeticContext::new()?.revision(), 0],
        };
        let injection = Injection::after(usize::MAX);
        let result = budget.with_retained_payload(PREFIX, AT, |budget| {
            budget.with_retained_payload(UNRELATED, AT, |budget| {
                cas.with_retained_graph(budget, |budget| {
                    super::admission::shells(&mut heap, 17, 2549, 3474, budget, seal)
                })
            })
        });
        if short == 0 {
            drop(result?);
            assert_eq!(Injection::attempts(), 3);
        } else {
            let crate::LoanError::Budget(fault) = result.err().ok_or("one-byte-short owner")?
            else {
                return Err("typed owner boundary refusal".into());
            };
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: Exhaustion::Limit,
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: staged - 1,
                    needed: staged,
                }
            );
            assert_eq!(Injection::attempts(), 0);
        }
        assert!(heap.loans.debits.iter().all(Option::is_none));
        drop(injection);
    }
    Ok(())
}

/// The last funded use owns the producing graph independently of heap and account survival.
/// # Panics
/// Panics only if releasing one edge destroys or substitutes the other use's authority.
/// ´claim:matrix:retained-dyadic-power-lifetime´
/// ´test:unit:retained-dyadic-power-lifetime´
#[expect(
    clippy::too_many_lines,
    reason = "lifetime control includes both cross-family debit substitutions"
)]
#[test]
fn retained_dyadic_power_lifetime() -> TestResult {
    public_stopped_preparation()?;
    allocation_and_scan_prefixes()?;
    let (mut dyadic_heap, mut dyadic_account, dyadic_law) = fixture::prepare()?;
    let dyadic_slot = dyadic_heap
        .loans
        .debits
        .iter()
        .position(|entry| {
            matches!(
                entry,
                Some(crate::loan::registry::Debit {
                    kind: crate::loan::registry::AllocationKind::DyadicOwner { .. },
                    ..
                })
            )
        })
        .ok_or("live dyadic owner debit")?;
    let dyadic_debit = dyadic_heap.loans.debits[dyadic_slot]
        .as_mut()
        .ok_or("dyadic debit")?;
    let crate::loan::registry::DebitOwner::Dyadic(dyadic_identity) = dyadic_debit.owner else {
        return Err("dyadic owner identity".into());
    };
    dyadic_debit.owner = crate::loan::registry::DebitOwner::Prepared(dyadic_identity);
    let dyadic_bytes = dyadic_heap.resource_bytes()?;
    assert!(matches!(
        dyadic_heap.reconcile_detached(&mut dyadic_account, 1000, None),
        crate::ScanOutcome::Refused(crate::LoanError::WrongLoan)
    ));
    assert_eq!(dyadic_heap.resource_bytes()?, dyadic_bytes);
    dyadic_heap.loans.debits[dyadic_slot]
        .as_mut()
        .ok_or("dyadic debit after refusal")?
        .owner = crate::loan::registry::DebitOwner::Dyadic(dyadic_identity);
    assert!(dyadic_law.authority().is_ok());

    let mut expm1_heap = crate::Heap::new(HeapLimits::default())?;
    let mut expm1_account =
        crate::ExecutionAccount::new(Budget::new(super::super::retained_expm1::tests::limits())?)?;
    let expm1_law =
        super::super::retained_expm1::tests::prepare(&mut expm1_heap, &mut expm1_account)?;
    let expm1_slot = expm1_heap
        .loans
        .debits
        .iter()
        .position(|entry| {
            matches!(
                entry,
                Some(crate::loan::registry::Debit {
                    kind: crate::loan::registry::AllocationKind::PreparedOwner { .. },
                    ..
                })
            )
        })
        .ok_or("live expm1 owner debit")?;
    let expm1_debit = expm1_heap.loans.debits[expm1_slot]
        .as_mut()
        .ok_or("expm1 debit")?;
    let crate::loan::registry::DebitOwner::Prepared(expm1_identity) = expm1_debit.owner else {
        return Err("expm1 owner identity".into());
    };
    expm1_debit.owner = crate::loan::registry::DebitOwner::Dyadic(expm1_identity);
    let expm1_bytes = expm1_heap.resource_bytes()?;
    assert!(matches!(
        expm1_heap.reconcile_detached(&mut expm1_account, 1000, None),
        crate::ScanOutcome::Refused(crate::LoanError::WrongLoan)
    ));
    assert_eq!(expm1_heap.resource_bytes()?, expm1_bytes);
    expm1_heap.loans.debits[expm1_slot]
        .as_mut()
        .ok_or("expm1 debit after refusal")?
        .owner = crate::loan::registry::DebitOwner::Prepared(expm1_identity);
    assert!(expm1_law.authority().is_ok());

    let (mut heap, mut account, law) = fixture::prepare()?;
    let extra = heap.retain_retained_dyadic_power(&law, &mut account)?;
    let identity = law.authority()?.judgment().request().target.clone();
    let mut foreign_account = crate::ExecutionAccount::new(Budget::new(fixture::limits())?)?;
    let (refusal, extra) = heap
        .release_retained_dyadic_power(extra, &mut foreign_account)
        .expect_err("foreign account must preserve the same use");
    assert!(matches!(refusal, crate::LoanError::ForeignAccount));
    assert_eq!(extra.authority()?.judgment().request().target, identity);
    heap.release_retained_dyadic_power(law, &mut account)
        .map_err(|(error, _)| error)?;
    assert_eq!(extra.authority()?.judgment().request().target, identity);
    drop(heap);
    drop(account);
    assert_eq!(
        extra
            .evaluate(1.0, &ArithmeticContext::new()?)?
            .reading()
            .endpoints,
        [1.0, 1.0]
    );
    let (mut heap, mut account, law) = fixture::prepare()?;
    drop(law);
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.residence(&mut account, 1000)?,
        crate::Residence::Empty { .. }
    ));
    Ok(())
}

/// A public second-round attempt keeps its first checked bound but refuses owner creation.
/// # Panics
/// Panics only if a genuine later Work stop is accepted or loses its returned graph.
fn public_stopped_preparation() -> TestResult {
    use nrz_bindings::DyadicPowerPreparationError;
    use nrz_bindings::readings::algebra::enclosure::{EnclosureOutcome, UndecidedReason};
    let (_, first_account, _) = fixture::prepare()?;
    let one_round_work = first_account.usage().work;
    let (cas, target, context) = fixture::original(1, 2)?;
    let mut heap = crate::Heap::new(HeapLimits::default())?;
    let mut account = crate::ExecutionAccount::new(Budget::new(nrz_bindings::Limits {
        work: one_round_work,
        ..fixture::limits()
    })?)?;
    let policy = nrz_bindings::readings::algebra::enclosure::RefinementPolicy {
        rounds: 2,
        ..fixture::policy()
    };
    let (error, returned) = heap
        .prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            [1.0, 2.0],
            policy,
            fixture::scalar_limits(),
            &ArithmeticContext::new()?,
            &mut |_| false,
            &mut account,
        )
        .expect_err("second refinement round exhausts original Work allowance");
    let crate::RetainedDyadicPowerError::Preparation(crate::PreparedDyadicPowerError::Bindings(
        DyadicPowerPreparationError::Stopped(report),
    )) = error
    else {
        return Err("public stopped preparation changed phase".into());
    };
    assert_eq!(
        report.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::WorkExhausted)
    );
    assert_eq!(report.completed_rounds, 1);
    assert!(report.checked.is_some());
    assert!(report.resource_fault.is_some());
    returned.expression(&target, &mut Budget::new(fixture::limits())?)?;
    assert!(heap.loans.debits.iter().all(Option::is_none));
    Ok(())
}

/// Check every acquired owner and additional-use allocation prefix and its unwind cleanup.
/// # Panics
/// Panics only if a typed fault, retained prefix, identity or cleanup inventory changes.
#[expect(
    clippy::too_many_lines,
    reason = "the selected lifecycle control keeps all allocation prefixes together"
)]
fn allocation_and_scan_prefixes() -> TestResult {
    use crate::loan::fault::Injection;
    for unwind in [false, true] {
        for prefix in 0..3 {
            let mut heap = crate::Heap::new(HeapLimits::default())?;
            let mut account = crate::ExecutionAccount::new(Budget::new(fixture::limits())?)?;
            let (cas, target, context) = fixture::original(1, 2)?;
            let initial = heap.resource_bytes()?;
            let injection = if unwind {
                Injection::unwinding_after(prefix)
            } else {
                Injection::after(prefix)
            };
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        drop(heap.prepare_retained_dyadic_power(
                            cas,
                            &target,
                            &context,
                            [1.0, 2.0],
                            fixture::policy(),
                            fixture::scalar_limits(),
                            &ArithmeticContext::new().expect("arithmetic fixture"),
                            &mut |_| false,
                            &mut account,
                        ));
                    }))
                    .is_err()
                );
            } else {
                let (error, returned) = heap
                    .prepare_retained_dyadic_power(
                        cas,
                        &target,
                        &context,
                        [1.0, 2.0],
                        fixture::policy(),
                        fixture::scalar_limits(),
                        &ArithmeticContext::new()?,
                        &mut |_| false,
                        &mut account,
                    )
                    .expect_err("constructor allocation prefix");
                assert!(matches!(
                    error,
                    crate::RetainedDyadicPowerError::Ownership(crate::LoanError::AllocationRefused)
                ));
                returned.expression(&target, &mut Budget::new(fixture::limits())?)?;
                returned.context_header(&context, &mut Budget::new(fixture::limits())?)?;
            }
            assert_eq!(Injection::attempts(), prefix + 1);
            assert_eq!(
                Injection::prefix(),
                match prefix {
                    0 => [0, 0, 0],
                    1 => [1, 1, 0],
                    _ => [2, 1, 1],
                }
            );
            assert!(heap.loans.debits.iter().all(Option::is_none));
            assert_eq!(
                heap.loans.debits.capacity(),
                if prefix == 0 { 0 } else { 2 }
            );
            assert_eq!(
                heap.resource_bytes()? - initial,
                if prefix == 0 {
                    0
                } else {
                    2 * bytes::headers()[2]
                }
            );
            drop(injection);
        }
        for prefix in 0..2 {
            let (mut heap, mut account, law) = fixture::prepare()?;
            let identity = law.authority()?.judgment().request().target.clone();
            let reading = law
                .evaluate(2.0, &ArithmeticContext::new()?)?
                .reading()
                .endpoints;
            let inventory = heap.retained_dyadic_power_inventory(&law, &mut account)?;
            let injection = if unwind {
                Injection::unwinding_after(prefix)
            } else {
                Injection::after(prefix)
            };
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        drop(heap.retain_retained_dyadic_power(&law, &mut account));
                    }))
                    .is_err()
                );
            } else {
                assert!(matches!(
                    heap.retain_retained_dyadic_power(&law, &mut account),
                    Err(crate::LoanError::AllocationRefused)
                ));
            }
            assert_eq!(Injection::attempts(), prefix + 1);
            assert_eq!(Injection::prefix(), [prefix, prefix, 0]);
            assert_eq!(heap.loans.debits.iter().flatten().count(), 2);
            drop(injection);
            assert_eq!(
                heap.retained_dyadic_power_inventory(&law, &mut account)?,
                inventory
            );
            assert_eq!(law.authority()?.judgment().request().target, identity);
            assert_eq!(
                law.evaluate(2.0, &ArithmeticContext::new()?)?
                    .reading()
                    .endpoints,
                reading
            );
        }
    }

    let (mut heap, mut account, law) = fixture::prepare()?;
    let owner = std::rc::Rc::downgrade(law.record.owner.as_ref().ok_or("owner edge")?);
    let use_record = std::rc::Rc::downgrade(&law.record);
    let occupied = heap.resource_bytes()?;
    drop(law);
    assert!(owner.upgrade().is_none() && use_record.upgrade().is_none());
    assert_eq!(heap.resource_bytes()?, occupied);
    assert!(matches!(
        heap.residence(&mut account, 0)?,
        crate::Residence::Pending { .. }
    ));
    let crate::ScanOutcome::Pending(cursor) = heap.reconcile_detached(&mut account, 1, None) else {
        return Err("first weak debit leaves a suffix".into());
    };
    assert_eq!(heap.loans.debits.iter().flatten().count(), 1);
    assert!(matches!(
        heap.residence(&mut account, 0)?,
        crate::Residence::Pending { .. }
    ));
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1, Some(cursor)),
        crate::ScanOutcome::Complete
    ));
    assert!(heap.loans.debits.iter().all(Option::is_none));
    let empty = heap.resource_bytes()?;
    let before = account.usage().work;
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1000, None),
        crate::ScanOutcome::Complete
    ));
    assert_eq!(heap.resource_bytes()?, empty);
    assert_eq!(
        account.usage().work - before,
        u64::try_from(heap.loans.debits.len())?
    );
    let before = account.usage().work;
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1000, None),
        crate::ScanOutcome::Complete
    ));
    assert_eq!(heap.resource_bytes()?, empty);
    assert_eq!(heap.loans.debits.iter().flatten().count(), 0);
    assert_eq!(
        account.usage().work - before,
        u64::try_from(heap.loans.debits.len())?
    );
    assert!(matches!(
        heap.residence(&mut account, 1000)?,
        crate::Residence::Empty { .. }
    ));

    let (mut heap, mut account, law) = fixture::prepare()?;
    let identity = law.authority()?.judgment().request().target.clone();
    heap.loans.revision = u64::MAX;
    assert!(matches!(
        heap.retain_retained_dyadic_power(&law, &mut account),
        Err(crate::LoanError::RevisionCapacity)
    ));
    assert_eq!(heap.loans.debits.iter().flatten().count(), 2);
    assert_eq!(law.authority()?.judgment().request().target, identity);
    scan_work_boundaries()?;
    Ok(())
}

/// Two expired debit visits cost scan plus authenticated removal, then residence inventory.
/// # Panics
/// Panics only if exact or one-below Work changes a suffix, residence, or consumed prefix.
fn scan_work_boundaries() -> TestResult {
    use nrz_bindings::readings::algebra::Location;
    /// Fixed location for the caller's Work reservation.
    const AT: Location = Location::new("retained dyadic scan Work fixture");
    for short in [0, 1] {
        let (mut heap, mut account, law) = fixture::prepare()?;
        drop(law);
        assert_eq!(heap.loans.debits.len(), 2);
        let needed = u64::try_from(heap.loans.debits.len() * 2)?;
        assert_eq!(needed, 4);
        let remaining = needed - short;
        let prior = account.limits().work - account.usage().work - remaining;
        account.ledger.work(prior, AT)?;
        let before = account.usage().work;
        let result = heap.reconcile_detached(&mut account, 2, None);
        if short == 0 {
            assert!(matches!(result, crate::ScanOutcome::Complete));
            assert!(heap.loans.debits.iter().all(Option::is_none));
        } else {
            assert!(matches!(result, crate::ScanOutcome::WorkExhausted(_)));
            assert!(heap.loans.debits[0].is_none() && heap.loans.debits[1].is_some());
        }
        assert_eq!(account.usage().work - before, remaining);
    }
    for short in [0, 1] {
        let (mut heap, mut account, law) = fixture::prepare()?;
        drop(law);
        assert_eq!(heap.loans.groups.len(), 0);
        assert_eq!(heap.loans.pins.len(), 0);
        assert_eq!(heap.loans.debits.len(), 2);
        let debit_slots = heap.loans.debits.len();
        assert_eq!(heap.resource_work()?, 2 * debit_slots);
        let reconciliation = 2 * 2;
        let vacant_reclaim = heap.loans.groups.len();
        assert_eq!(vacant_reclaim, 0);
        let inventory = 2 * debit_slots + debit_slots;
        assert_eq!(inventory, 6);
        let needed = u64::try_from(reconciliation + vacant_reclaim + inventory)?;
        let remaining = needed - short;
        let prior = account.limits().work - account.usage().work - remaining;
        account.ledger.work(prior, AT)?;
        let before = account.usage().work;
        let outcome = heap.residence(&mut account, 2)?;
        assert_eq!(
            matches!(outcome, crate::Residence::Empty { .. }),
            short == 0
        );
        assert!(heap.loans.debits.iter().all(Option::is_none));
        assert_eq!(
            account.usage().work - before,
            if short == 0 {
                needed
            } else {
                needed - u64::try_from(inventory)?
            }
        );
    }
    Ok(())
}

/// Native reuse borrows the captured program and performs no new exact preparation.
/// # Panics
/// Panics only if repeated native calls change exact account usage or owner identity.
/// ´claim:matrix:retained-dyadic-power-observation´
/// ´test:unit:retained-dyadic-power-observation´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one selected observation control compares all attempt phases"
)]
fn retained_dyadic_power_observation() -> TestResult {
    use crate::loan::fault::Injection;
    let observer = observation::OwnerObserver::default();
    crate::observer_dispatch::with_observer(observer.clone(), || -> TestResult {
        tracing::callsite::rebuild_interest_cache();
        let (mut heap, mut account, law) = fixture::prepare()?;
        let cold_total = account.usage().work;
        assert_eq!(cold_total, 127_549);
        let cold_span = observer.spans("nrz.prepare")[0];
        assert_eq!(
            observer.field(cold_span, "result").as_deref(),
            Some("\"accepted\"")
        );
        assert_eq!(
            observer.field(cold_span, "owner_bytes").as_deref(),
            Some("1808")
        );
        let cold_lower = observer
            .spans("nrz.lower")
            .into_iter()
            .find(|id| observer.field(*id, "proof_bytes").is_some())
            .ok_or("cold matrix lower span")?;
        let cold_lower_work: u64 = observer
            .field(cold_lower, "work_used")
            .ok_or("cold lower Work")?
            .parse()?;
        let cold_binding_work: u64 = observer
            .field(cold_span, "bindings_work")
            .ok_or("cold bindings Work")?
            .parse()?;
        assert!(cold_binding_work > 0 && cold_binding_work < cold_lower_work);
        let arithmetic = ArithmeticContext::new()?;
        let before = account.usage().work;
        let events = observer.work_events();
        let first = law.evaluate(1.0, &arithmetic)?.reading();
        let second = law.evaluate(1.0, &arithmetic)?.reading();
        assert_eq!(first.endpoints, second.endpoints);
        assert_eq!(account.usage().work, before);
        assert_eq!(observer.work_events(), events);
        let extra = heap.retain_retained_dyadic_power(&law, &mut account)?;
        let before = account.usage().work;
        let events = observer.work_events();
        assert_eq!(
            extra
                .evaluate(2.0, &arithmetic)?
                .authority()
                .judgment()
                .request()
                .target,
            law.authority()?.judgment().request().target
        );
        assert_eq!(account.usage().work, before);
        assert_eq!(observer.work_events(), events);
        heap.release_retained_dyadic_power(extra, &mut account)
            .map_err(|(error, _)| error)?;
        let inventory = heap.retained_dyadic_power_inventory(&law, &mut account)?;
        assert_eq!(inventory.uses, 2);
        let mut warm_heap = crate::Heap::new(HeapLimits::default())?;
        let mut warm_account = crate::ExecutionAccount::new(Budget::new(fixture::limits())?)?;
        let (mut seed_cas, seed_target, seed_context) = fixture::original(1, 2)?;
        let mut seed_budget = Budget::new(fixture::limits())?;
        let mut seed_work = 0;
        let seed = nrz_bindings::DyadicPowerPreparation::prepare(
            &mut seed_cas,
            &seed_target,
            &seed_context,
            [1.0, 2.0],
            fixture::policy(),
            &mut seed_budget,
            &mut |_| false,
            &mut seed_work,
        )?;
        assert_eq!(seed.retained_bytes(), 2549);
        let seed_total = seed_budget.usage().work;
        let seed_judgments = seed_cas.retained_judgment_bytes();
        let mut repeated_work = 0;
        let repeated = nrz_bindings::DyadicPowerPreparation::prepare(
            &mut seed_cas,
            &seed_target,
            &seed_context,
            [1.0, 2.0],
            fixture::policy(),
            &mut seed_budget,
            &mut |_| false,
            &mut repeated_work,
        )?;
        assert_eq!(repeated.retained_bytes(), seed.retained_bytes());
        assert_eq!(seed_cas.retained_judgment_bytes(), seed_judgments);
        assert_eq!(seed_budget.usage().work - seed_total, repeated_work);
        let warm = fixture::prepare_on(&mut warm_heap, &mut warm_account, true)?;
        assert_eq!(warm.authority()?.retained_bytes(), 2549);
        assert_eq!(warm_account.usage().work, 169_039);
        let warm_delta = warm_account.usage().work - seed_total;
        let warm_span = *observer.spans("nrz.prepare").last().ok_or("warm span")?;
        assert_eq!(
            observer.field(warm_span, "result").as_deref(),
            Some("\"accepted\"")
        );
        assert!(warm_delta > 0 && warm_delta < cold_total);
        let warm_lower = observer
            .spans("nrz.lower")
            .into_iter()
            .rfind(|id| observer.field(*id, "proof_bytes").is_some())
            .ok_or("warm matrix lower span")?;
        let warm_lower_work: u64 = observer
            .field(warm_lower, "work_used")
            .ok_or("warm lower Work")?
            .parse()?;
        let warm_binding_work: u64 = observer
            .field(warm_span, "bindings_work")
            .ok_or("warm bindings Work")?
            .parse()?;
        assert!(warm_binding_work > 0 && warm_binding_work < warm_lower_work);
        assert_eq!(warm_binding_work, repeated_work);
        assert_eq!(warm_delta - warm_lower_work, cold_total - cold_lower_work);
        assert!(seed_cas.retained_judgment_bytes() > 0);

        let (cas, target, context) = fixture::original(0, 1)?;
        let mut refused_heap = crate::Heap::new(HeapLimits::default())?;
        let mut refused_account = crate::ExecutionAccount::new(Budget::new(fixture::limits())?)?;
        let (refusal, returned) = refused_heap
            .prepare_retained_dyadic_power(
                cas,
                &target,
                &context,
                [1.0, 2.0],
                fixture::policy(),
                fixture::scalar_limits(),
                &arithmetic,
                &mut |_| false,
                &mut refused_account,
            )
            .expect_err("zero exponent refuses before owner admission");
        assert!(matches!(
            refusal,
            crate::RetainedDyadicPowerError::Preparation(crate::PreparedDyadicPowerError::Native(
                DyadicPowerFailure::OutsideDomain
            ))
        ));
        returned.expression(&target, &mut Budget::new(fixture::limits())?)?;
        let refused_span = *observer.spans("nrz.prepare").last().ok_or("refused span")?;
        assert_eq!(
            observer.field(refused_span, "result").as_deref(),
            Some("\"preparation-refused\"")
        );
        assert!(observer.field(refused_span, "owner_bytes").is_none());
        assert_eq!(
            observer.field(refused_span, "bindings_work").as_deref(),
            Some("0")
        );
        let (refusal_oracle, oracle_target, oracle_context) = fixture::original(0, 1)?;
        let mut oracle_budget = Budget::new(fixture::limits())?;
        assert!(matches!(
            nrz_bindings::DyadicPowerPreparation::original_exponent(
                &refusal_oracle,
                &oracle_target,
                &oracle_context,
                &mut oracle_budget,
            ),
            Err(nrz_bindings::DyadicPowerPreparationError::ExponentOutsideDomain)
        ));
        assert_eq!(refused_account.usage().work, oracle_budget.usage().work);
        let refused_lower = *observer
            .spans("nrz.lower")
            .last()
            .ok_or("refused lower span")?;
        assert_eq!(
            observer.field(refused_lower, "result").as_deref(),
            Some("\"refused\"")
        );
        assert_eq!(
            observer
                .field(refused_lower, "work_used")
                .ok_or("refusal Work")?
                .parse::<u64>()?,
            oracle_budget.usage().work
        );
        assert!(oracle_budget.usage().work > 0);

        let (cas, target, context) = fixture::original(1, 2)?;
        let mut failed_heap = crate::Heap::new(HeapLimits::default())?;
        let mut failed_account = crate::ExecutionAccount::new(Budget::new(fixture::limits())?)?;
        let injection = Injection::after(0);
        let (failure, returned) = failed_heap
            .prepare_retained_dyadic_power(
                cas,
                &target,
                &context,
                [1.0, 2.0],
                fixture::policy(),
                fixture::scalar_limits(),
                &arithmetic,
                &mut |_| false,
                &mut failed_account,
            )
            .expect_err("table allocation refuses owner");
        assert!(matches!(
            failure,
            crate::RetainedDyadicPowerError::Ownership(crate::LoanError::AllocationRefused)
        ));
        returned.expression(&target, &mut Budget::new(fixture::limits())?)?;
        assert_eq!(Injection::attempts(), 1);
        assert!(failed_heap.loans.debits.iter().all(Option::is_none));
        let failed_span = *observer.spans("nrz.prepare").last().ok_or("failed span")?;
        assert_eq!(
            observer.field(failed_span, "result").as_deref(),
            Some("\"ownership-refused\"")
        );
        assert!(observer.field(failed_span, "owner_bytes").is_none());
        assert_eq!(
            observer
                .field(failed_span, "bindings_work")
                .ok_or("failed bindings Work")?
                .parse::<u64>()?,
            cold_binding_work
        );
        let failed_lower = observer
            .spans("nrz.lower")
            .into_iter()
            .rfind(|id| observer.field(*id, "proof_bytes").is_some())
            .ok_or("rolled-back lower span")?;
        assert_eq!(
            observer
                .field(failed_lower, "work_used")
                .ok_or("rolled-back lower Work")?
                .parse::<u64>()?,
            cold_lower_work
        );
        assert_eq!(failed_account.usage().work, cold_total);
        drop(injection);
        Ok(())
    })?;
    assert_eq!(observer.spans("nrz.prepare").len(), 4);
    assert_eq!(observer.spans("nrz.retain").len(), 1);
    assert_eq!(observer.spans("nrz.release").len(), 1);
    assert_eq!(observer.spans("nrz.kernel").len(), 3);
    for id in observer.spans("nrz.prepare").into_iter().take(2) {
        assert_eq!(
            observer.field(id, "result").as_deref(),
            Some("\"accepted\"")
        );
        assert_eq!(observer.field(id, "owner_bytes").as_deref(), Some("1808"));
        assert_eq!(
            observer.field(id, "evidence_bytes").as_deref(),
            Some("2549")
        );
        assert_eq!(observer.field(id, "key_bytes").as_deref(), Some("3474"));
        assert_eq!(observer.field(id, "use_bytes").as_deref(), Some("160"));
    }
    let matrix_lower: Vec<_> = observer
        .spans("nrz.lower")
        .into_iter()
        .filter(|id| observer.field(*id, "proof_bytes").is_some())
        .collect();
    assert_eq!(matrix_lower.len(), 3);
    for id in matrix_lower {
        assert_eq!(
            observer.field(id, "result").as_deref(),
            Some("\"accepted\"")
        );
    }
    let id = observer.spans("nrz.retain")[0];
    assert_eq!(
        observer.field(id, "result").as_deref(),
        Some("\"accepted\"")
    );
    let id = observer.spans("nrz.release")[0];
    assert_eq!(
        observer.field(id, "result").as_deref(),
        Some("\"released\"")
    );
    Ok(())
}
