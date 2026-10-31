// SPDX-License-Identifier: AGPL-3.0-only
//! Closed symbol relations are checked independently of numerical storage.

use crate::testing::declarations::TestResult;
use nrz_bindings::{
    AdmissionError, BindingSymbols, CompanionKind, Descriptor, HomogeneousRecipe, Kind, Measure,
    Role, SymbolKind,
};
use nrz_cas::{
    Rational,
    limits::{Budget, Limits},
    syntax::{AtomicSort, Field, Sort},
};

/// Every companion has its own immutable symbol and ordered target edge.
/// ´test:integration:binding-symbols-retain-companion-kinds-and-edges´
#[test]
fn binding_symbols_retain_companion_kinds_and_edges() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(&Descriptor::ratio(), [2, 1], [true, true], &mut budget)?;
    for (symbol, kind) in [
        (symbols.bound(), CompanionKind::DeterministicBound),
        (symbols.error(), CompanionKind::NumericalError),
    ] {
        let symbol = symbol.ok_or("missing companion")?;
        assert_eq!(symbol.kind(), SymbolKind::Companion(kind));
        assert_ne!(symbol, symbols.value());
        assert_eq!(symbol.target(), Some(symbols.value().node()));
        assert_eq!(symbol.descriptor().measure().offset(), &Rational::zero());
        let cas = symbol.cas()?;
        let relation =
            cas.describe_syntax(symbol.relation().ok_or("missing edge")?, &mut budget)?;
        assert_eq!(relation.dependencies.len(), 2);
        assert_eq!(&relation.dependencies[0].target, symbol.node());
        assert_eq!(&relation.dependencies[1].target, symbols.value().node());
    }
    Ok(())
}

/// A planned copy has a fresh declared result and fresh companions before any heap exists.
/// ´test:integration:copy-result-symbols-precede-execution´
#[test]
fn copy_result_symbols_precede_execution() -> TestResult {
    let quantity = Descriptor::ratio();
    let recipe = HomogeneousRecipe::check([Some(&quantity)].into_iter(), 1)?;
    let mut budget = Budget::new(Limits::default())?;
    let source = BindingSymbols::declare(&quantity, [2, 1], [true, true], &mut budget)?;
    let result = source.copy_result([1, 1], &mut budget)?;
    recipe.check_copy(&source, &result)?;
    assert_ne!(source.value(), result.value());
    assert_ne!(source.bound(), result.bound());
    assert_ne!(source.error(), result.error());
    assert_eq!(result.value().shape(), [1, 1]);
    assert!(matches!(
        result.value().sort(),
        Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            ..
        })
    ));
    assert_eq!(result.value().target(), Some(source.value().node()));
    let unrelated = BindingSymbols::declare(&quantity, [2, 1], [true, true], &mut budget)?;
    assert!(matches!(
        recipe.check_copy(&unrelated, &result),
        Err(AdmissionError::ResultMismatch)
    ));
    Ok(())
}

/// Equal dimensions cannot authorize another physical kind or erase an affine role.
/// ´test:integration:homogeneous-recipe-refuses-kind-point-and-formal-substitution´
#[test]
fn homogeneous_recipe_refuses_kind_point_and_formal_substitution() -> TestResult {
    let ratio = Descriptor::ratio();
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
    let point = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Point)?;
    assert_eq!(ratio.dimension(), angle.dimension());
    for entries in [
        [Some(&ratio), Some(&angle)],
        [Some(&point), Some(&point)],
        [Some(&ratio), None],
    ] {
        assert!(matches!(
            HomogeneousRecipe::check(entries.into_iter(), 2),
            Err(AdmissionError::UnsupportedRecipe)
        ));
    }
    let recipe = HomogeneousRecipe::check([Some(&ratio)].into_iter(), 1)?;
    assert!(matches!(
        recipe.check_descriptor(&angle),
        Err(AdmissionError::ValueMismatch)
    ));
    Ok(())
}

/// Companion meaning and target identity both matter even with equal numerical units.
/// ´test:integration:binding-symbols-refuse-swapped-and-foreign-companions´
#[test]
fn binding_symbols_refuse_swapped_and_foreign_companions() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let first = BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], &mut budget)?;
    let second = BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], &mut budget)?;
    for (bound, error) in [
        (first.error().cloned(), first.bound().cloned()),
        (second.bound().cloned(), first.error().cloned()),
        (first.bound().cloned(), second.error().cloned()),
        (None, first.error().cloned()),
        (None, None),
        (first.bound().cloned(), None),
    ] {
        assert!(matches!(
            BindingSymbols::check(first.value().clone(), bound, error),
            Err(AdmissionError::CompanionMismatch)
        ));
    }
    Ok(())
}

