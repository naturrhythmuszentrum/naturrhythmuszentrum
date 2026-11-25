// SPDX-License-Identifier: AGPL-3.0-only
//! Production interning diagnostics across admission, lookup and transaction boundaries.

use crate::error::Location;
use crate::graph::{InterningSnapshot, LogicalId, Source, Vertex, vertex_key};
use crate::limits::{Budget, BudgetKind, Limits};
use crate::syntax::Draft;
use crate::testing::admission::Fixture;
use crate::testing::common::{TestResult, default_budget};
use crate::{Cas, Fault, FaultKind, InvalidCode};

/// Enumerate half-open midpoint probes over sorted plain keys, independent of the index.
/// Equality returns immediately; an absent probe returns its insertion position.
fn count_binary_probe(
    reference: &[(&[u8], LogicalId)],
    probe: &[u8],
    totals: &mut InterningSnapshot,
) -> TestResult<(usize, bool)> {
    let (mut low, mut high) = (0, reference.len());
    while low < high {
        let middle = low + (high - low) / 2;
        let candidate = reference[middle].0;
        let prefix = probe
            .iter()
            .zip(candidate.iter())
            .take_while(|(left, right)| left == right)
            .count();
        let compared = prefix + usize::from(prefix < probe.len().min(candidate.len()));
        totals.comparisons += 1;
        totals.compared_bytes += u64::try_from(compared)?;
        match probe.cmp(candidate) {
            std::cmp::Ordering::Less => high = middle,
            std::cmp::Ordering::Equal => return Ok((middle, true)),
            std::cmp::Ordering::Greater => low = middle + 1,
        }
    }
    Ok((low, false))
}

/// Equal-length names have a long shared prefix; equal keys also examine the payload tail.
fn source(name: &str) -> Draft {
    Draft::Source {
        key: format!("long-structural-{name}"),
        text: "shared payload".into(),
    }
}

/// Obtain the full keys without asking the interning index to compare anything.
fn keys(cas: &Cas, drafts: &[Draft]) -> TestResult<Vec<Vec<u8>>> {
    drafts
        .iter()
        .map(|draft| {
            let Draft::Source { key, text } = draft else {
                return Err("expected source draft".into());
            };
            let vertex = Vertex::Source(Source {
                key: key.clone(),
                text: text.clone(),
            });
            Ok(vertex_key(&vertex, &cas.state, &mut default_budget()?)?.1)
        })
        .collect()
}

/// Compare results with plain sorted keys, counting each midpoint probe independently.
fn measured_sequence(drafts: &[Draft], probes: &[usize]) -> TestResult<InterningSnapshot> {
    let mut cas = Cas::new(Limits::default())?;
    let encoded = keys(&cas, drafts)?;
    let mut totals = InterningSnapshot::default();
    let mut reference: Vec<(&[u8], LogicalId)> = Vec::new();
    for (draft, bytes) in drafts
        .iter()
        .zip(&encoded)
        .take(drafts.len().saturating_sub(1))
    {
        let handles = cas.admit(std::slice::from_ref(draft), &mut default_budget()?)?;
        let id = handles.first().ok_or("missing source")?.logical_id();
        let (position, found) = count_binary_probe(&reference, bytes, &mut totals)?;
        assert!(!found);
        reference.insert(position, (bytes.as_slice(), id));
        assert_eq!(cas.interning_snapshot(), totals);
    }
    for (position, index) in probes.iter().enumerate() {
        let bytes = encoded.get(*index).ok_or("missing probe")?;
        let actual = if position % 2 == 0 {
            cas.state.interned(bytes, &mut default_budget()?)?
        } else {
            cas.state
                .intern
                .get(
                    bytes,
                    0,
                    &mut default_budget()?,
                    Location::new("interning lookup"),
                )?
                .copied()
        };
        let (middle, found) = count_binary_probe(&reference, bytes, &mut totals)?;
        let expected = found.then(|| reference[middle].1);
        assert_eq!(actual, expected);
        assert_eq!(cas.interning_snapshot(), totals);
    }
    Ok(cas.interning_snapshot())
}

/// Both lookup and insertion match independent counts, including hits, misses and long prefixes.
/// ´test:crate:interning-comparisons-match-independent-keys´
#[test]
fn interning_comparisons_match_independent_keys() -> TestResult {
    let drafts = ["c", "a", "f", "b", "e", "d", "g"].map(source);
    // Six admissions take 9 comparisons and four probes take 10, for 19 total.
    // The independent byte comparator counts 472 compared byte pairs.
    let expected = InterningSnapshot {
        comparisons: 19,
        compared_bytes: 472,
    };
    assert_eq!(measured_sequence(&drafts, &[0, 1, 2, 6])?, expected);
    assert_eq!(measured_sequence(&drafts, &[0, 1, 2, 6])?, expected);
    Ok(())
}

