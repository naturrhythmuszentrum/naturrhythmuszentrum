// SPDX-License-Identifier: AGPL-3.0-only
//! Original LC impedance blocks feed the predeclared consumers without a surrogate scalar import.

use super::{TestResult, contains, enclosure, fixture, ledger, scalar};
use nrz_bindings::{AxisPhaseImpedance, AxisPhaseOutcome, Budget};
use nrz_matrix::{
    AxisPhasePlan, Heap, HeapLimits, LiveDecisionError, PhasorError, PreparedAxisPhase,
    PublishedAxisPhase,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};

/// Actual fixed physical equation and source, beside the independent original sign declaration.
pub(super) struct BlockCase<P: Scalar> {
    /// Original target and correspondence hypothesis declared before numerical use.
    pub(super) source: super::fixture::Fixture,
    /// Existing original whole block, selected copies and all source companions.
    pub(super) physical: crate::phasor_solve_testing::Case<P>,
    /// Predeclared exact connection to that original equation's fixed impedance.
    declaration: AxisPhaseImpedance,
    /// Complete original copy inventory prepared before any consumer attempt.
    operand: nrz_matrix::PhasorOperand<P>,
    /// One shared certified pi region for both branches.
    pub(super) plan: AxisPhasePlan<P>,
    /// Original arithmetic proof revision.
    pub(super) context: ArithmeticContext,
}

impl<P: Scalar + 'static> BlockCase<P> {
    /// The fixed physical map and original sign expression are declared independently.
    pub(super) fn new(
        frequency: i64,
        reactance: i64,
        radius: &str,
        rounded: bool,
    ) -> TestResult<Self> {
        let source = fixture(frequency)?;
        let physical = crate::phasor_solve_testing::fixed_case(
            reactance,
            scalar(radius)?,
            rounded,
            &mut crate::phasor_solve_testing::resources::fresh_budget()?,
        )?;
        let declaration = AxisPhaseImpedance::new(
            source.case.clone(),
            physical.definition.clone(),
            &mut ledger()?,
        )?;
        let context = ArithmeticContext::new()?;
        let pi = super::super::pi_conversion::pi::<P>(&context, &mut ledger()?)?;
        let plan = AxisPhasePlan::new(
            source.case.clone(),
            &pi,
            LiteralLimits::new(32, 32, 1024),
            &context,
            &mut ledger()?,
        )?;
        let operand = physical.operands(&mut ledger()?)?[0].clone();
        Ok(Self {
            source,
            physical,
            declaration,
            operand,
            plan,
            context,
        })
    }
    /// Every copied center and radius is captured only after its complete exact association matches.
    pub(super) fn prepare(
        &self,
        lower: &str,
        upper: &str,
        budget: &mut Budget,
    ) -> TestResult<PreparedAxisPhase<P>> {
        let frozen = self.physical.heap.freeze_phasor_axis_phase_input(
            &self.declaration,
            &self.operand,
            budget,
        )?;
        Ok(self
            .physical
            .heap
            .prepare_axis_phase(
                frozen,
                &mut self.source.case.attempts(1),
                &enclosure(&self.source, lower, upper)?,
                &self.plan,
                &self.context,
                budget,
            )
            .map_err(|cause| format!("original block preparation [{lower},{upper}]: {cause:?}"))?)
    }
    /// Publication adds only one dependent external authority to the existing original source.
    pub(super) fn publish(
        &self,
        lower: &str,
        upper: &str,
        budget: &mut Budget,
    ) -> TestResult<PublishedAxisPhase<P>> {
        Ok(self
            .physical
            .heap
            .publish_axis_phase(self.prepare(lower, upper, budget)?, budget)?)
    }
}

/// Both stored reactance copies keep their actual rounding errors in both scalar formats.
/// ´test:integration:lc-axis-phase-original-block-twins´
#[test]
fn lc_axis_phase_original_block_twins() -> TestResult {
    twins::<f32>()?;
    twins::<f64>()
}

