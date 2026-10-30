// SPDX-License-Identifier: AGPL-3.0-only
//! A transparent affine duration reading retains the original selected physical source.

use super::{
    Declaration, Draft, Frame, Handle, HeaderDraft, Operator, Rational, Reference, RegionExpr,
    SelectionFixture, TestResult, admit, cover_in, exact, ledger, real,
};
use nrz_bindings::readings::algebra::{
    decision::{IntegerCellRange, ScalarKernelFamily},
    readings::{AffineMap, EntryIndex, SelectionSpec},
    syntax::{AtomicSort, Extent, Field, Sort},
};
use nrz_bindings::{
    BaseDimension, Descriptor, Dimension, Kind, Measure, ReferenceDefinition, Role,
    readings::{CoordinateFamily, EntryMeaning, PhysicalReading, ReadingDefinition},
};

/// The source remains a literal one-entry Real matrix, distinct from its scalar reading declaration.
const fn matrix() -> Sort {
    Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(1),
        columns: Extent::Literal(1),
    })
}

/// Declare the complete physical source and result identities before the original floor relation.
fn frame() -> TestResult<Frame> {
    let mut frame = Frame::new()?;
    frame.header = admit(
        &mut frame.cas,
        Draft::Header(HeaderDraft::Extend {
            parent: Reference::Existing(frame.header.clone()),
            declarations: [("duration_input", matrix()), ("duration_reading", real())]
                .into_iter()
                .map(|(key, sort)| Declaration {
                    key: key.into(),
                    display_name: key.into(),
                    sort,
                })
                .collect(),
            spaces: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    frame.ambient = frame.region(RegionExpr::Ambient)?;
    Ok(frame)
}

/// Exact syntax precedes the context that assumes bounds and the original quotient correspondence.
struct Original {
    /// Complete free coordinate family.
    root: Handle,
    /// Declared scalar result without an equation of its own.
    symbol: Handle,
    /// Exact scalar entry whose whole family is covered.
    selected: Handle,
    /// Transparent affine equation retained by the target.
    definition: Handle,
    /// Declared exact scale and offset.
    map: AffineMap,
}

/// Define the original map without substituting its numerical center.
fn original(frame: &mut Frame) -> TestResult<Original> {
    let root = frame.term(Operator::Free("duration_input".into()), matrix(), &[])?;
    let symbol = frame.term(Operator::Free("duration_reading".into()), real(), &[])?;
    let selected = frame.cas.select_entries(
        &root,
        &SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
        &mut ledger()?,
    )?;
    let map = AffineMap::new(
        1,
        1,
        vec![Rational::from_ratio(2, 1, &mut ledger()?)?],
        vec![Rational::from_ratio(-1, 100, &mut ledger()?)?],
        &mut ledger()?,
    )?;
    let definition = frame
        .cas
        .define_reading(&symbol, &selected, &map, &mut ledger()?)?;
    Ok(Original {
        root,
        symbol,
        selected,
        definition,
        map,
    })
}

/// A coherent time coordinate uses one retained reference for both source and result meanings.
fn meaning() -> TestResult<EntryMeaning> {
    let descriptor = Descriptor::new(
        Kind::Named(
            "duration".into(),
            Box::new(Dimension::base(BaseDimension::Time)),
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    let reference = ReferenceDefinition::unit("duration reference", &descriptor)?;
    Ok(EntryMeaning::new(
        descriptor,
        reference,
        None,
        &mut ledger()?,
    )?)
}

/// The physical reading reuses the identical canonical definition under the complete source context.
pub(super) fn fixture() -> TestResult<(SelectionFixture, ScalarKernelFamily, PhysicalReading)> {
    let (mut frame, mut family) = cover_in(frame()?, false)?;
    family.count = Some(Box::new(IntegerCellRange {
        coordinate: family.coordinate.clone(),
        lower: Some(Rational::zero()),
        upper: None,
    }));
    let original = original(&mut frame)?;
    let pi = frame.term(
        Operator::Constant(nrz_bindings::readings::algebra::syntax::Constant::Pi),
        real(),
        &[],
    )?;
    let thousand = exact(&mut frame, "1000", false)?;
    let period = frame.term(Operator::Div, real(), &[pi, thousand])?;
    let target = frame.term(
        Operator::Div,
        real(),
        &[original.definition.clone(), period],
    )?;
    let (mut source, family) = super::family::finish(frame, family, original.selected, target)?;
    let meaning = meaning()?;
    let coordinates = CoordinateFamily::new(
        &source.frame.cas,
        &source.context,
        &original.root,
        vec![meaning.clone()],
        &mut ledger()?,
    )?;
    let reading = PhysicalReading::define(
        &mut source.frame.cas,
        &coordinates,
        ReadingDefinition {
            symbol: original.symbol,
            selection: SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
            map: original.map,
            entries: vec![meaning],
        },
        &mut ledger()?,
    )?;
    assert_eq!(reading.node(), &original.definition);
    assert_eq!(reading.selected(), &family.source);
    Ok((source, family, reading))
}