/// Index growth crossings preserve lookup results and independent comparison totals.
/// ´test:crate:interning-comparisons-across-index-splits´
#[test]
fn interning_comparisons_across_index_splits() -> TestResult {
    let mut drafts: Vec<_> = (0..64)
        .map(|index| source(&format!("{:02}", (17 * index) % 64)))
        .collect();
    drafts.push(source("missing"));
    let probes: Vec<_> = (0..drafts.len()).collect();
    let expected = measured_sequence(&drafts, &probes)?;
    assert_eq!(measured_sequence(&drafts, &probes)?, expected);
    Ok(())
}

/// A nested committed insertion and its later rollback removal both remain counted.
/// ´test:crate:interning-comparisons-survive-outer-rollback´
#[test]
fn interning_comparisons_survive_outer_rollback() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let original = source("a");
    cas.admit(std::slice::from_ref(&original), &mut default_budget()?)?;
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let counts = cas.counts();
    let refusal = Fault::invalid(InvalidCode::Rule, Location::new("diagnostic rollback test"));
    // One 22-byte admission comparison survives; inverse removal examines 40 bytes.
    let result: Result<(), Fault> = cas.transaction(&mut default_budget()?, |cas, budget| {
        let draft = source("c");
        cas.admit(std::slice::from_ref(&draft), budget)?;
        assert_eq!(
            cas.interning_snapshot(),
            InterningSnapshot {
                comparisons: 1,
                compared_bytes: 22,
            }
        );
        Err(refusal.clone())
    });
    assert_eq!(result, Err(refusal));
    assert_eq!(
        cas.interning_snapshot(),
        InterningSnapshot {
            comparisons: 2,
            compared_bytes: 62,
        }
    );
    assert_eq!(cas.counts(), counts);
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}

/// Invalid public admission also restores graph state without refunding comparator work.
/// ´test:crate:interning-comparisons-survive-invalid-batch´
#[test]
fn interning_comparisons_survive_invalid_batch() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let original = source("a");
    cas.admit(std::slice::from_ref(&original), &mut default_budget()?)?;
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let drafts = [
        source("c"),
        Draft::Source {
            key: "long-structural-a".into(),
            text: "conflict".into(),
        },
    ];
    let fault = cas
        .admit(&drafts, &mut default_budget()?)
        .err()
        .ok_or("accepted conflict")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::SourceConflict));
    assert_eq!(
        cas.interning_snapshot(),
        InterningSnapshot {
            comparisons: 2,
            compared_bytes: 62,
        }
    );
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}

/// The existing lookup work charge still refuses before invoking the diagnostic comparator.
/// ´test:crate:interning-refusal-precedes-comparator´
#[test]
fn interning_refusal_precedes_comparator() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let draft = source("a");
    cas.admit(std::slice::from_ref(&draft), &mut default_budget()?)?;
    let encoded = keys(&cas, std::slice::from_ref(&draft))?;
    let key = encoded.first().ok_or("missing key")?;
    let before = cas.interning_snapshot();
    let mut limited = Budget::new(Limits {
        work: 0,
        ..Limits::default()
    })?;
    assert_eq!(
        cas.state.interned(key, &mut limited),
        Err(Fault::exhausted(
            BudgetKind::Work,
            0,
            u64::try_from(key.len())? + 1,
            Location::new("interning lookup"),
        ))
    );
    assert_eq!(cas.interning_snapshot(), before);
    Ok(())
}

/// Reading snapshots is observational: identities, physical order, bytes and charges agree.
/// ´test:crate:interning-snapshots-preserve-results-and-bytes´
#[test]
fn interning_snapshots_preserve_results_and_bytes() -> TestResult {
    let (mut observed, witness) = Fixture::with_witness()?;
    let (mut control, other_witness) = Fixture::with_witness()?;
    let mut observed_budget = default_budget()?;
    let mut control_budget = default_budget()?;
    let before = observed.cas.interning_snapshot();
    assert_eq!(observed.cas.interning_snapshot(), before);
    let drafts = observed.batch();
    let other = control.batch();
    let handles = observed.cas.admit(&drafts, &mut observed_budget)?;
    let expected = control.cas.admit(&other, &mut control_budget)?;
    let after = observed.cas.interning_snapshot();
    assert_eq!(observed.cas.interning_snapshot(), after);
    assert!(after.comparisons > before.comparisons);
    assert_eq!(
        handles
            .iter()
            .map(crate::Handle::logical_id)
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(crate::Handle::logical_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(observed_budget.usage(), control_budget.usage());
    assert_eq!(observed.cas.counts(), control.cas.counts());
    assert_eq!(observed.cas.state.indices, control.cas.state.indices);
    assert_eq!(
        format!("{:?}", observed.cas.state),
        format!("{:?}", control.cas.state)
    );
    assert_eq!(
        observed.cas.canonical_snapshot(&mut default_budget()?)?,
        control.cas.canonical_snapshot(&mut default_budget()?)?
    );
    assert_eq!(
        observed
            .cas
            .export_witness(&witness, &mut default_budget()?)?,
        control
            .cas
            .export_witness(&other_witness, &mut default_budget()?)?
    );
    assert_eq!(
        observed.cas.interning_snapshot(),
        control.cas.interning_snapshot()
    );
    Ok(())
}