/// Separate rounded signed copies cannot be replaced by one preferred center.
fn twins<P: Scalar + 'static>() -> TestResult {
    for (frequency, reactance, lower, upper, expected) in [
        (1000, -75, "-76", "-74", AxisPhaseOutcome::Negative),
        (4000, 75, "74", "76", AxisPhaseOutcome::Positive),
    ] {
        for rounded in [false, true] {
            let case = BlockCase::<P>::new(frequency, reactance, "0.25", rounded)?;
            let usage = case.physical.heap.provenance_usage();
            let published = case.publish(lower, upper, &mut ledger()?)?;
            assert_eq!(case.physical.heap.provenance_usage(), usage);
            assert_eq!(
                published
                    .current(&case.physical.heap, &case.context, &mut ledger()?)?
                    .outcome(),
                expected
            );
            assert!(published.inputs().phasor().is_some());
            assert_eq!(published.inputs().operands().len(), 0);
            let phase = published.phase(&case.physical.heap, &case.context, &mut ledger()?)?;
            let positive = if expected == AxisPhaseOutcome::Negative {
                phase.negated()
            } else {
                phase
            };
            contains(
                positive,
                super::super::pi_conversion::oracle::factor(1, 2, false, 56)?,
            )?;
            assert!(
                published
                    .current(&case.physical.heap, &case.context, &mut ledger()?)?
                    .reciprocal()
                    .ok_or("original nonzero")?
                    .is_discharged()
            );
            assert!(
                published
                    .reciprocal(&case.physical.heap, &case.context, &mut ledger()?)
                    .is_ok()
            );
        }
    }
    Ok(())
}

/// Zero and unresolved full boxes leave original impedance readable and current guards unchanged.
/// ´test:integration:lc-axis-phase-original-block-undefined-undecided´
#[test]
fn lc_axis_phase_original_block_undefined_undecided() -> TestResult {
    unresolved::<f32>()?;
    unresolved::<f64>()
}

/// The exact equation's fixed nonzero fact cannot authorize a zero-containing numerical divisor.
fn unresolved<P: Scalar + 'static>() -> TestResult {
    for (frequency, reactance, radius, lower, upper, expected) in [
        (2000, 0, "0", "0", "0", AxisPhaseOutcome::Undefined),
        (2000, 0, "0.25", "-1/4", "1/4", AxisPhaseOutcome::Undecided),
        (1000, -75, "76", "-152", "2", AxisPhaseOutcome::Undecided),
        (4000, 75, "76", "-2", "152", AxisPhaseOutcome::Undecided),
    ] {
        let mut case = BlockCase::<P>::new(frequency, reactance, radius, false)?;
        let original = case
            .physical
            .heap
            .reading_entries(&case.physical.bindings[0])?;
        let published = case.publish(lower, upper, &mut ledger()?)?;
        assert_eq!(
            published
                .current(&case.physical.heap, &case.context, &mut ledger()?)?
                .outcome(),
            expected
        );
        assert!(
            published
                .phase(&case.physical.heap, &case.context, &mut ledger()?)
                .is_err()
        );
        assert!(
            published
                .reciprocal(&case.physical.heap, &case.context, &mut ledger()?)
                .is_err()
        );
        let solve = case
            .physical
            .publication(&mut crate::phasor_solve_testing::resources::fresh_budget()?)?;
        let result = case.physical.heap.publish_phasor_solve(
            &solve,
            &case.context,
            &mut crate::phasor_solve_testing::resources::fresh_budget()?,
        );
        if expected == AxisPhaseOutcome::Undefined {
            assert!(matches!(result, Err(PhasorError::ZeroImpedance)));
        } else if reactance == 0 {
            assert!(matches!(result, Err(PhasorError::ReactanceNotSeparated)));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(
            case.physical
                .heap
                .reading_entries(&case.physical.bindings[0])?,
            original
        );
        assert!(
            published
                .current(&case.physical.heap, &case.context, &mut ledger()?)
                .is_ok()
        );
    }
    Ok(())
}

/// Actual source and either radius-only mutation retire both continuations together.
/// ´test:integration:lc-axis-phase-original-block-retirement´
#[test]
fn lc_axis_phase_original_block_retirement() -> TestResult {
    retirement::<f32>()?;
    retirement::<f64>()
}

