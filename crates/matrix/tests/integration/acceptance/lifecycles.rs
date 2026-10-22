// SPDX-License-Identifier: AGPL-3.0-only
//! Public truss and resistor inputs compose through solve, sensitivity, covariance and publication.

use super::{oracle, support};
use crate::kernel_oracle::Rational;
use nrz_accuracy::BudgetDecision;
use nrz_bindings::Dimension;
use nrz_matrix::{MomentAssurance, SolveStage};

/// Truss input, physical shape, solve, implicit derivative and member-force covariance compose.
/// ´test:integration:integrated-truss-matches-exact-physical-oracles´
#[test]
fn integrated_truss_matches_exact_physical_oracles() -> support::ResultOf {
    let case = support::truss()?;
    metadata(&case, "truss")?;
    assert_eq!(case.storage(), [30, 141]);
    assert_eq!(case.factors, 1);
    assert!(case.refusals.is_empty());
    assert_eq!(case.samples.len(), 2);
    assert_eq!(case.provenance[0], 4);
    let displacement = &case.samples[0];
    let matrix = [1000.5, 0.5, 0.5, 1.5];
    let rhs = [1.0, 0.001];
    let nominal = crate::solve_public::exact(&matrix, &rhs, 2)?;
    let exact = oracle::fractions(&[(2999, 3_001_000), (1001, 3_001_000)])?;
    oracle::values(displacement, &nominal, &exact)?;
    assert_eq!(
        displacement.quantity.dimension(),
        &Dimension::integers([1, 0, 0, 0, 0, 0, 0])?
    );
    solve_costs(displacement, 1536, 14_272, 6)?;
    let jacobian = oracle::jacobian(&matrix, &rhs, 2)?;
    oracle::parameter(&displacement.parameters[1], &jacobian)?;
    oracle::parameter(&displacement.parameters[2], &jacobian)?;
    let response = crate::solve_public::exact(&matrix, &[1.0, 1.0], 2)?;
    oracle::parameter(
        &displacement.parameters[0],
        &oracle::outer(&response, Rational::new(1, 16)?)?,
    )?;
    oracle::parameter(&displacement.parameters[3], &nominal)?;
    assert_eq!(
        displacement
            .parameters
            .iter()
            .map(|p| p.centers.len())
            .collect::<Vec<_>>(),
        [4, 12, 12, 2]
    );
    forces(&case.samples[1], &displacement.centers, &exact, &response)
}

/// The two orthogonal force observables use the same load source and scale both covariance sides.
fn forces(
    sample: &support::Sample,
    represented: &[f64],
    exact: &[Rational],
    response: &[Rational],
) -> support::ResultOf {
    let scale = |values: &[Rational]| -> support::ResultOf<Vec<Rational>> {
        Ok(vec![
            values[0].multiply(Rational::new(1000, 1)?)?,
            values[1],
        ])
    };
    let nominal = oracle::represented(represented)?;
    oracle::values(sample, &scale(&nominal)?, &scale(exact)?)?;
    oracle::parameter(
        &sample.parameters[0],
        &oracle::outer(&scale(response)?, Rational::new(1, 16)?)?,
    )?;
    assert_eq!(
        sample.quantity.dimension(),
        &Dimension::integers([1, 1, -2, 0, 0, 0, 0])?
    );
    assert_eq!(sample.statistical_work, 1472);
    assert_eq!(sample.kernel.as_ref().ok_or("force map")?.work(), 74);
    assert_eq!(sample.parameters[0].shape, [2, 2]);
    Ok(())
}

/// All gain outputs and every cross-point block agree with the exact common-source ladder.
/// ´test:integration:integrated-resistor-retains-physical-cross-point-covariance´
#[test]
fn integrated_resistor_retains_physical_cross_point_covariance() -> support::ResultOf {
    let case = support::resistor()?;
    metadata(&case, "resistor")?;
    assert_eq!(case.storage(), [224, 3705]);
    assert_eq!(case.factors, 1);
    assert!(case.refusals.is_empty());
    assert_eq!(case.samples.len(), 18);
    assert_eq!(case.provenance[0], 36);
    ladder_costs(&case);
    let base = oracle::fractions(&[(1, 1), (5, 13), (2, 13), (1, 13), (-8, 13)])?;
    for (point, gain) in [1_i128, 2, 3].into_iter().enumerate() {
        ladder_point(&case, point, gain, &base)?;
    }
    cross_and_mean(&case, &base)
}

