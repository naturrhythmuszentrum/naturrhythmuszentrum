// SPDX-License-Identifier: AGPL-3.0-only
//! Design-derived implication and rational bracket oracles for the registered law.

use super::super::super::{Interval, roots};
use super::super::service::Certificate;
use super::*;

/// The portable claim has the design's implication fields; replay separately authenticates scope.
/// The expected implication is taken from `dec:nrz:cas-lc-root-composition`.
/// ´test:unit:lc-root-registered-composition-oracle´
#[test]
fn lc_root_registered_composition_oracle() -> TestResult {
    let mut source = fixture()?;
    let receipt = source
        .cas
        .construct_lc_root(&source.request, &mut ledger()?)?;
    let certificate = receipt.certificate(&source.cas, &mut ledger()?)?;
    let mut receiver = fixture()?;
    let replayed = receiver
        .cas
        .check_lc_root(&receiver.request, &certificate, &mut ledger()?)?;
    assert_eq!(
        replayed.request(&receiver.cas, &mut ledger()?)?,
        &receiver.request
    );
    let scope = super::super::wire::scope(&receiver.cas, &receiver.request, &mut ledger()?)?;
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    let claim = super::super::wire::decode(&receiver.request, &scope, &bytes, &mut ledger()?)?;
    assert_eq!(claim.universal, expected_binders(&receiver.request));
    assert_eq!(claim.premises, expected_premises());
    assert_eq!(claim.conclusions, expected_conclusions());
    assert_eq!(claim.expansion, expected_expansion());
    assert_eq!(
        claim.native,
        [
            Native::ProductTotal,
            Native::RootTotal,
            Native::ReciprocalTotal,
            Native::ReciprocalLocalOperation,
        ]
    );
    original_scope_refusals(&certificate)?;
    Ok(())
}

