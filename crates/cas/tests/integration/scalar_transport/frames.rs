// SPDX-License-Identifier: AGPL-3.0-only
//! Well-formed context retargeting remains distinct from wrong-category wire damage.

use crate::testing::answer::{index, records};
use crate::testing::budgets::proof_budget as budget;
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::scalar_transport::{Chain, chain, meet};
use nrz_cas::domain::{AssumptionDraft, ContextDraft};
use nrz_cas::syntax::{Draft, PredicateKind};
use nrz_cas::{FaultKind, Handle, InvalidCode, Reference};

fn fixture(reordered: bool) -> TestResult<(Chain, Handle)> {
    let mut f = chain(reordered, 3)?;
    let predicate = f
        .fixture
        .predicate(PredicateKind::Defined, &[f.fixture.one.clone()])?;
    let old = f.frames.clone();
    for level in 1..4 {
        f.frames[level] = admit(
            &mut f.fixture.cas,
            Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(f.frames[level - 1].clone()),
                region: Reference::Existing(f.restriction.clone()),
                hypotheses: vec![AssumptionDraft {
                    key: format!("frame-{level}"),
                    condition: Reference::Existing(predicate.clone()),
                }],
            }),
            &mut budget()?,
        )?;
    }
    for step in &mut f.steps {
        let level = old
            .iter()
            .position(|frame| frame == &step.context)
            .ok_or("step context")?;
        step.context = f.frames[level].clone();
    }
    f.expected.context = f.frames[3].clone();
    for residual in &mut f.expected.residuals {
        residual.home = f.frames[3].clone();
    }
    Ok((f, predicate))
}

fn meet_record(
    bytes: &[u8],
    inventory: &[(u8, std::ops::Range<usize>)],
    a: u32,
    d: u32,
) -> TestResult<u32> {
    let mut members = [a, d];
    members.sort_unstable();
    let mut suffix = vec![4, 2, 0, 0, 0];
    for member in members {
        suffix.extend_from_slice(&member.to_le_bytes());
    }
    suffix.push(0);
    let (r, _) = inventory
        .iter()
        .enumerate()
        .find(|(_, (tag, range))| *tag == 1 && bytes[range.clone()].ends_with(&suffix))
        .ok_or("transported region record")?;
    Ok(u32::try_from(r)?)
}

fn retarget(bytes: &[u8], restriction: bool) -> TestResult<Vec<u8>> {
    let inventory = records(bytes)?;
    let contexts: Vec<_> = inventory
        .iter()
        .enumerate()
        .filter(|(_, (tag, _))| *tag == 8)
        .collect();
    assert_eq!(contexts.len(), 4);
    let (_, (_, root)) = contexts[0];
    assert_eq!(&bytes[root.start..root.start + 6], &[1, 0, 0, 0, 0, 0]);
    assert_eq!(root.len(), 20);
    assert_eq!(bytes[root.start + 10], 0);
    assert_eq!(bytes[root.start + 15], 0);
    let a = index(bytes, &mut (root.start + 11))?;
    let (_, (_, first)) = contexts[2];
    let (target, (_, second)) = contexts[3];
    for (level, (_, (_, frame))) in contexts[1..].iter().enumerate() {
        assert_eq!(&bytes[frame.start..frame.start + 6], &[1, 0, 0, 0, 0, 0]);
        assert_eq!(frame.len(), 43);
        assert_eq!(bytes[frame.start + 10], 1);
        assert_eq!(bytes[frame.start + 19], 1);
        assert_eq!(index(bytes, &mut (frame.start + 24))?, 1);
        assert_eq!(index(bytes, &mut (frame.start + 28))?, 7);
        assert_eq!(
            &bytes[frame.start + 32..frame.start + 39],
            format!("frame-{}", level + 1).as_bytes()
        );
    }
    let d = index(bytes, &mut (first.start + 15))?;
    assert_eq!(
        index(bytes, &mut (second.start + 11))?,
        u32::try_from(contexts[2].0)?
    );
    let original_restriction = index(bytes, &mut (second.start + 20))?;
    assert_eq!(inventory[usize::try_from(original_restriction)?].0, 1);
    let (offset, replacement, admissible) = if restriction {
        let first_domain = index(bytes, &mut (contexts[1].1.1.start + 15))?;
        let r = meet_record(bytes, &inventory, a, first_domain)?;
        let domain = meet_record(bytes, &inventory, r, d)?;
        assert!(usize::try_from(domain)? < target);
        (second.start + 20, r, domain)
    } else {
        (second.start + 11, u32::try_from(contexts[1].0)?, d)
    };
    assert!(usize::try_from(replacement)? < target);
    assert_eq!(
        inventory[usize::try_from(replacement)?].0,
        if restriction { 1 } else { 8 }
    );
    assert_eq!(inventory[usize::try_from(admissible)?].0, 1);
    let mut changed = bytes.to_vec();
    changed[offset..offset + 4].copy_from_slice(&replacement.to_le_bytes());
    // Retain the actual reconstructed admissible region, so context admission stays well formed.
    changed[second.start + 15..second.start + 19].copy_from_slice(&admissible.to_le_bytes());
    assert_eq!(records(&changed)?, inventory);
    Ok(changed)
}

