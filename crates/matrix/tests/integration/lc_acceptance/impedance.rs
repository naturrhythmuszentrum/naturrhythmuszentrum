// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic original L/C declarations produce every driven row after one retained spectrum.

mod fixture;
mod lifecycle;
mod resources;
use crate::integration::axis_phase::fixture as axis;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{
    AxisPhaseImpedance, Budget, Descriptor, Kind, LcDrivenImpedance, Measure, ReferenceDefinition,
    Role,
    readings::{EntryMeaning, algebra::Rational},
};
use nrz_matrix::{
    AxisPhasePlan, Heap, LcImpedancePublication, LcSpectralBatch, LcSpectralBatchLimits,
    LcSpectralSchema, LcSpectrum, PublishedLcImpedance,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, NumericLiteral, Scalar};

/// One declared deterministic experiment with a shared original L/C binding.
struct Case<P: Scalar> {
    /// Numerical source and output owner.
    heap: Heap,
    /// Original physical L/C source plan shared by every point.
    spectrum: LcSpectrum<P>,
    /// Actual one-attempt invariant schedule with stable caller row identities.
    batch: LcSpectralBatch<P>,
    /// All exact driven output symbols and equations precede execution.
    rows: Vec<fixture::Row<P>>,
    /// Point-varying producer declarations, each retaining the same bound invariant owner.
    outputs: Vec<LcImpedancePublication<P>>,
    /// Original exact sign case and rational enclosure for each point.
    axes: Vec<axis::Fixture>,
    /// Both phase alternatives share one original certified constant region.
    phases: Vec<AxisPhasePlan<P>>,
    /// Original admitted arithmetic environment.
    context: ArithmeticContext,
}

/// Declare all original exact models, finite keys and complete output groups before the root attempt.
fn setup<P: Scalar + 'static>(uncertain: bool) -> TestResult<Case<P>> {
    let context = ArithmeticContext::new()?;
    let physical = crate::spectral_testing::Fixture::new(["5", "1/10", "1/2000"])?;
    let (mut heap, inputs) = inputs::<P>(&physical, uncertain, &context)?;
    let schema = LcSpectralSchema::new(
        &physical.graph,
        physical.law.clone(),
        LiteralLimits::default(),
        &mut ledger()?,
    )?;
    let spectrum = schema.bind(&heap, inputs, &mut ledger()?)?;
    let batch = LcSpectralBatch::prepare(
        &schema,
        &heap,
        &[
            (41, spectrum.clone()),
            (9, spectrum.clone()),
            (73, spectrum.clone()),
        ],
        LcSpectralBatchLimits {
            points: 3,
            bindings: 1,
            key_bytes: 4096,
        },
        &context,
        &mut ledger()?,
    )?;
    let pi = crate::integration::pi_conversion::pi::<P>(&context, &mut ledger()?)?;
    let mut rows = Vec::new();
    let mut outputs = Vec::new();
    let mut axes = Vec::new();
    let mut phases = Vec::new();
    for (frequency, reactance) in [(1000, -75), (2000, 0), (4000, 75)] {
        let row = fixture::Row::new(
            &mut heap,
            reactance,
            &mut crate::phasor_solve_testing::resources::fresh_budget()?,
        )
        .map_err(|cause| format!("row {frequency} original declaration setup: {cause:?}"))?;
        let original = axis::fixture(frequency)?;
        let association = AxisPhaseImpedance::new(
            original.case.clone(),
            row.definition.clone(),
            &mut ledger()?,
        )?;
        let definition = LcDrivenImpedance::new(
            physical.law.clone(),
            [
                Rational::parse("1/40", &mut ledger()?)?,
                Rational::parse("1/100000", &mut ledger()?)?,
            ],
            (
                meaning(Kind::angular_frequency()?)?,
                Rational::from_ratio(frequency, 1, &mut ledger()?)?,
            ),
            meaning(Kind::Angle)?,
            association,
            &mut ledger()?,
        )?;
        outputs.push(LcImpedancePublication::new(
            spectrum.clone(),
            definition,
            std::rc::Rc::clone(&row.graph),
            row.impedance.clone(),
            row.radii.clone(),
            LiteralLimits::new(32, 32, 1024),
            &mut ledger()?,
        )?);
        phases.push(AxisPhasePlan::new(
            original.case.clone(),
            &pi,
            LiteralLimits::new(32, 32, 1024),
            &context,
            &mut ledger()?,
        )?);
        rows.push(row);
        axes.push(original);
    }
    Ok(Case {
        heap,
        spectrum,
        batch,
        rows,
        outputs,
        axes,
        phases,
        context,
    })
}

