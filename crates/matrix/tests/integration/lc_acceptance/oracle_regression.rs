// SPDX-License-Identifier: AGPL-3.0-only
//! The shared exact test oracle normalizes zero and dyadic mantissas without changing proof limits.

use crate::reading_testing::TestResult;
use crate::reciprocal_testing::oracle::Rational;

/// Signed zeros avoid powers entirely and trailing zeros preserve the default dyadic exponent reach.
/// ´test:integration:lc-acceptance-shared-oracle-zero-and-dyadic´
#[test]
fn lc_acceptance_shared_oracle_zero_and_dyadic() -> TestResult {
    for value in [0.0_f64, -0.0] {
        assert!(Rational::binary(value)?.covered(0.0, 0.0)?);
    }
    for (value, numerator) in [(0.25_f64, 1), (-0.25, -1)] {
        let decoded = Rational::binary(value)?;
        let expected = Rational::new(numerator, 4)?;
        assert!(decoded.at_most(&expected)?);
        assert!(expected.at_most(&decoded)?);
    }
    let boundary = f64::from_bits((1023_u64 - 256) << 52);
    assert!(Rational::binary(boundary)?.covered(boundary, 0.0)?);
    assert!(Rational::binary(f64::from_bits(1)).is_err());
    Ok(())
}
