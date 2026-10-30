// SPDX-License-Identifier: AGPL-3.0-only
//! Actual scalar dyadic records become an independently checked rational decision input.

use super::decision_fixture::{fixture, ledger};
use crate::binding_testing::TestResult;
use nrz_bindings::Budget;
use nrz_bindings::readings::algebra::{
    Rational,
    decision::{EnclosureOrigin, ScalarKernelCertificate, ScalarKernelLaw, ScalarKernelRecord},
    proof::Certificate,
};
use nrz_scalar::{
    ArithmeticContext, Dyadic, KernelBudget, KernelCertificate, KernelEvaluation, KernelLaw,
    LiteralLimits,
};

/// Interpret only the exported signed integer coefficient and binary exponent.
fn exact(value: Dyadic, budget: &mut Budget) -> TestResult<Rational> {
    let coefficient = Rational::from_ratio(value.significand, 1, budget)?;
    let power = Rational::from_ratio(2, 1, budget)?.pow(i64::from(value.exponent), budget)?;
    Ok(coefficient.mul(&power, budget)?)
}

/// Retain a closed law and convert endpoints without a floating sum or difference.
fn record(
    value: KernelCertificate,
    law: ScalarKernelLaw,
    budget: &mut Budget,
) -> TestResult<ScalarKernelRecord> {
    Ok(ScalarKernelRecord {
        law,
        lower: exact(value.output.lower, budget)?,
        upper: exact(value.output.upper, budget)?,
    })
}

/// Construct actual reciprocal/product records for the existing rational selection fixture.
fn quotient() -> TestResult<[KernelCertificate; 4]> {
    let context = ArithmeticContext::new()?;
    let mut budget = KernelBudget::new(1050);
    let literals = LiteralLimits::new(32, 32, 128);
    let numerator = KernelEvaluation::<f64>::rational(51, 1, literals, &context, &mut budget)?;
    let denominator = KernelEvaluation::<f64>::rational(16, 1, literals, &context, &mut budget)?;
    let reciprocal = KernelEvaluation::reciprocal(denominator.enclosure(), &context, &mut budget)?;
    let product = KernelEvaluation::product(
        numerator.enclosure(),
        reciprocal.enclosure(),
        &context,
        &mut budget,
    )?;
    let records = [
        numerator.export(&mut budget)?,
        denominator.export(&mut budget)?,
        reciprocal.export(&mut budget)?,
        product.export(&mut budget)?,
    ];
    assert_eq!(budget.usage().work, 1050);
    assert_eq!(budget.usage().constructed, 4);
    assert_eq!(budget.usage().exported, 4);
    let KernelLaw::Reciprocal { input } = records[2].law else {
        return Err("missing reciprocal law".into());
    };
    assert_eq!(input, records[1].output);
    let KernelLaw::Product { left, right } = records[3].law else {
        return Err("missing product law".into());
    };
    assert_eq!(left, records[0].output);
    assert_eq!(right, records[2].output);
    Ok(records)
}

/// Exported floating evaluation records certify the exact rational count through the CAS facade.
/// ´test:integration:kernel-export-cas-selection´
/// ´claim:matrix:kernel-export-cas-selection´
#[test]
fn kernel_export_cas_selection() -> TestResult {
    let mut source = fixture(false, "3", "51/16")?;
    let values = quotient()?;
    let mut budget = ledger()?;
    let mut records = Vec::new();
    for value in &values[..2] {
        let KernelLaw::Rational {
            numerator,
            denominator,
        } = value.law
        else {
            return Err("missing original rational law".into());
        };
        let law =
            ScalarKernelLaw::Rational(Rational::from_ratio(numerator, denominator, &mut budget)?);
        records.push(record(*value, law, &mut budget)?);
    }
    records.push(record(
        values[2],
        ScalarKernelLaw::Reciprocal { input: 1 },
        &mut budget,
    )?);
    records.push(record(
        values[3],
        ScalarKernelLaw::Product { left: 0, right: 2 },
        &mut budget,
    )?);
    let Certificate::IntegerSelection(request) =
        &mut source.steps.last_mut().ok_or("selection")?.certificate
    else {
        return Err("missing selection certificate".into());
    };
    let output = records.last().ok_or("missing product")?;
    request.lower = output.lower.clone();
    request.upper = output.upper.clone();
    request.origin = EnclosureOrigin::ScalarKernel(Box::new(ScalarKernelCertificate {
        version: 1,
        expression: request.target.clone(),
        context: request.context.clone(),
        input_family: request.input_family.clone(),
        records,
    }));
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut budget)?;
    let (_, checked) = source
        .frame
        .cas
        .integer_selection_evidence(&witness, &mut ledger()?)?;
    assert!(matches!(checked.origin, EnclosureOrigin::ScalarKernel(_)));
    let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
    let mut receiver = nrz_bindings::readings::algebra::Cas::new(ledger()?.limits().clone())?;
    let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
    assert_eq!(receiver.export_witness(&replayed, &mut ledger()?)?, bytes);
    Ok(())
}