/// Declare exact source coordinates with separate numerical-only radii in the original physical frame.
fn inputs<P: Scalar + 'static>(
    physical: &crate::spectral_testing::Fixture,
    uncertain: bool,
    context: &ArithmeticContext,
) -> TestResult<(Heap, [nrz_matrix::ReadingBinding<P>; 2])> {
    let mut values = Vec::new();
    for text in ["1/40", "1/100000"] {
        let decoded =
            NumericLiteral::new(text, LiteralLimits::new(32, 32, 1024))?.decode::<P>(context)?;
        let error = decoded
            .enclosure()
            .displacement_from(decoded.center(), context)?
            .get();
        values.push([
            decoded.center().get(),
            if uncertain { error + error } else { error },
            error,
        ]);
    }
    physical.import(values.try_into().map_err(|_| "two original inputs")?)
}

/// The exact one-radian and angular-frequency references retain distinct named physical kinds.
fn meaning(kind: Kind) -> TestResult<EntryMeaning> {
    let descriptor = Descriptor::new(kind, Measure::coherent(), Role::Difference)?;
    let reference = ReferenceDefinition::unit("original driven coherent reference", &descriptor)?;
    Ok(EntryMeaning::new(
        descriptor,
        reference,
        None,
        &mut ledger()?,
    )?)
}

/// Every point-varying publication consumes the actual root already produced by this same schedule.
fn produce<P: Scalar + 'static>(
    case: &mut Case<P>,
    budget: &mut Budget,
) -> TestResult<Vec<PublishedLcImpedance<P>>> {
    case.batch.execute(&case.heap, &case.context, budget)?;
    let mut result = Vec::new();
    for (row, output) in case.outputs.iter().enumerate() {
        let invariant = case.batch.result(row, &case.heap, &case.context, budget)?;
        result.push(case.heap.publish_lc_impedance_reusing(
            output,
            invariant,
            &case.context,
            budget,
        )?);
    }
    Ok(result)
}

/// Original exact expression production precedes all sign/current continuations in both formats.
/// ´test:integration:lc-original-driven-impedance-table´
#[test]
fn lc_original_driven_impedance_table() -> TestResult {
    nominal::<f32>()?;
    nominal::<f64>()
}

/// Independent reactance/current oracles cover all rows; the exact resonant block remains successful.
fn nominal<P: Scalar + 'static>() -> TestResult {
    let mut case = setup::<P>(false)?;
    let mut budget = crate::phasor_solve_testing::resources::fresh_budget()?;
    let impedance = produce(&mut case, &mut budget)?;
    let mut decisions = Vec::new();
    for (row, ((output, expected), bounds)) in impedance
        .iter()
        .zip([-75, 0, 75])
        .zip([["-76", "-74"], ["0", "0"], ["74", "76"]])
        .enumerate()
    {
        let values = case
            .heap
            .reading_entries(&output.readings().bindings()[0])?;
        let radius = case
            .heap
            .reading_companion_entries(&output.readings().bindings()[2], 2)?;
        assert!(
            crate::kernel_oracle::Rational::new(expected, 1)?
                .covered(values[1].to_f64(), radius[0].to_f64())?
        );
        if expected == 0 {
            assert_eq!(values, [P::ZERO; 4]);
        }
        assert!(
            output.readings().bindings()[0]
                .lc_driven_impedance()
                .is_some()
        );
        let frozen = case.heap.freeze_phasor_axis_phase_input(
            output.definition().impedance(),
            &fixture::operand(output.readings().bindings(), &mut budget)?,
            &mut budget,
        )?;
        let prepared = case.heap.prepare_axis_phase(
            frozen,
            &mut case.axes[row].case.attempts(1),
            &axis::enclosure(&case.axes[row], bounds[0], bounds[1])?,
            &case.phases[row],
            &case.context,
            &mut budget,
        )?;
        decisions.push(case.heap.publish_axis_phase(prepared, &mut budget)?);
    }
    for (row, (source, decision)) in impedance.iter().zip(&decisions).enumerate() {
        let current = case.rows[row].current(source.readings().bindings(), &mut budget)?;
        let result = case
            .heap
            .publish_phasor_solve(&current, &case.context, &mut budget);
        if row == 1 {
            assert!(matches!(
                result,
                Err(nrz_matrix::PhasorError::ZeroImpedance)
            ));
            assert!(matches!(
                decision.phase(&case.heap, &case.context, &mut budget),
                Err(nrz_matrix::LiveDecisionError::UndefinedPhase)
            ));
        } else {
            let result = result?;
            let value = case
                .heap
                .reading_entries(&result.readings().bindings()[0])?[1];
            let radius = result.evaluation().totals(&case.heap, &case.context)?[1];
            let sign = if row == 0 { 1 } else { -1 };
            assert!(
                crate::reciprocal_testing::oracle::Rational::new(sign, 25)?
                    .covered(value.to_f64(), radius.to_f64())?
            );
            decision.phase(&case.heap, &case.context, &mut budget)?;
        }
    }
    let root = case
        .batch
        .result(1, &case.heap, &case.context, &mut budget)?;
    assert!(
        crate::reciprocal_testing::oracle::Rational::new(2000, 1)?.covered(
            root.imaginary_rates(&case.heap, &case.context)?[0]
                .center
                .to_f64(),
            root.imaginary_rates(&case.heap, &case.context)?[0]
                .total
                .to_f64()
        )?
    );
    Ok(())
}
