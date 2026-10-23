// SPDX-License-Identifier: AGPL-3.0-only
//! Specialized reading cases and resource boundaries for matrix-private adapter tests.

pub(crate) use crate::reading_testing::{Fixture, TestResult, ledger, map, matrix, meaning, real};
use nrz_bindings::Limits;
use nrz_bindings::readings::algebra::{
    readings::{AffineMap, EntryIndex, SelectionSpec},
    syntax::{AtomicSort, Field, Sort},
};
use nrz_bindings::readings::{CoordinateFamily, PhysicalReading};

/// A whole reading and an independently defined component of the same family.
pub(crate) struct Case {
    /// Complete independent graph and context.
    pub(crate) fixture: Fixture,
    /// The whole state or Complex interpretation.
    pub(crate) whole: PhysicalReading,
    /// A Real component reading of specified source entries.
    pub(crate) component: PhysicalReading,
}
/// Mixed volts and amperes use one ordered two-coordinate family.
pub(crate) fn state_case() -> TestResult<Case> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("v", real()),
        ("i", real()),
    ])?;
    let entries = vec![
        meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?,
        meaning("current", [0, 0, 0, 1, 0, 0, 0])?,
    ];
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        entries.clone(),
        &mut ledger()?,
    )?;
    let whole = fixture.define(
        &family,
        "x",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        entries.clone(),
    )?;
    let entry = entries.first().ok_or("component")?.clone();
    let selected = SelectionSpec::Entry(EntryIndex { row: 0, column: 0 });
    let component = fixture.define(
        &family,
        "v",
        selected,
        AffineMap::identity(1, &mut ledger()?)?,
        vec![entry],
    )?;
    Ok(Case {
        fixture,
        whole,
        component,
    })
}
/// The full block expansion retains the negative and repeated coordinates.
pub(crate) fn complex_case() -> TestResult<Case> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("block", matrix(2, 2)),
        ("c", Sort::Value(AtomicSort::Scalar(Field::Complex))),
        ("r", real()),
    ])?;
    let entry = meaning("impedance", [2, 1, -3, -2, 0, 0, 0])?;
    let family = {
        let family = CoordinateFamily::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("z")?,
            vec![entry.clone(); 2],
            &mut ledger()?,
        )?;
        let symbol = fixture.symbol("block")?;
        family.expand(
            &mut fixture.cas,
            &symbol,
            &map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0, 0, 0, 0])?,
            vec![entry.clone(); 4],
            &mut ledger()?,
        )?
    };
    let whole = fixture.define(
        &family,
        "c",
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0, 0])?,
        vec![entry.clone(); 2],
    )?;
    let component = fixture.define(
        &family,
        "r",
        SelectionSpec::Entry(EntryIndex { row: 1, column: 1 }),
        map(1, 1, &[1], &[0])?,
        vec![entry],
    )?;
    Ok(Case {
        fixture,
        whole,
        component,
    })
}

/// Check exact Work, cumulative precharge and staged-payload boundaries without raising a limit.
pub(crate) fn boundaries(mut attempt: impl FnMut(Limits, u64) -> TestResult<u64>) -> TestResult {
    let needed = attempt(Limits::default(), 0)?;
    assert!(needed > 0);
    assert!(
        attempt(
            Limits {
                work: needed,
                ..Limits::default()
            },
            0
        )
        .is_ok()
    );
    assert!(
        attempt(
            Limits {
                work: needed - 1,
                ..Limits::default()
            },
            0
        )
        .is_err()
    );
    assert!(
        attempt(
            Limits {
                work: needed,
                ..Limits::default()
            },
            1
        )
        .is_err()
    );
    assert!(
        attempt(
            Limits {
                staged_payload_bytes: 0,
                ..Limits::default()
            },
            0
        )
        .is_err()
    );
    let mut lower = 0;
    let mut upper = Limits::default().staged_payload_bytes;
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if attempt(
            Limits {
                staged_payload_bytes: middle,
                ..Limits::default()
            },
            0,
        )
        .is_ok()
        {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    attempt(
        Limits {
            staged_payload_bytes: upper,
            ..Limits::default()
        },
        0,
    )?;
    assert!(
        attempt(
            Limits {
                staged_payload_bytes: upper - 1,
                ..Limits::default()
            },
            0
        )
        .is_err()
    );
    Ok(())
}