/// Valid parent and restriction targets cannot preserve the original transported theorem.
/// ´test:integration:scalar-transport-wire-frames´
#[test]
fn scalar_transport_wire_frames() -> TestResult {
    let (mut source, _) = fixture(false)?;
    let witness =
        source
            .fixture
            .cas
            .check_proof(&source.frames[3], &source.steps, &mut budget()?)?;
    let bytes = source
        .fixture
        .cas
        .export_witness(&witness, &mut budget()?)?;
    for restriction in [false, true] {
        let changed = retarget(&bytes, restriction)?;
        for reordered in [false, true] {
            let (mut receiver, predicate) = fixture(reordered)?;
            let a = receiver.fixture.domain.clone();
            let d = receiver.fixture.cas.admissible_region(
                &receiver.frames[2],
                &mut crate::testing::budgets::graph_budget()?,
            )?;
            let first = receiver.fixture.cas.admissible_region(
                &receiver.frames[1],
                &mut crate::testing::budgets::graph_budget()?,
            )?;
            let transported = meet(&mut receiver.fixture, &[a, first])?;
            let expected_domain = if restriction {
                meet(&mut receiver.fixture, &[transported.clone(), d])?
            } else {
                d
            };
            // Independently admit exactly the modified edge, including its reconstructed domain.
            let alternate = admit(
                &mut receiver.fixture.cas,
                Draft::Context(ContextDraft::Local {
                    parent: Reference::Existing(
                        receiver.frames[1 + usize::from(restriction)].clone(),
                    ),
                    region: Reference::Existing(if restriction {
                        transported
                    } else {
                        receiver.restriction.clone()
                    }),
                    hypotheses: vec![AssumptionDraft {
                        key: "frame-3".into(),
                        condition: Reference::Existing(predicate),
                    }],
                }),
                &mut budget()?,
            )?;
            assert_eq!(
                receiver
                    .fixture
                    .cas
                    .admissible_region(&alternate, &mut crate::testing::budgets::graph_budget()?)?,
                expected_domain
            );
            let before = receiver.fixture.cas.canonical_snapshot(&mut budget()?)?;
            let fault = receiver
                .fixture
                .cas
                .replay(&changed, &receiver.expected, &mut budget()?)
                .expect_err("retargeted context");
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::Context),
                "{fault:?}"
            );
            assert_eq!(fault.location.operation, "context theorem", "{fault:?}");
            assert_eq!(
                receiver.fixture.cas.canonical_snapshot(&mut budget()?)?,
                before
            );
            let intact = receiver
                .fixture
                .cas
                .replay(&bytes, &receiver.expected, &mut budget()?)?;
            assert_eq!(intact.judgment(), &receiver.expected);
            assert_eq!(
                receiver
                    .fixture
                    .cas
                    .export_witness(&intact, &mut budget()?)?,
                bytes
            );
        }
    }
    Ok(())
}
