// SPDX-License-Identifier: AGPL-3.0-only
//! Outward conversion of numerical parameters preserves their original mathematical target.

use crate::OperationError;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Precision, Scalar};

/// Convert the displayed candidate and both independently certified endpoints.
pub(crate) fn entry<P: Scalar + 'static, Q: Scalar + 'static>(
    center: P,
    truth: Enclosure<P>,
    context: &ArithmeticContext,
) -> Result<(Q, Enclosure<Q>), OperationError> {
    match (P::PRECISION, Q::PRECISION) {
        (Precision::F32, Precision::F64) => {
            let (center, truth) = same::<P, f32>(center, truth)?;
            let converted = nrz_scalar::widen(Finite::new(center)?, truth);
            same(converted.center().get(), converted.enclosure())
        }
        (Precision::F64, Precision::F32) => {
            let (center, truth) = same::<P, f64>(center, truth)?;
            let converted = nrz_scalar::narrow(Finite::new(center)?, truth, context)?;
            same(converted.center().get(), converted.enclosure())
        }
        (Precision::F32, Precision::F32) | (Precision::F64, Precision::F64) => same(center, truth),
    }
}
/// The sealed format bridge cannot reinterpret scalar bits or alter an endpoint.
fn same<P: Scalar + 'static, Q: Scalar + 'static>(
    center: P,
    truth: Enclosure<P>,
) -> Result<(Q, Enclosure<Q>), OperationError> {
    Ok((
        crate::conversion::same_format(center)?,
        Enclosure::new(
            crate::conversion::same_format(truth.lower().get())?,
            crate::conversion::same_format(truth.upper().get())?,
        )?,
    ))
}

#[cfg(test)]
mod tests {
    use crate::kernel_oracle::Rational;
    use crate::kernel_testing::TestResult;
    use nrz_scalar::{ArithmeticContext, Enclosure};

    /// Outward parameter conversion contains both old endpoints, independently of the new candidate.
    /// ´test:unit:statistical-precision-encloses-parameter-endpoints´
    #[test]
    fn statistical_precision_encloses_parameter_endpoints() -> TestResult {
        let original = 0.1_f64;
        let context = ArithmeticContext::new()?;
        let upper = original.next_up();
        let (_, truth) =
            super::entry::<f64, f32>(original, Enclosure::new(original, upper)?, &context)?;
        assert!(
            Rational::binary(f64::from(truth.lower().get()))?
                .at_most(Rational::binary(original)?)?
        );
        assert!(
            Rational::binary(upper)?.at_most(Rational::binary(f64::from(truth.upper().get()))?)?
        );
        Ok(())
    }
}
