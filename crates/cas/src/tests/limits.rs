// SPDX-License-Identifier: AGPL-3.0-only
//! Complete receiving-policy meets and immutable store-policy access.

use crate::budget::Limits;
use crate::{Cas, Fault};

/// Every field alternates the restrictive operand, including all solving ceilings.
/// ´test:unit:limits-meet-complete´
#[test]
fn limits_meet_complete() -> Result<(), Fault> {
    let left = profile(100, 1_000);
    let right = profile(1_000, 100);
    let expected = profile(100, 100);
    left.validate()?;
    right.validate()?;
    expected.validate()?;
    let saved = (left.clone(), right.clone());
    assert_eq!(left.meet(&right), expected);
    assert_eq!(right.meet(&left), expected);
    let mut intersection = left.clone();
    intersection.intersect(&right);
    assert_eq!(intersection, expected);
    assert_eq!((&left, &right), (&saved.0, &saved.1));
    let cas = Cas::new(left.clone())?;
    assert_eq!(cas.limits(), &left);
    assert!(std::ptr::eq(cas.limits(), &raw const cas.limits));
    assert_eq!(cas.limits().meet(&right), expected);
    assert_eq!(cas.limits(), &left);
    Ok(())
}

/// Exhaustive construction makes an added policy field require an oracle update.
/// Offsets distinguish fields; the intermediate bound admits either component bound.
fn profile(even: u64, odd: u64) -> Limits {
    Limits {
        source_bytes: even + 1,
        name_bytes: odd + 2,
        original_occurrences: even + 3,
        depth: odd + 4,
        vertices: even + 5,
        edges: odd + 6,
        staged_vertices: even + 7,
        staged_edges: odd + 8,
        payload_bytes: even + 9,
        staged_payload_bytes: odd + 10,
        work: even + 11,
        arithmetic_steps: odd + 12,
        component_bits: even + 13,
        intermediate_bits: odd + 5000,
        literal_bytes: even + 15,
        decimal_exponent: odd + 16,
        integer_exponent: even + 17,
        polynomial_degree: odd + 18,
        polynomial_variables: even + 19,
        polynomial_terms: odd + 20,
        matrix_entries: even + 21,
        condition_atoms: odd + 22,
        condition_steps: even + 23,
        match_attempts: odd + 24,
        match_batch: even + 25,
        frontier_labels: odd + 26,
        queue_entries: even + 27,
        transitions: odd + 28,
        expansion_credits: even + 29,
        peak_expression_size: odd + 30,
        proof_nodes: even + 31,
        witness_bytes: odd + 32,
        output_bytes: even + 33,
        diagnostic_records: odd + 34,
        replay_work: even + 35,
        cost_quota: odd + 36,
        progress_quota: even + 37,
        case_arms: odd + 38,
        region_nodes: even + 39,
        region_cells: odd + 40,
        refinement_pairs: even + 41,
        local_contexts: odd + 42,
        hypothesis_depth: even + 43,
        boundary_nodes: odd + 44,
        jet_order: even + 45,
        jet_entries: odd + 46,
        delta_order: even + 47,
        test_nodes: odd + 48,
        pairing_steps: even + 49,
        pullback_zeros: odd + 50,
        coefficient_ops: even + 51,
        denominator_factors: odd + 52,
        coefficient_maps: even + 53,
        tower_levels: odd + 54,
        tower_degree: even + 55,
        tower_reductions: odd + 56,
        gcd_steps: even + 57,
        component_splits: odd + 58,
        orbit_permutations: even + 59,
        orbit_members: odd + 60,
        orbit_terms: even + 61,
        symmetric_steps: odd + 62,
        resolvent_depth: even + 63,
        resolvent_degree: odd + 64,
        selection_slots: even + 65,
        solution_branches: odd + 66,
        family_parameters: even + 67,
        solve_identities: odd + 68,
        linear_pivots: even + 69,
        isolation_steps: odd + 70,
        local_branches: even + 71,
    }
}