/// Every map and solve retains its own complete deterministic and statistical phase allowance.
fn ladder_costs(case: &support::Case) {
    let work = case
        .samples
        .iter()
        .filter_map(|sample| sample.kernel.as_ref().map(nrz_matrix::KernelEvidence::work))
        .collect::<Vec<_>>();
    assert_eq!(
        work,
        [
            147, 243, 293, 77, 147, 243, 293, 77, 147, 243, 293, 77, 40, 40, 365
        ]
    );
    let statistics = case
        .samples
        .iter()
        .map(|sample| sample.statistical_work)
        .collect::<Vec<_>>();
    assert_eq!(
        statistics,
        [
            1056, 74_424, 11_752, 10_112, 3080, 1056, 74_424, 11_752, 10_112, 3080, 1056, 74_424,
            11_752, 10_112, 3080, 15_672, 15_672, 12_872
        ]
    );
}

/// Check one gain against the common exact response and every local and composed derivative.
fn ladder_point(
    case: &support::Case,
    point: usize,
    gain: i128,
    base: &[Rational],
) -> support::ResultOf {
    let samples = &case.samples[5 * point..5 * point + 5];
    let sample = &samples[1];
    let exact = base
        .iter()
        .map(|value| value.multiply(Rational::new(gain, 1)?))
        .collect::<support::ResultOf<Vec<_>>>()?;
    let reduced = vec![exact[3], exact[2], exact[1]];
    oracle::values(sample, &reduced, &reduced)?;
    oracle::parameter(
        &sample.parameters[0],
        &oracle::outer(&reduced, Rational::new(1, 10000)?)?,
    )?;
    oracle::parameter(&sample.parameters[3], &reduced)?;
    solve_costs(sample, 4608, 74_424, 12)?;
    ladder_derivatives(sample, u8::try_from(gain)?)?;
    assert_eq!(
        sample
            .solve
            .as_ref()
            .ok_or("solve")?
            .factor_use()
            .ok_or("factor")?
            .reused(),
        point != 0
    );
    assert_eq!(
        sample
            .parameters
            .iter()
            .map(|p| p.centers.len())
            .collect::<Vec<_>>(),
        [9, 36, 36, 3]
    );
    let nominal = reconstructed(&oracle::represented(&sample.centers)?)?;
    oracle::values(&samples[2], &nominal, &exact)?;
    oracle::parameter(
        &samples[2].parameters[0],
        &oracle::outer(&exact, Rational::new(1, 10000)?)?,
    )?;
    physical_outputs(&samples[3..], &samples[2].centers, &exact)?;
    Ok(())
}

/// Fixed exact circuit elimination and reconstruction are checked independently of production maps.
fn reconstructed(nodes: &[Rational]) -> support::ResultOf<Vec<Rational>> {
    Ok(vec![
        nodes[2]
            .multiply(Rational::new(3, 1)?)?
            .subtract(nodes[1])?,
        nodes[2],
        nodes[1],
        nodes[0],
        nodes[1].subtract(nodes[2].multiply(Rational::new(2, 1)?)?)?,
    ])
}

/// Physical block restoration retains voltage/current quantities and their paired covariance units.
fn physical_outputs(
    samples: &[support::Sample],
    represented: &[f64],
    exact: &[Rational],
) -> support::ResultOf {
    let nominal = oracle::represented(represented)?;
    oracle::values(&samples[0], &nominal[..4], &exact[..4])?;
    oracle::values(&samples[1], &nominal[4..], &exact[4..])?;
    assert_eq!(
        samples[0].quantity.dimension(),
        &Dimension::integers([2, 1, -3, -1, 0, 0, 0])?
    );
    assert_eq!(
        samples[1].quantity.dimension(),
        &Dimension::integers([0, 0, 0, 1, 0, 0, 0])?
    );
    assert_eq!(
        samples[0].parameters[0].dimensions,
        vec![Dimension::integers([4, 2, -6, -2, 0, 0, 0])?; 16]
    );
    assert_eq!(
        samples[1].parameters[0].dimensions,
        vec![Dimension::integers([0, 0, 0, 2, 0, 0, 0])?]
    );
    assert_eq!(
        [samples[0].statistical_work, samples[1].statistical_work],
        [10_112, 3080]
    );
    Ok(())
}

