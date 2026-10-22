// SPDX-License-Identifier: AGPL-3.0-only
//! Original Real production and its independently replayed application closure.

/// Every Real coverage application must replay against its actual original declaration frame.
/// ´test:crate:real-producer-prefixes´
#[test]
fn real_producer_prefixes() -> crate::testing::common::TestResult {
    crate::testing::solve_blind::real_prefixes()
}

/// Independent integer oracles cover every Real stratum and replay its full Real-only dependency closure.
/// ´test:crate:real-independent-acceptance´
#[test]
fn real_independent_acceptance() -> crate::testing::common::TestResult {
    crate::testing::solve_blind::real_acceptance()
}

/// Real ground inputs independently exercise both cubic counts and every degree-drop result shape.
/// ´test:crate:real-ground-degree-ladder´
#[test]
fn real_ground_degree_ladder() -> crate::testing::common::TestResult {
    use crate::solving::{SolutionBranchKind, SolveForm, SolveView};
    for (coefficients, count) in [
        ([1, -3, 0, 1], Some(3)),
        ([1, 1, 0, 1], Some(1)),
        ([2, -3, 0, 1], Some(2)),
        ([8, 0, 0, 1], Some(1)),
        ([1, 0, 1, 0], Some(0)),
        ([-2, 1, 0, 0], Some(1)),
        ([1, 0, 0, 0], Some(0)),
        ([0, 0, 0, 0], None),
    ] {
        let (mut fixture, request) =
            crate::testing::solve_blind::real_polynomial(&coefficients, SolveForm::AnyExact)?;
        let outcome = fixture.cas.run(&request);
        let crate::request::Outcome::Solved { answer, .. } = outcome else {
            return Err(format!("{coefficients:?}: {outcome:?}").into());
        };
        assert_eq!(answer.form, SolveForm::RealExact);
        let SolveView::SolutionSet(set) = fixture.cas.solve_view(&answer.set)? else {
            return Err("Real ground set".into());
        };
        assert_eq!(set.branches.len(), 1);
        let SolveView::SolutionBranch(branch) = fixture.cas.solve_view(&set.branches[0])? else {
            return Err("Real ground branch".into());
        };
        match (&branch.kind, count) {
            (SolutionBranchKind::Finite { tuples }, Some(expected)) => {
                assert_eq!(tuples.len(), expected);
            }
            (SolutionBranchKind::Empty, Some(0)) | (SolutionBranchKind::Image { .. }, None) => {}
            _ => return Err("Real ground result shape".into()),
        }
    }
    Ok(())
}

/// Both the D convention and the zero stratum are part of the checked total partition.
/// ´test:crate:real-discriminant-stratum-mutations´
#[test]
fn real_discriminant_stratum_mutations() -> crate::testing::common::TestResult {
    crate::testing::solve_blind::real_stratum_mutations()
}

/// A different declaration field, base region or original denominator is a different expected theorem.
/// ´test:crate:real-fresh-expected-claim-refusals´
#[test]
fn real_fresh_expected_claim_refusals() -> crate::testing::common::TestResult {
    crate::testing::solve_blind::real_expected_refusals()
}