/// Equal center bytes are insufficient, and transient foreign-heap probes preserve current authority.
fn retirement<P: Scalar + 'static>() -> TestResult {
    for channel in [None, Some(0), Some(1)] {
        let mut case = BlockCase::<P>::new(1000, -75, "0.25", channel.is_some())?;
        let published = case.publish("-76", "-74", &mut ledger()?)?;
        let foreign = Heap::new(HeapLimits::default())?;
        assert!(
            published
                .phase(&foreign, &case.context, &mut ledger()?)
                .is_err()
        );
        assert!(
            published
                .phase(&case.physical.heap, &case.context, &mut ledger()?)
                .is_ok()
        );
        let binding = &case.physical.bindings[if channel.is_some() { 3 } else { 0 }];
        if let Some(channel) = channel {
            let parameters = binding
                .companions()
                .iter()
                .enumerate()
                .filter(|(_, companion)| {
                    companion
                        .targets()
                        .iter()
                        .any(|target| target.reading() == binding.reading())
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            assert_eq!(parameters.len(), 2);
            let original = case
                .physical
                .heap
                .reading_companion_entries(binding, parameters[channel])?;
            case.physical.heap.replace_reading_companion_entries(
                binding,
                parameters[channel],
                &original,
                &mut ledger()?,
            )?;
        } else {
            let original = case.physical.heap.reading_entries(binding)?;
            case.physical
                .heap
                .replace_reading_entries(binding, &original, &mut ledger()?)?;
        }
        assert!(
            published
                .phase(&case.physical.heap, &case.context, &mut ledger()?)
                .is_err()
        );
        assert!(matches!(
            published.reciprocal(&case.physical.heap, &case.context, &mut ledger()?),
            Err(LiveDecisionError::Retired)
        ));
    }
    Ok(())
}

/// Original varying equations, substituted targets and voltage blocks cannot supply impedance authority.
/// ´test:integration:lc-axis-phase-original-block-refusals´
#[test]
fn lc_axis_phase_original_block_refusals() -> TestResult {
    let case = BlockCase::<f64>::new(1000, -75, "0.25", false)?;
    let other = fixture(4000)?;
    assert!(
        AxisPhaseImpedance::new(other.case, case.physical.definition.clone(), &mut ledger()?)
            .is_err()
    );
    let varying = crate::phasor_solve_testing::case(
        -75.0_f64,
        0.25,
        false,
        &mut crate::phasor_solve_testing::resources::fresh_budget()?,
    )?;
    assert!(
        AxisPhaseImpedance::new(case.source.case.clone(), varying.definition, &mut ledger()?)
            .is_err()
    );
    let operands = case.physical.operands(&mut ledger()?)?;
    assert!(
        case.physical
            .heap
            .freeze_phasor_axis_phase_input(&case.declaration, &operands[1], &mut ledger()?)
            .is_err()
    );
    let broad = BlockCase::<f64>::new(1000, -75, "76", false)?;
    let original = broad
        .source
        .case
        .attempts(1)
        .attempt(&enclosure(&broad.source, "-76", "-74")?, &mut ledger()?)?;
    assert!(
        original
            .reciprocal()
            .ok_or("original nonzero discharge")?
            .is_discharged()
    );
    assert!(broad.publish("-76", "-74", &mut ledger()?).is_err());
    Ok(())
}

/// Undefined phase and refused current leave an already published independent spectrum available.
/// ´test:integration:lc-axis-phase-preserves-independent-spectrum´
#[test]
fn lc_axis_phase_preserves_independent_spectrum() -> TestResult {
    independent::<f32>()?;
    independent::<f64>()
}

/// The autonomous root is evaluated once; the phase consumer retains only the driven impedance.
fn independent<P: Scalar + 'static>() -> TestResult {
    let mut case = BlockCase::<P>::new(2000, 0, "0", false)?;
    let spectral = crate::spectral_testing::Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let sources = spectral.import_into(
        &mut case.physical.heap,
        [
            [scalar("0.25")?, scalar("0.015625")?, P::ZERO],
            [scalar("0.0625")?, scalar("0.00390625")?, P::ZERO],
        ],
        &mut ledger()?,
    )?;
    let plan = spectral.plan(sources, &mut ledger()?)?;
    let publication = spectral.publication(plan, &mut ledger()?)?;
    let spectrum =
        case.physical
            .heap
            .publish_lc_spectrum(&publication, &case.context, &mut ledger()?)?;
    let before = spectrum.reading(0, &case.physical.heap, &case.context, &mut ledger()?)?[1];
    let phase = case.publish("0", "0", &mut ledger()?)?;
    assert!(matches!(
        phase.phase(&case.physical.heap, &case.context, &mut ledger()?),
        Err(LiveDecisionError::UndefinedPhase)
    ));
    let after = spectrum.reading(0, &case.physical.heap, &case.context, &mut ledger()?)?[1];
    assert_eq!(before.center, after.center);
    assert_eq!(before.total, after.total);
    assert_eq!(before.error, after.error);
    assert!(
        case.physical
            .heap
            .reading_entries(&case.physical.bindings[0])
            .is_ok()
    );
    Ok(())
}