/// The complete 15 by 15 joint covariance costs 225 entries, plus separate evaluation regions.
fn cross_and_mean(case: &support::Case, base: &[Rational]) -> support::ResultOf {
    assert_eq!(case.cross.len(), 10);
    assert_eq!(
        case.cross[..9]
            .iter()
            .map(|p| p.centers.len())
            .sum::<usize>(),
        225
    );
    for (index, block) in case.cross[..9].iter().enumerate() {
        let multiplier = i128::try_from((index / 3 + 1) * (index % 3 + 1))?;
        oracle::parameter(
            block,
            &oracle::outer(base, Rational::new(multiplier, 10000)?)?,
        )?;
    }
    let physical = case.cross.last().ok_or("physical cross")?;
    assert_eq!(physical.shape, [4, 1]);
    assert!(crate::statistics_oracle::covered(
        Rational::new(-3, 42250)?,
        physical.centers[1],
        physical.errors[1]
    )?);
    assert_eq!(
        physical.dimensions,
        vec![Dimension::integers([2, 1, -3, 0, 0, 0, 0])?; 4]
    );
    let mean = case.samples.last().ok_or("mean")?;
    let expected = base
        .iter()
        .map(|value| value.multiply(Rational::new(2, 1)?))
        .collect::<support::ResultOf<Vec<_>>>()?;
    let sum = case.samples.get(16).ok_or("ordered sum")?;
    let nominal = oracle::represented(&sum.centers)?
        .iter()
        .map(|value| value.multiply(Rational::new(1, 3)?))
        .collect::<support::ResultOf<Vec<_>>>()?;
    oracle::values(mean, &nominal, &expected)?;
    oracle::parameter(
        &mean.parameters[0],
        &oracle::outer(base, Rational::new(4, 10000)?)?,
    )?;
    assert_eq!(mean.statistical_work, 12_872);
    let wrong = base[1]
        .multiply(base[1])?
        .multiply(Rational::new(14, 90000)?)?;
    assert!(!crate::statistics_oracle::covered(
        wrong,
        mean.parameters[0].centers[6],
        mean.parameters[0].errors[6]
    )?);
    Ok(())
}

/// Exact counts refer to public admission allowances and complete logical derivative columns.
fn solve_costs(
    sample: &support::Sample,
    work: usize,
    statistics: usize,
    directions: usize,
) -> support::ResultOf {
    assert_eq!(sample.stage, Some(SolveStage::Published));
    assert_eq!(sample.assurance, Some(MomentAssurance::AffineMoments));
    assert_eq!(sample.statistical_work, statistics);
    let proof = sample.solve.as_ref().ok_or("solve proof")?;
    assert!(proof.is_complete());
    assert_eq!(proof.work(), work);
    assert_eq!(proof.sensitivities().len(), directions);
    assert_eq!(
        proof.budget().ok_or("budget")?.decision(),
        BudgetDecision::Met
    );
    assert_eq!(proof.arithmetic_revision(), 1);
    for (index, direction) in proof.sensitivities().iter().enumerate() {
        assert_eq!(direction.direction(), index);
        assert!(direction.residual_norm().is_finite() && direction.radius().is_finite());
    }
    Ok(())
}

/// Environment assumptions and execution order remain inspectable independently of runtime values.
fn metadata(case: &support::Case, name: &str) -> support::ResultOf {
    assert_eq!(case.name, name);
    assert_eq!(
        case.timings.last().ok_or("total runtime observation")?.0,
        "total"
    );
    for sample in &case.samples {
        assert_ne!(sample.name, "");
        assert_eq!(
            sample.environment,
            "IEEE binary32/binary64; nearest ties to even; gradual underflow; scalar outward arithmetic revision 1"
        );
    }
    Ok(())
}

/// Independently eliminate each local differential and then compose the exact source gain.
fn ladder_derivatives(sample: &support::Sample, gain: u8) -> support::ResultOf {
    let operator = [2.0, -1.0, 0.0, -1.0, 3.0, -1.0, 0.0, -1.0, 3.0];
    let rhs = [0.0, 0.0, f64::from(gain)];
    let jacobian = oracle::jacobian(&operator, &rhs, 3)?;
    oracle::parameter(&sample.parameters[2], &jacobian)?;
    let mut composed = jacobian;
    for entry in composed.get_mut(27..).ok_or("RHS differential columns")? {
        *entry = entry.multiply(Rational::new(i128::from(gain), 1)?)?;
    }
    oracle::parameter(&sample.parameters[1], &composed)
}
