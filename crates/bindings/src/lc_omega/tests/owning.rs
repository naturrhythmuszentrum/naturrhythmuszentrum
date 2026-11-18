// SPDX-License-Identifier: AGPL-3.0-only
//! Independent owner inventories, separate from the unchanged borrowed-view pins.
use super::*;
use std::cell::Cell;

/// Lend fixture rows through the future bridge's indexed interface.
fn request<'a>(
    original: LcOmegaOriginalsRequest<'a>,
) -> LcOmegaDeclarationRequest<'a, impl Fn(usize) -> Option<&'a [BindingSymbols; 2]>> {
    LcOmegaDeclarationRequest {
        question: original.question,
        input_len: original.inputs.len(),
        input_at: move |index| original.inputs.get(index),
        positions: original.positions,
        counts: original.counts,
        output: original.output,
        radian: original.radian,
        registry: original.registry,
    }
}

/// Exercise owning admission, not merely its borrowed validation component.
pub(super) fn admit(
    original: LcOmegaOriginalsRequest<'_>,
    budget: &mut Budget,
) -> Result<LcOmegaDeclarationToken, AdmissionError> {
    LcOmegaDeclarationToken::admit(&request(original), budget)
}

/// Owning identity is independent of structural equality; capture is single-pass.
pub(super) fn identities() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let f = fixture(&mut budget)?;
    let visits = Cell::new([0; 5]);
    let proposed = LcOmegaDeclarationRequest {
        question: &f.question,
        input_len: 5,
        positions: [0, 1],
        counts: [1, 1, 1],
        output: &f.output,
        radian: &f.radian,
        registry: &f.registry,
        input_at: |index| {
            let mut counts = visits.get();
            counts[index] += 1;
            visits.set(counts);
            f.inputs.get(index)
        },
    };
    let token = LcOmegaDeclarationToken::admit(&proposed, &mut budget)?;
    assert_eq!(visits.get(), [1; 5]);
    token.check_originals(&request(f.request()), &mut budget)?;
    let shared = token.share(&mut budget)?;
    token.check_same(&shared, &mut budget)?;
    let separate = admit(f.request(), &mut budget)?;
    assert_eq!(
        token.check_same(&separate, &mut budget),
        Err(AdmissionError::ValueMismatch)
    );
    let foreign = fixture(&mut budget)?;
    assert_eq!(
        token.check_originals(&request(foreign.request()), &mut budget),
        Err(AdmissionError::ValueMismatch)
    );
    let mut changed = f.request();
    changed.output = &foreign.output;
    assert_eq!(
        token.check_originals(&request(changed), &mut budget),
        Err(AdmissionError::ResultMismatch)
    );
    let mut swapped = f.inputs.clone();
    swapped.swap(0, 3);
    let mut changed = f.request();
    changed.inputs = &swapped;
    assert_eq!(
        token.check_originals(&request(changed), &mut budget),
        Err(AdmissionError::ValueMismatch)
    );
    assert_eq!(
        format!("{token:?}"),
        "LcOmegaDeclarationToken { positions: [0, 1], counts: [1, 1, 1], .. }"
    );
    aliases(&f, &mut budget);
    Ok(())
}

/// Positional and channel aliasing both refuse before token publication.
fn aliases(f: &Fixture, budget: &mut Budget) {
    let mut positional = f.inputs.clone();
    positional[3] = positional[0].clone();
    let mut channel = f.inputs.clone();
    channel[0][1] = channel[0][0].clone();
    for inputs in [positional, channel] {
        let mut proposed = f.request();
        proposed.inputs = &inputs;
        assert!(matches!(
            admit(proposed, budget),
            Err(AdmissionError::ValueMismatch)
        ));
    }
    let mut proposed = f.request();
    proposed.output = &f.inputs[0][0];
    assert!(matches!(
        admit(proposed, budget),
        Err(AdmissionError::ResultMismatch)
    ));
}

/// Missing rows and late physical refusal preserve owners and cumulative prefixes.
pub(super) fn failures() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let f = fixture(&mut budget)?;
    let before = budget.usage().work;
    let proposed = LcOmegaDeclarationRequest {
        question: &f.question,
        input_len: 5,
        positions: [0, 1],
        counts: [1, 1, 1],
        output: &f.output,
        radian: &f.radian,
        registry: &f.registry,
        input_at: |index| {
            if index == 2 {
                None
            } else {
                f.inputs.get(index)
            }
        },
    };
    assert!(matches!(
        LcOmegaDeclarationToken::admit(&proposed, &mut budget),
        Err(AdmissionError::ValueMismatch)
    ));
    // Preflight1 + allocation2 + edges10 + evidence4 + two rows26 + missing access1.
    assert_eq!(budget.usage().work, before + 44);
    let wrong_registry = Registry::new(ExponentPolicy::Rational);
    let mut proposed = f.request();
    proposed.registry = &wrong_registry;
    let before = budget.usage().work;
    assert!(matches!(
        admit(proposed, &mut budget),
        Err(AdmissionError::Units(_))
    ));
    // Owning prefix137 followed by physical admission through registry visit873.
    assert_eq!(budget.usage().work, before + 1010);
    let token = admit(f.request(), &mut budget)?;
    token.check_originals(&request(f.request()), &mut budget)?;
    Ok(())
}

