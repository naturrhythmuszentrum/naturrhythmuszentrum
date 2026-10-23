// SPDX-License-Identifier: AGPL-3.0-only
//! Outward coefficient evaluation never turns represented points into symbolic premises.

use super::{
    AT, Heap, HeapError, ReadingBinding, ReciprocalDerivativeContract,
    ReciprocalDerivativeError as Error, program::Step,
};
use nrz_bindings::{Budget, readings::algebra::limits::BudgetKind};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar};

/// Represented operating point and its outward operator-coefficient partials.
type Evaluation<P> = (Vec<P>, Vec<Enclosure<P>>);

/// All constants are enclosed before any numerical input is read.
pub(crate) fn evaluate<P: Scalar + 'static>(
    heap: &Heap,
    contract: &ReciprocalDerivativeContract,
    inputs: &[ReadingBinding<P>],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Evaluation<P>, Error> {
    check(contract, inputs, budget)?;
    let program = &contract.0.program;
    let count = program
        .steps
        .len()
        .checked_add(program.outputs.len())
        .and_then(|value| value.checked_add(inputs.len()))
        .ok_or(HeapError::ExtentOverflow)?;
    let bytes = u64::try_from(count)
        .map_err(|_| HeapError::ExtentOverflow)?
        .checked_mul(256)
        .ok_or(HeapError::ExtentOverflow)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    let constants = constants::<P>(contract, context, budget)?;
    let point = point(heap, inputs, budget)?;
    let mut values = crate::binding::buffer(program.steps.len())?;
    for (index, step) in program.steps.iter().enumerate() {
        budget.work(1, AT)?;
        values.push(execute(
            step,
            constants[index],
            &point,
            &values,
            context,
            budget,
        )?);
    }
    let mut outputs = crate::binding::buffer(program.outputs.len())?;
    for &index in &program.outputs {
        outputs.push(*values.get(index).ok_or(Error::Identity)?);
    }
    Ok((point, outputs))
}

/// Literal scans and arithmetic receive only their prepaid finite caller allowance.
fn constants<P: Scalar>(
    contract: &ReciprocalDerivativeContract,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Vec<Option<Enclosure<P>>>, Error> {
    let mut constants = crate::binding::buffer(contract.0.program.steps.len())?;
    for step in &contract.0.program.steps {
        constants.push(if let Step::Constant(text) = step {
            let work = u64::try_from(text.len())
                .map_err(|_| HeapError::ExtentOverflow)?
                .checked_mul(12)
                .and_then(|value| value.checked_add(8))
                .ok_or(HeapError::ExtentOverflow)?;
            budget.work(work, AT)?;
            let policy = contract.0.literals;
            let limits = LiteralLimits::new(
                policy.max_digits(),
                policy.max_exponent(),
                policy.max_work().min(work),
            );
            Some(
                NumericLiteral::new(text, limits)?
                    .decode::<P>(context)?
                    .enclosure(),
            )
        } else {
            None
        });
    }
    Ok(constants)
}

/// Fixed inputs retain their original positivity requirement and version dependency.
fn point<P: Scalar + 'static>(
    heap: &Heap,
    inputs: &[ReadingBinding<P>],
    budget: &mut Budget,
) -> Result<Vec<P>, Error> {
    let mut point = crate::binding::buffer(inputs.len())?;
    for input in inputs {
        budget.work(1, AT)?;
        let values = heap.reading_entries(input)?;
        let [value] = values.as_slice() else {
            return Err(Error::Unsupported);
        };
        if *value <= P::ZERO {
            return Err(nrz_scalar::ArithmeticFailure::ZeroInDivisor.into());
        }
        point.push(*value);
    }
    Ok(point)
}

/// The exact original argument slots receive their own declared physical readings.
fn check<P: Scalar>(
    contract: &ReciprocalDerivativeContract,
    inputs: &[ReadingBinding<P>],
    budget: &mut Budget,
) -> Result<(), Error> {
    let definition = contract.exact().definition();
    if inputs.len() != definition.inputs().len() {
        return Err(Error::Identity);
    }
    for ((input, formal), meaning) in inputs
        .iter()
        .zip(definition.inputs())
        .zip(definition.frame().entries())
    {
        budget.work(1, AT)?;
        let reading = input.reading();
        if reading.symbol() != formal || reading.family().context() != definition.frame().context()
        {
            return Err(Error::Identity);
        }
        let [entry] = reading.entries() else {
            return Err(Error::Unsupported);
        };
        entry.check(meaning)?;
        let identity = reading.map().coefficients()
            == [nrz_bindings::readings::algebra::Rational::one()]
            && reading.map().offsets() == [nrz_bindings::readings::algebra::Rational::zero()];
        if !identity || input.adapter().entries().len() != 1 {
            return Err(Error::Unsupported);
        }
    }
    Ok(())
}

/// Every referenced index was admitted by the exact DAG compiler before numerical evaluation.
fn execute<P: Scalar>(
    step: &Step,
    constant: Option<Enclosure<P>>,
    point: &[P],
    values: &[Enclosure<P>],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Enclosure<P>, Error> {
    let get = |index: usize| values.get(index).copied().ok_or(Error::Identity);
    Ok(match *step {
        Step::Constant(_) => constant.ok_or(Error::Identity)?,
        Step::Input(index) => {
            Enclosure::singleton(Finite::new(*point.get(index).ok_or(Error::Identity)?)?)
        }
        Step::Neg(index) => get(index)?.negated(),
        Step::Add(left, right) => get(left)?.add(get(right)?, context)?,
        Step::Sub(left, right) => get(left)?.sub(get(right)?, context)?,
        Step::Mul(left, right) => get(left)?.mul(get(right)?, context)?,
        Step::Div(left, right) => get(left)?.div(get(right)?, context)?,
        Step::Pow(index, exponent) => power(get(index)?, exponent, context, budget)?,
    })
}

/// Binary powering has bounded logarithmic work even for a large admitted integer exponent.
fn power<P: Scalar>(
    mut base: Enclosure<P>,
    exponent: i64,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Enclosure<P>, Error> {
    let mut bits = exponent.unsigned_abs();
    let mut result = Enclosure::singleton(Finite::new(P::ONE)?);
    while bits != 0 {
        budget.work(2, AT)?;
        if bits & 1 != 0 {
            result = result.mul(base, context)?;
        }
        bits >>= 1;
        if bits != 0 {
            base = base.mul(base, context)?;
        }
    }
    if exponent < 0 {
        result = Enclosure::singleton(Finite::new(P::ONE)?).div(result, context)?;
    }
    Ok(result)
}
