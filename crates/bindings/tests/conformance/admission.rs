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

/// Opaque tickets cannot omit a required region or assign one region to two symbols.
/// ´test:integration:symbol-binding-refuses-missing-and-double-region-owners´
#[test]
fn symbol_binding_refuses_missing_and_double_region_owners() -> TestResult {
    let symbols = BindingSymbols::declare(
        &Descriptor::ratio(),
        [1, 1],
        [true, true],
        &mut Budget::new(Limits::default())?,
    )?;
    nrz_bindings::SymbolBinding::check(symbols.clone(), [Some(1), Some(2), Some(3)])?;
    for regions in [
        [None, Some(2), Some(3)],
        [Some(1), None, Some(3)],
        [Some(1), Some(2), None],
        [Some(1), Some(1), Some(3)],
        [Some(1), Some(2), Some(2)],
    ] {
        assert!(matches!(
            nrz_bindings::SymbolBinding::check(symbols.clone(), regions),
            Err(AdmissionError::RegionInventory)
        ));
    }
    Ok(())
}
