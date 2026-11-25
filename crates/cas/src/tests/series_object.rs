// SPDX-License-Identifier: AGPL-3.0-only
//! Object records carry syntax through the existing graph without creating a theorem.

use crate::domain::Layer;
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::series::SeriesObject;
use crate::solving::{CoefficientDomainData, SolveDraft};
use crate::syntax::Operator;
use crate::testing::common::{TestResult, default_budget, real};
use crate::testing::solve_graph::{Fixture, admit, expression};

/// Independent stores admit the scalar leaves in opposite orders.
fn fixture(reordered: bool) -> TestResult<(Cas, Handle)> {
    let mut f = Fixture::new()?;
    let field = admit(
        &mut f.cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
    )?;
    let mut numbers = Vec::new();
    for text in if reordered {
        ["2", "1", "0"]
    } else {
        ["0", "1", "2"]
    } {
        numbers.push(admit(
            &mut f.cas,
            &expression(
                Reference::Existing(f.header.clone()),
                Operator::Rational(crate::Rational::parse(text, &mut default_budget()?)?),
                real(),
                Vec::new(),
            ),
        )?);
    }
    if reordered {
        numbers.reverse();
    }
    let x = admit(
        &mut f.cas,
        &expression(
            Reference::Existing(f.header.clone()),
            Operator::Free("x".into()),
            real(),
            Vec::new(),
        ),
    )?;
    let zero = numbers.first().ok_or("zero")?.clone();
    let coefficients = numbers
        .iter()
        .map(|value| Reference::Existing(value.clone()))
        .collect();
    let object = admit(
        &mut f.cas,
        &Draft::SeriesObject(SeriesObject {
            field: Reference::Existing(field),
            centre: Reference::Existing(zero),
            variable: Reference::Existing(x),
            scope: Vec::new(),
            context: Reference::Existing(f.context),
            order: 3,
            coefficients,
        }),
    )?;
    Ok((f.cas, object))
}

/// Portable object decoding uses ordinary admission in both allocation orders.
/// ´test:crate:series-object-wire-admission´
#[test]
fn series_object_wire_admission() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let (sender, source) = fixture(false)?;
    let record = crate::wire_records::describe(
        &sender,
        &crate::wire_build::Entity::Object(source.logical_id()),
        &mut default_budget()?,
    )?;
    assert_eq!(record.tag, 38);
    assert_eq!(source.logical_id().kind.tag(), 41);
    assert_eq!(record.layer, Layer::Logic);
    assert!(record.space.is_none());
    assert_eq!(record.references.len(), 7);
    for reordered in [false, true] {
        let (mut receiver, expected) = fixture(reordered)?;
        let objects: Vec<_> = receiver
            .series_object(&expected, &mut budget)?
            .references()
            .cloned()
            .map(crate::wire_decode::Decoded::Object)
            .collect();
        let mut payload = record.payload.clone();
        for (index, (offset, _)) in record.references.iter().enumerate() {
            let end = offset.checked_add(4).ok_or("offset")?;
            let target = payload.get_mut(*offset..end).ok_or("reference bytes")?;
            assert_eq!(target, [0, 0, 0, 0]);
            target.copy_from_slice(&u32::try_from(index)?.to_le_bytes());
        }
        let mut budget = default_budget()?;
        let mut reader = crate::wire_read::Reader::new(&payload, &mut budget)?;
        let actual = crate::wire_objects::decode(
            &mut receiver,
            38,
            Layer::Logic,
            None,
            &mut reader,
            &objects,
        )?;
        reader.finished()?;
        assert_eq!(actual, expected);
        assert_eq!(
            crate::wire_build::syntax_key(&sender, source.logical_id(), &mut default_budget()?)?,
            crate::wire_build::syntax_key(&receiver, actual.logical_id(), &mut default_budget()?)?
        );
    }
    Ok(())
}

/// An object root has no checked conclusion even when every syntax record is valid.
/// ´test:crate:series-object-wire-root-has-no-authority´
#[test]
fn series_object_wire_root_has_no_authority() -> TestResult {
    let (sender, source) = fixture(false)?;
    let (records, root, rules, _) = crate::wire_build::indexed_table(
        &sender,
        crate::wire_build::Entity::Object(source.logical_id()),
        &mut default_budget()?,
    )?;
    assert!(rules.is_empty());
    let bytes = crate::wire_build::envelope(
        records,
        root,
        &rules,
        crate::wire::MAGIC,
        &mut default_budget()?,
    )?;
    let mut receiver = Cas::new(crate::limits::Limits::default())?;
    let snapshot = receiver.canonical_snapshot(&mut default_budget()?)?;
    assert!(receiver.inspect(&bytes, &mut default_budget()?).is_err());
    assert_eq!(
        receiver.canonical_snapshot(&mut default_budget()?)?,
        snapshot
    );
    Ok(())
}
