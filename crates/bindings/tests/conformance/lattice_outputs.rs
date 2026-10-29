// SPDX-License-Identifier: AGPL-3.0-only
//! Physical selected tuples retain exact kinds, original coordinates and independent capacity.

use super::{
    decision_fixture::ledger,
    lattice_selection::{admitted, prepared, select},
};
use crate::testing::declarations::TestResult;
use nrz_bindings::{DecisionError, Kind, LatticeDecisionCase, LatticeIndexKind, LatticeMetric};
use nrz_cas::{
    ExactConstant, Rational, lattice::LatticeConvention, proof::Statement, syntax::Constant,
};
use std::rc::Rc;

/// Selected phase and torus coordinates keep angular representatives and explicitly named winding kinds.
/// ´claim:bindings:lattice-selected-angles´
/// ´test:integration:bindings-lattice-selected-angles´
#[test]
fn bindings_lattice_selected_angles() -> TestResult {
    let pi = ExactConstant::named(Constant::Pi, &mut ledger()?)?;
    for kind in [0, 1] {
        let selected = admitted(kind, LatticeConvention::Floor)?;
        let expected = if kind == 0 { vec!["1"] } else { vec!["3", "1"] };
        for (position, index) in expected.into_iter().enumerate() {
            let k = Rational::parse(index, &mut ledger()?)?;
            assert_eq!(selected.indices()[position], k);
            assert!(matches!(
                selected.index_kind(position),
                Some(LatticeIndexKind::SignedWinding(_))
            ));
            let coordinate = selected
                .representative(position)
                .ok_or("missing representative")?;
            assert_eq!(coordinate.descriptor().kind(), &Kind::Angle);
            let expected = if kind == 0 {
                pi.div(
                    &ExactConstant::from(Rational::from_ratio(2, 1, &mut ledger()?)?),
                    &mut ledger()?,
                )?
            } else {
                let original = ExactConstant::from(Rational::from_ratio(
                    if position == 0 { 20 } else { 10 },
                    1,
                    &mut ledger()?,
                )?);
                let factor = ExactConstant::from(Rational::from_ratio(
                    if position == 0 { 6 } else { 2 },
                    1,
                    &mut ledger()?,
                )?);
                original.sub(&pi.mul(&factor, &mut ledger()?)?, &mut ledger()?)?
            };
            assert_eq!(coordinate.exact(), &expected);
        }
        assert!(selected.witness().is_discharged());
        assert!(selected.reconstruction().is_discharged());
        let site = selected.site(&mut ledger()?)?;
        for position in 0..selected.indices().len() {
            let factor = ExactConstant::from(
                selected.indices()[position]
                    .mul(&Rational::from_ratio(2, 1, &mut ledger()?)?, &mut ledger()?)?,
            );
            let expected = pi.mul(&factor, &mut ledger()?)?;
            assert_eq!(
                site.coordinate(position).ok_or("missing site")?.exact(),
                &expected
            );
            assert_eq!(
                site.selection().witness().proof(),
                selected.witness().proof()
            );
        }
    }
    Ok(())
}