/// Each changed inventory is a valid receiving-store request with a different original scope.
fn original_scope_refusals(certificate: &Certificate) -> TestResult {
    for change in [
        ScopeChange::SwappedConstraints,
        ScopeChange::DroppedConstraint,
        ScopeChange::ReorderedInputs,
    ] {
        let mut receiver = fixture()?;
        let mut altered = receiver.request.clone();
        match change {
            ScopeChange::SwappedConstraints => altered.constraints.swap(0, 1),
            ScopeChange::DroppedConstraint => {
                altered.constraints.pop();
            }
            ScopeChange::ReorderedInputs => altered.inputs.swap(0, 3),
        }
        let before = receiver.cas.canonical_snapshot(&mut ledger()?)?;
        let error = receiver
            .cas
            .check_lc_root(&altered, certificate, &mut ledger()?)
            .err()
            .ok_or("changed original scope accepted")?;
        assert!(matches!(error.kind, crate::FaultKind::Invalid(_)));
        assert_eq!(receiver.cas.retained_judgment_count(), 0);
        assert_eq!(receiver.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Three independent original-inventory changes that preserve valid local handle ownership.
enum ScopeChange {
    SwappedConstraints,
    DroppedConstraint,
    ReorderedInputs,
}

/// First-occurrence input binders precede the six distinct endpoint parameters.
fn expected_binders(request: &Request) -> Vec<Handle> {
    let mut binders = Vec::new();
    for input in &request.inputs {
        if !binders.contains(input) {
            binders.push(input.clone());
        }
    }
    binders.extend(request.endpoints.iter().cloned());
    binders
}

/// Positivity, product coverage, squared brackets and reciprocal inclusion are antecedents.
const fn expected_premises() -> [Predicate; 11] {
    use Relation::{AtMost, Less};
    use Term::{C, Endpoint as E, Inverse as I, L, Product as P, Square as S, Zero as Z};
    [
        Predicate(Z, Less, L),
        Predicate(Z, Less, C),
        Predicate(Z, Less, E(0)),
        Predicate(E(0), AtMost, P),
        Predicate(P, AtMost, E(1)),
        Predicate(Z, Less, E(2)),
        Predicate(Z, AtMost, E(3)),
        Predicate(S(2), AtMost, E(0)),
        Predicate(E(1), AtMost, S(3)),
        Predicate(E(4), AtMost, I(3)),
        Predicate(I(2), AtMost, E(5)),
    ]
}

/// The reverse polynomial is a consequence, never a replacement for the root target.
const fn expected_conclusions() -> [Predicate; 7] {
    use Relation::{AtMost, Equal, Less};
    use Term::{Endpoint as E, Product as P, Reciprocal as A, Reverse as V, Root as T, Zero as Z};
    [
        Predicate(Z, Less, P),
        Predicate(Z, Less, T),
        Predicate(V, Equal, Z),
        Predicate(E(2), AtMost, T),
        Predicate(T, AtMost, E(3)),
        Predicate(E(4), AtMost, A),
        Predicate(A, AtMost, E(5)),
    ]
}

/// These five ordered steps are the reviewed expansion of the closed root argument.
const fn expected_expansion() -> [Step; 5] {
    [
        Step::ProductPositive,
        Step::RootUnique,
        Step::RootBracket,
        Step::ReciprocalOrder,
        Step::ReciprocalBracket,
    ]
}

/// Replacing a native antecedent cannot turn the registered law into unconditional authority.
/// ´test:unit:lc-root-registered-native-antecedent-oracle´
#[test]
fn lc_root_registered_native_antecedent_oracle() -> TestResult {
    let mut source = fixture()?;
    let receipt = source
        .cas
        .construct_lc_root(&source.request, &mut ledger()?)?;
    let certificate = receipt.certificate(&source.cas, &mut ledger()?)?;
    let mut receiver = fixture()?;
    let scope = super::super::wire::scope(&receiver.cas, &receiver.request, &mut ledger()?)?;
    let bytes = certificate.to_bytes(&mut ledger()?)?;
    let original = super::super::wire::decode(&receiver.request, &scope, &bytes, &mut ledger()?)?;
    let reencoded = super::super::wire::encode(&original, &scope, &mut ledger()?)?;
    assert_eq!(reencoded, bytes);
    for (expected, replacement) in [
        (Native::ProductTotal, Native::RootTotal),
        (Native::RootTotal, Native::ProductTotal),
        (Native::ReciprocalTotal, Native::ProductTotal),
        (Native::ReciprocalLocalOperation, Native::ProductTotal),
    ] {
        let index = original
            .native
            .iter()
            .position(|obligation| *obligation == expected)
            .ok_or("missing original native obligation")?;
        let mut changed = original.clone();
        changed.native[index] = replacement;
        let wire = super::super::wire::encode(&changed, &scope, &mut ledger()?)?;
        let untrusted = Certificate::from_bytes(&wire, &mut ledger()?)?;
        let before = receiver.cas.canonical_snapshot(&mut ledger()?)?;
        let error = receiver
            .cas
            .check_lc_root(&receiver.request, &untrusted, &mut ledger()?)
            .err()
            .ok_or("changed native obligation accepted")?;
        assert_eq!(
            error.kind,
            crate::FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
        );
        assert_eq!(receiver.cas.retained_judgment_count(), 0);
        assert_eq!(receiver.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// The design's 7/5 bracket is an exact-rational premise, not a `RootGrid` endpoint.
/// ´test:unit:lc-root-design-bracket-constants´
#[test]
fn lc_root_design_bracket_constants() -> TestResult {
    use std::cmp::Ordering;
    let mut budget = ledger()?;
    let pl = Rational::from_ratio(49, 25, &mut budget)?;
    let pu = Rational::from_ratio(9, 4, &mut budget)?;
    let lower = Rational::from_ratio(7, 5, &mut budget)?;
    let upper = Rational::from_ratio(3, 2, &mut budget)?;
    let product = Rational::from_ratio(2, 1, &mut budget)?;
    assert_eq!(lower.mul(&lower, &mut budget)?, pl);
    assert_eq!(upper.mul(&upper, &mut budget)?, pu);
    assert_eq!(pl.compare(&product, &mut budget)?, Ordering::Less);
    assert_eq!(product.compare(&pu, &mut budget)?, Ordering::Less);
    let al = Rational::from_ratio(2, 3, &mut budget)?;
    let au = Rational::from_ratio(5, 7, &mut budget)?;
    assert_eq!(al.mul(&upper, &mut budget)?, Rational::one());
    assert_eq!(au.mul(&lower, &mut budget)?, Rational::one());
    assert_eq!(al.compare(&au, &mut budget)?, Ordering::Less);
    let invalid_al = Rational::from_ratio(3, 4, &mut budget)?;
    assert_eq!(invalid_al.compare(&al, &mut budget)?, Ordering::Greater);
    Ok(())
}

/// `RootGrid` checks the same P=2 through dyadic neighbors and rejects broken endpoints.
/// Its grid-one-eighth [11/8,3/2] encloses the design's narrower 7/5 lower bound.
/// ´test:unit:lc-root-grid-bracket-oracle´
#[test]
fn lc_root_grid_bracket_oracle() -> TestResult {
    let mut budget = ledger()?;
    let product = Rational::from_ratio(2, 1, &mut budget)?;
    let lower = Rational::from_ratio(11, 8, &mut budget)?;
    let upper = Rational::from_ratio(3, 2, &mut budget)?;
    let input = Interval::new(product.clone(), product, &mut budget)?;
    let root = Interval::new(lower.clone(), upper.clone(), &mut budget)?;
    roots::check(&input, &root, 3, &mut budget)?;
    let too_high = Interval::new(upper.clone(), upper, &mut budget)?;
    let too_low = Interval::new(lower.clone(), lower, &mut budget)?;
    assert!(roots::check(&input, &too_high, 3, &mut budget).is_err());
    assert!(roots::check(&input, &too_low, 3, &mut budget).is_err());
    Ok(())
}