/// The complete homogeneous traversal succeeds at N entries and refuses N minus one.
/// ´test:integration:homogeneous-admission-entry-boundary´
#[test]
fn homogeneous_admission_entry_boundary() -> TestResult {
    let quantity = Descriptor::ratio();
    let entries = [Some(&quantity); 3];
    HomogeneousRecipe::check(entries.into_iter(), 3)?;
    assert!(matches!(
        HomogeneousRecipe::check(entries.into_iter(), 2),
        Err(AdmissionError::ResourceLimit)
    ));
    let empty = HomogeneousRecipe::check([].into_iter(), 0)?;
    assert_eq!(empty.descriptor(), &quantity);
    Ok(())
}

/// One cumulative CAS ledger covers input and copied result construction without reset.
/// ´test:integration:binding-symbol-graph-work-boundary´
#[test]
fn binding_symbol_graph_work_boundary() -> TestResult {
    let mut measured = Budget::new(Limits::default())?;
    construct(&mut measured)?;
    let needed = measured.usage().work;
    let mut exact = Budget::new(Limits {
        work: needed,
        ..Limits::default()
    })?;
    construct(&mut exact)?;
    assert_eq!(exact.usage().work, needed);
    let mut short = Budget::new(Limits {
        work: needed - 1,
        ..Limits::default()
    })?;
    assert!(construct(&mut short).is_err());
    Ok(())
}

/// The same closed graph construction is used at each measured work boundary.
fn construct(budget: &mut Budget) -> TestResult {
    let input = BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], budget)?;
    input.copy_result([1, 1], budget)?;
    Ok(())
}