/// Equal grid tuples retain different floor and nearest statements and cannot exchange attachments.
/// ´claim:bindings:lattice-selected-convention´
/// ´test:integration:bindings-lattice-selected-convention´
#[test]
fn bindings_lattice_selected_convention() -> TestResult {
    let floor = admitted(2, LatticeConvention::Floor)?;
    let nearest = admitted(2, LatticeConvention::Nearest)?;
    assert_eq!(floor.indices(), nearest.indices());
    assert!(matches!(
        floor.witness().judgment().statement,
        Statement::LatticeSelection(_)
    ));
    assert!(matches!(
        nearest.witness().judgment().statement,
        Statement::LatticeNearestSelection(_)
    ));
    for selected in [&floor, &nearest] {
        for (position, expected) in ["1/20", "1/80"].into_iter().enumerate() {
            assert_eq!(
                selected
                    .representative(position)
                    .ok_or("missing grid representative")?
                    .exact()
                    .as_rational(),
                Some(&Rational::parse(expected, &mut ledger()?)?)
            );
            assert!(matches!(
                selected.index_kind(position),
                Some(LatticeIndexKind::QuantizationIndex(_))
            ));
        }
    }
    let mut value = prepared(2, LatticeConvention::Floor)?;
    let mut changed = value.case.specification().clone();
    changed.convention = LatticeConvention::Nearest;
    changed.metric = Some(LatticeMetric::EuclideanNormalized);
    let changed = LatticeDecisionCase::declare(&value.fixture.frame.cas, changed, &mut ledger()?)?;
    let witness = select(&mut value)?;
    let outputs = value
        .fixture
        .frame
        .cas
        .lattice_selection_outputs(&witness, &mut ledger()?)?;
    let graph = Rc::new(value.fixture.frame.cas);
    let before = graph.counts();
    assert!(matches!(
        changed.admit(
            Rc::clone(&graph),
            &outputs,
            value.case.attachment(),
            &mut ledger()?
        ),
        Err(DecisionError::Attachment)
    ));
    assert!(matches!(
        changed.admit(
            Rc::clone(&graph),
            &outputs,
            changed.attachment(),
            &mut ledger()?
        ),
        Err(DecisionError::Case)
    ));
    assert_eq!(graph.counts(), before);
    assert_eq!(outputs.selection().proof(), witness.proof());
    Ok(())
}

/// Nonunit references restore selected representatives as points and basis contributions as differences.
/// ´claim:bindings:lattice-selected-points´
/// ´test:integration:bindings-lattice-selected-points´
#[test]
fn bindings_lattice_selected_points() -> TestResult {
    let mut value = prepared(2, LatticeConvention::Floor)?;
    let spec = super::lattice_points::specification(&mut value.fixture)?;
    value.case = LatticeDecisionCase::declare(&value.fixture.frame.cas, spec, &mut ledger()?)?;
    let witness = select(&mut value)?;
    let outputs = value
        .fixture
        .frame
        .cas
        .lattice_selection_outputs(&witness, &mut ledger()?)?;
    let selected = value.case.admit(
        Rc::new(value.fixture.frame.cas),
        &outputs,
        value.case.attachment(),
        &mut ledger()?,
    )?;
    let site = selected.site(&mut ledger()?)?;
    for (position, representative, location) in [(0, "101/10", "21/2"), (1, "1603/80", "329/16")] {
        let coordinate = selected
            .representative(position)
            .ok_or("missing point representative")?;
        assert_eq!(coordinate.descriptor().role(), nrz_bindings::Role::Point);
        assert_eq!(
            coordinate.exact().as_rational(),
            Some(&Rational::parse(representative, &mut ledger()?)?)
        );
        assert_eq!(
            site.coordinate(position)
                .ok_or("missing point site")?
                .exact()
                .as_rational(),
            Some(&Rational::parse(location, &mut ledger()?)?)
        );
    }
    Ok(())
}

/// Equal-valued changed physical sources cannot reuse the original selected attachment.
/// ´claim:bindings:lattice-selected-source-identity´
/// ´test:integration:bindings-lattice-selected-source-identity´
#[test]
fn bindings_lattice_selected_source_identity() -> TestResult {
    let mut value = prepared(2, LatticeConvention::Floor)?;
    let other = super::lattice_physical::source(2, "1", "changed selected physical source")?;
    let spec = super::lattice_physical::specification(&mut value.fixture, 2, &other)?;
    let changed = LatticeDecisionCase::declare(&value.fixture.frame.cas, spec, &mut ledger()?)?;
    let witness = select(&mut value)?;
    let outputs = value
        .fixture
        .frame
        .cas
        .lattice_selection_outputs(&witness, &mut ledger()?)?;
    let graph = Rc::new(value.fixture.frame.cas);
    let before = graph.counts();
    assert!(matches!(
        changed.admit(
            Rc::clone(&graph),
            &outputs,
            value.case.attachment(),
            &mut ledger()?
        ),
        Err(DecisionError::Attachment)
    ));
    assert!(matches!(
        value.case.admit(
            Rc::clone(&graph),
            &outputs,
            changed.attachment(),
            &mut ledger()?
        ),
        Err(DecisionError::Attachment)
    ));
    assert_eq!(graph.counts(), before);
    assert_eq!(outputs.selection().proof(), witness.proof());
    Ok(())
}

