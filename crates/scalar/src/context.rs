// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit environment admission with limited runtime probes.

use crate::{ArithmeticFailure, Scalar};

/// Admission of the documented floating environment assumptions.
///
/// The target is restricted to `x86_64-unknown-linux-gnu`. Each compiler/profile
/// must also pass the independent boundary corpus before use. Runtime probes
/// detect selected violations; they cannot establish that foreign code never
/// changes rounding or underflow behaviour. Keep the environment unchanged for
/// the lifetime of this context. Fast math and excess precision are unsupported.
#[derive(Debug)]
pub struct ArithmeticContext {
    /// Arithmetic proof revision recorded by the admitted context.
    revision: u32,
}

impl ArithmeticContext {
    /// Check the supported target and selected ties/gradual-underflow probes.
    ///
    /// # Errors
    /// Returns `ArithmeticFailure::UnsupportedEnvironment` if a check fails.
    pub fn new() -> Result<Self, ArithmeticFailure> {
        let target = cfg!(all(
            target_arch = "x86_64",
            target_os = "linux",
            target_env = "gnu"
        ));
        admit(target, probe::<f32>() && probe::<f64>())
    }

    /// Arithmetic proof revision for callers' environment records.
    #[must_use]
    pub const fn revision(&self) -> u32 {
        self.revision
    }
}

/// Keep target and probe admission inseparable from context construction.
const fn admit(target: bool, probes: bool) -> Result<ArithmeticContext, ArithmeticFailure> {
    if !target || !probes {
        return Err(ArithmeticFailure::UnsupportedEnvironment);
    }
    Ok(ArithmeticContext { revision: 1 })
}

/// Probe ties-to-even and gradual underflow without constant folding inputs.
fn probe<P: Scalar>() -> bool {
    let one = std::hint::black_box(P::ONE);
    let two = one + one;
    let tiny = std::hint::black_box(P::ZERO.next_up());
    let normal = std::hint::black_box(P::min_positive_normal());
    let half_ulp = (one.next_up() - one) / two;
    (one + half_ulp).same_bits(one)
        && (one.next_up() + half_ulp).same_bits(one.next_up().next_up())
        && (tiny + tiny) > tiny
        && (tiny / two).same_bits(P::ZERO)
        && ((normal / two) * two).same_bits(normal)
        && (normal - normal.next_down()).same_bits(tiny)
        && (two * two).sqrt().same_bits(two)
}

#[cfg(test)]
mod tests {
    use super::admit;
    use crate::ArithmeticFailure;

    /// Private admission cannot bypass either target or environment refusal.
    /// ´test:unit:environment-admission-refuses´
    #[test]
    fn environment_admission_refuses() {
        for (target, probes) in [(false, false), (false, true), (true, false)] {
            assert!(matches!(
                admit(target, probes),
                Err(ArithmeticFailure::UnsupportedEnvironment)
            ));
        }
        assert!(admit(true, true).is_ok());
    }
}