/// Opaque associations require complete presence; actual overlap compatibility needs live admission.
/// ´test:integration:symbol-binding-requires-complete-associations´
#[test]
fn symbol_binding_requires_complete_associations() -> TestResult {
    let symbols = BindingSymbols::declare(
        &Descriptor::ratio(),
        [1, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    for regions in [vec![1, 2, 3], vec![1, 1, 3], vec![1, 2, 2]] {
        nrz_bindings::SymbolBinding::check(symbols.clone(), regions)?;
    }
    for regions in [vec![], vec![1], vec![1, 2], vec![1, 2, 3, 4]] {
        assert!(matches!(
            nrz_bindings::SymbolBinding::check(symbols.clone(), regions),
            Err(AdmissionError::RegionInventory)
        ));
    }
    Ok(())
}

/// Shared environments preserve three fresh symbols and both ordered target relations per result.
/// ´test:integration:frozen-symbol-environments-preserve-fresh-targets´
/// ´claim:bindings:frozen-symbol-freshness´ Two declarations from one frozen environment share no value, companion or relation identity and retain their own exact targets.
#[test]
fn frozen_symbol_environments_preserve_fresh_targets() -> TestResult {
    let schema = nrz_bindings::SymbolSchema::freeze(
        &Descriptor::ratio(),
        [1, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    let first = schema.declare(&mut Budget::new(Limits::default())?)?;
    let second = schema.declare(&mut Budget::new(Limits::default())?)?;
    assert_ne!(first.value(), second.value());
    let mut inspection = Budget::new(Limits::default())?;
    for (a, b) in [
        (first.bound(), second.bound()),
        (first.error(), second.error()),
    ] {
        let a = a.ok_or("first companion")?;
        let b = b.ok_or("second companion")?;
        assert_ne!(a, b);
        assert_ne!(a.relation(), b.relation());
        assert_eq!(a.target(), Some(first.value().node()));
        assert_eq!(b.target(), Some(second.value().node()));
        for (companion, value) in [(a, first.value()), (b, second.value())] {
            let operands = companion.cas()?.operands(
                companion.relation().ok_or("target relation")?,
                &mut inspection,
            )?;
            assert_eq!(operands, [companion.node().clone(), value.node().clone()]);
        }
    }
    Ok(())
}

/// Equal dimensions and equal numerical scales cannot substitute for the retained descriptor.
/// ´test:integration:frozen-symbol-schema-refuses-incompatible-meaning´
/// ´claim:bindings:frozen-symbol-compatibility´ A frozen ratio schema refuses angle, exact-scale, measure-name, shape and inventory substitutions before symbol publication.
#[test]
fn frozen_symbol_schema_refuses_incompatible_meaning() -> TestResult {
    let schema = nrz_bindings::SymbolSchema::freeze(
        &Descriptor::ratio(),
        [1, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
    let alias = Descriptor::new(
        Kind::Ratio,
        Measure::new(
            "ratio alias",
            nrz_bindings::Scale::Exact(Rational::one()),
            Rational::zero(),
        )?,
        Role::Difference,
    )?;
    let two = Rational::from_ratio(2, 1, &mut Budget::new(Limits::default())?)?;
    let scaled = Descriptor::new(
        Kind::Ratio,
        Measure::new("SI", nrz_bindings::Scale::Exact(two), Rational::zero())?,
        Role::Difference,
    )?;
    for (descriptor, shape, companions) in [
        (&angle, [1, 1], [true, true]),
        (&scaled, [1, 1], [true, true]),
        (&alias, [1, 1], [true, true]),
        (&Descriptor::ratio(), [2, 1], [true, true]),
        (&Descriptor::ratio(), [1, 1], [false, false]),
    ] {
        let mut budget = Budget::new(Limits::default())?;
        let Err(fault) = schema.declare_for(descriptor, shape, companions, &mut budget) else {
            panic!("changed frozen schema must refuse");
        };
        assert_eq!(fault, AdmissionError::ValueMismatch);
        assert!(budget.usage().work > 0);
    }
    schema.declare_for(
        &Descriptor::ratio(),
        [1, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    Ok(())
}

/// A policy refusal neither revokes immutable authority nor exposes a partial symbol bundle.
/// ´test:integration:frozen-symbol-schema-refuses-changed-policy´
/// ´claim:bindings:frozen-symbol-policy´ Changed name-comparison policy refuses a retained environment while a compatible fresh owner still accepts it.
#[test]
fn frozen_symbol_schema_refuses_changed_policy() -> TestResult {
    let schema = nrz_bindings::SymbolSchema::freeze(
        &Descriptor::ratio(),
        [1, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    let first = schema.declare(&mut Budget::new(Limits::default())?)?;
    let before = first
        .value()
        .cas()?
        .canonical_snapshot(&mut Budget::new(Limits::default())?)?;
    let mut budget = Budget::new(Limits {
        name_bytes: 255,
        ..Limits::default()
    })?;
    let Err(AdmissionError::Algebra(fault)) = schema.declare(&mut budget) else {
        panic!("changed frozen environment policy must refuse");
    };
    assert_eq!(
        fault.kind,
        nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::Policy)
    );
    assert!(budget.usage().work > 0);
    assert_eq!(
        first
            .value()
            .cas()?
            .canonical_snapshot(&mut Budget::new(Limits::default())?)?,
        before,
    );
    let retained = schema.clone();
    drop(schema);
    let second = retained.declare(&mut Budget::new(Limits::default())?)?;
    assert_ne!(first.value(), second.value());
    Ok(())
}

/// New immutable schema authority remains shareable even though existing quantity owners are local.
/// ´test:integration:frozen-symbol-schema-is-send-and-sync´
/// ´claim:bindings:frozen-symbol-thread-safety´ `SymbolSchema`, `FrozenEnvironment` and `Cas` implement `Send` and `Sync`.
#[test]
fn frozen_symbol_schema_is_send_and_sync() {
    /// Require both auto traits without constructing an owner.
    const fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<nrz_bindings::SymbolSchema>();
    require_send_sync::<nrz_cas::domain::FrozenEnvironment>();
    require_send_sync::<nrz_cas::Cas>();
}

/// A retained schema owns its freeze while each result pays only fresh graph admission.
/// ´test:integration:frozen-symbol-declaration-work-boundaries´
/// ´claim:bindings:frozen-symbol-work´ A schema freezes at 3046 Work and warm declarations require 8100, with atomic refusal at 8099 and unchanged earlier evidence.
#[test]
fn frozen_symbol_declaration_work_boundaries() -> TestResult {
    for limit in [8_100, 8_099] {
        let mut freeze = Budget::new(Limits::default())?;
        let schema = nrz_bindings::SymbolSchema::freeze(
            &Descriptor::ratio(),
            [1, 1],
            [true, true],
            &mut freeze,
        )?;
        assert_eq!(freeze.usage().work, 3_046);
        let first = schema.declare(&mut Budget::new(Limits::default())?)?;
        let before = first
            .value()
            .cas()?
            .canonical_snapshot(&mut Budget::new(Limits::default())?)?;
        let mut budget = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        let result = schema.declare(&mut budget);
        if limit == 8_100 {
            let second = result?;
            assert_eq!(budget.usage().work, 8_100);
            assert_ne!(first.value(), second.value());
            assert_ne!(first.bound(), second.bound());
            assert_ne!(first.error(), second.error());
        } else {
            let Err(AdmissionError::Algebra(fault)) = result else {
                panic!("one-below retained declaration must refuse");
            };
            assert!(matches!(
                fault.kind,
                nrz_cas::FaultKind::Exhausted {
                    budget: nrz_cas::limits::BudgetKind::Work,
                    ..
                }
            ));
        }
        assert!(budget.usage().work > 0);
        assert!(budget.usage().work <= limit);
        assert_eq!(
            first
                .value()
                .cas()?
                .canonical_snapshot(&mut Budget::new(Limits::default())?)?,
            before,
        );
    }
    Ok(())
}