/// Paper inventory: 1+2+10+4+65+55+876+2+2 Work; 160+16+8+240 retained bytes.
pub(super) fn boundaries() -> TestResult {
    let mut preparation = Budget::new(Limits::default())?;
    let f = fixture(&mut preparation)?;
    for ceiling in [1024, 1023] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        budget.work(7, AT)?;
        let result = admit(f.request(), &mut budget);
        if ceiling == 1024 {
            result?;
            assert_eq!(budget.usage().work, 1024);
        } else {
            exhausted(result, BudgetKind::Work, 1023, 1024)?;
            assert_eq!(budget.usage().work, 1022);
        }
    }
    admission_bytes(&f)?;
    let token = admit(f.request(), &mut preparation)?;
    comparison(&f, &token)?;
    sharing(&token)
}

/// Existing caller residence coexists with new owner and checked-view scratch.
fn admission_bytes(f: &Fixture) -> TestResult {
    for (kind, exact) in [
        (BudgetKind::OutputBytes, 424),
        (BudgetKind::StagedPayloadBytes, 541),
    ] {
        for ceiling in [exact, exact - 1] {
            let mut limits = Limits::default();
            if kind == BudgetKind::OutputBytes {
                limits.output_bytes = ceiling;
            } else {
                limits.staged_payload_bytes = ceiling;
            }
            let mut budget = Budget::new(limits)?;
            budget.work(7, AT)?;
            let result = budget.with_retained_payload(37, AT, |budget| admit(f.request(), budget));
            if ceiling == exact {
                result?;
                assert_eq!(budget.usage().work, 1024);
            } else {
                let (limit, needed) = if kind == BudgetKind::OutputBytes {
                    (423, 424)
                } else {
                    (503, 504)
                };
                exhausted(result, kind, limit, needed)?;
                assert_eq!(budget.usage().work, 8);
            }
            if kind == BudgetKind::StagedPayloadBytes {
                assert_eq!(budget.limits().staged_payload_bytes, ceiling);
            }
        }
    }
    Ok(())
}

/// Indexed comparison costs 1+5+33+1 and reserves the owner increment once.
fn comparison(f: &Fixture, token: &LcOmegaDeclarationToken) -> TestResult {
    for ceiling in [47, 46] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        budget.work(7, AT)?;
        let result = token.check_originals(&request(f.request()), &mut budget);
        if ceiling == 47 {
            result?;
            assert_eq!(budget.usage().work, 47);
        } else {
            exhausted(result, BudgetKind::Work, 46, 47)?;
            assert_eq!(budget.usage().work, 46);
        }
    }
    for ceiling in [461, 460] {
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: ceiling,
            ..Limits::default()
        })?;
        budget.work(7, AT)?;
        let result = budget.with_retained_payload(37, AT, |budget| {
            token.check_originals(&request(f.request()), budget)
        });
        if ceiling == 461 {
            result?;
        } else {
            exhausted(result, BudgetKind::StagedPayloadBytes, 423, 424)?;
        }
        assert_eq!(budget.limits().staged_payload_bytes, ceiling);
    }
    for ceiling in [8, 7] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        budget.work(7, AT)?;
        let result = token.check_same(token, &mut budget);
        if ceiling == 8 {
            result?;
        } else {
            exhausted(result, BudgetKind::Work, 7, 8)?;
        }
    }
    Ok(())
}

/// One eight-byte token and two prepaid edge events; no duplicate owner allocation.
fn sharing(token: &LcOmegaDeclarationToken) -> TestResult {
    for ceiling in [9, 8] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        budget.work(7, AT)?;
        let result = token.share(&mut budget);
        if ceiling == 9 {
            result?;
            assert_eq!(budget.usage().work, 9);
        } else {
            exhausted(result, BudgetKind::Work, 8, 9)?;
            assert_eq!(budget.usage().work, 7);
        }
    }
    for (kind, exact) in [
        (BudgetKind::OutputBytes, 8),
        (BudgetKind::StagedPayloadBytes, 469),
    ] {
        for ceiling in [exact, exact - 1] {
            let mut limits = Limits::default();
            if kind == BudgetKind::OutputBytes {
                limits.output_bytes = ceiling;
            } else {
                limits.staged_payload_bytes = ceiling;
            }
            let mut budget = Budget::new(limits)?;
            budget.work(7, AT)?;
            let result = budget.with_retained_payload(37, AT, |budget| token.share(budget));
            if ceiling == exact {
                result?;
                assert_eq!(budget.usage().work, 9);
            } else {
                exhausted(result, kind, 7, 8)?;
                assert_eq!(budget.usage().work, 7);
            }
            if kind == BudgetKind::StagedPayloadBytes {
                assert_eq!(budget.limits().staged_payload_bytes, ceiling);
            }
        }
    }
    Ok(())
}