/// A predeclared counted kind must admit the selected mathematical value before any capacity consumer.
/// ´claim:bindings:lattice-selected-count-domain´
/// ´test:integration:bindings-lattice-selected-count-domain´
#[test]
fn bindings_lattice_selected_count_domain() -> TestResult {
    let mut value = prepared(2, LatticeConvention::Floor)?;
    let mut spec = value.case.specification().clone();
    spec.indices[0] = LatticeIndexKind::Count(Box::new(nrz_bindings::Descriptor::new(
        Kind::Count(
            "sample cells".into(),
            nrz_bindings::CountDomain::new(Some(3), Some(9))?,
        ),
        nrz_bindings::Measure::coherent(),
        nrz_bindings::Role::Difference,
    )?));
    let counted = LatticeDecisionCase::declare(&value.fixture.frame.cas, spec, &mut ledger()?)?;
    let witness = select(&mut value)?;
    let outputs = value
        .fixture
        .frame
        .cas
        .lattice_selection_outputs(&witness, &mut ledger()?)?;
    let graph = Rc::new(value.fixture.frame.cas);
    let before = graph.counts();
    assert!(matches!(
        counted.admit(
            Rc::clone(&graph),
            &outputs,
            counted.attachment(),
            &mut ledger()?
        ),
        Err(DecisionError::Units(nrz_bindings::UnitError::CountValue))
    ));
    assert_eq!(graph.counts(), before);
    assert_eq!(outputs.selection().proof(), witness.proof());
    Ok(())
}

/// Distinct derived values own distinct graphs, while optional sites cite retained displacements.
/// ´claim:bindings:lattice-selected-preparations´
/// ´test:integration:bindings-lattice-selected-preparations´
#[test]
fn bindings_lattice_selected_preparations() -> TestResult {
    for (kind, expected) in [
        (0, [1, 16, 2_930]),
        (1, [3, 64, 10_994]),
        (2, [0, 0, 1_162]),
    ] {
        let selected = admitted(kind, LatticeConvention::Floor)?;
        let usage = selected.preparation_usage();
        assert_eq!([usage.graphs, usage.records, usage.bytes], expected);
        let first = selected.site(&mut ledger()?)?;
        let second = selected.site(&mut ledger()?)?;
        for position in 0..selected.indices().len() {
            assert!(std::ptr::eq(
                first
                    .coordinate(position)
                    .ok_or("missing first site")?
                    .exact(),
                second
                    .coordinate(position)
                    .ok_or("missing second site")?
                    .exact()
            ));
        }
        if kind < 2 {
            let position = usize::from(kind);
            let (original, _) = selected.case().specification().axes[position].basis[position]
                .exact()
                .graph()
                .ok_or("missing retained period graph")?;
            let coordinate = first.coordinate(position).ok_or("missing shared site")?;
            let (site, _) = coordinate
                .exact()
                .graph()
                .ok_or("missing shared site graph")?;
            assert!(std::ptr::eq(original, site));
        }
        if kind == 1 {
            let product = first.coordinate(0).ok_or("missing six-Pi displacement")?;
            let first_residual = selected.representative(0).ok_or("missing first residual")?;
            let second_residual = selected
                .representative(1)
                .ok_or("missing second residual")?;
            assert_eq!(
                [
                    records(product.exact())?,
                    records(first_residual.exact())?,
                    records(second_residual.exact())?
                ],
                [16, 24, 24]
            );
            assert_ne!(product.exact(), first_residual.exact());
            assert_ne!(first_residual.exact(), second_residual.exact());
        }
    }
    Ok(())
}

/// Count independently owned exact records without a whole-Debug comparison.
fn records(value: &ExactConstant) -> TestResult<usize> {
    let (graph, _) = value.graph().ok_or("missing exact value graph")?;
    let counts = graph.counts();
    Ok(counts.vertices + counts.edges)
}
