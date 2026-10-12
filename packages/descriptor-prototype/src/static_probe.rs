// SPDX-License-Identifier: AGPL-3.0-only
//! A narrow seven-const comparison; every physical operation still uses the CAS.

use std::marker::PhantomData;

use nrz_algebra::Rational;
use nrz_units::{BinaryOp, Descriptor, Dimension, ExponentPolicy, Registry};

use crate::Error;

/// Keep dimension encodings closed to the compiled const-generic experiment.
mod sealed {
    /// Implemented only by the seven-exponent marker below.
    pub trait Encoding {}
}

/// One integral dimension marker in the CAS basis order.
#[derive(Clone, Copy, Debug)]
pub struct Exponents<
    const L: i64,
    const M: i64,
    const T: i64,
    const I: i64,
    const K: i64,
    const N: i64,
    const J: i64,
>;

/// Convert only a dimension declaration, never an independent dimension operation.
pub trait Encoding: sealed::Encoding {
    /// Expected dimension expressed in the authoritative CAS representation.
    fn dimension() -> Dimension;
}

impl<
    const L: i64,
    const M: i64,
    const T: i64,
    const I: i64,
    const K: i64,
    const N: i64,
    const J: i64,
> sealed::Encoding for Exponents<L, M, T, I, K, N, J>
{
}

impl<
    const L: i64,
    const M: i64,
    const T: i64,
    const I: i64,
    const K: i64,
    const N: i64,
    const J: i64,
> Encoding for Exponents<L, M, T, I, K, N, J>
{
    fn dimension() -> Dimension {
        Dimension::integers([L, M, T, I, K, N, J])
    }
}

/// A dynamically checked descriptor carrying a compile-time integral dimension.
///
/// The static dimension rejects incompatible addition at the call boundary.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::static_probe::{Exponents, Typed};
/// use nrz_units::Registry;
/// type Voltage = Typed<Exponents<2, 1, -3, -1, 0, 0, 0>>;
/// type Current = Typed<Exponents<0, 0, 0, 1, 0, 0, 0>>;
/// fn misuse(v: &Voltage, i: &Current, registry: &Registry) {
///     let result = v.add(i, registry);
/// }
/// ```
///
/// ```
/// use nrz_descriptor_prototype::static_probe::{Exponents, Typed};
/// use nrz_units::Registry;
/// type Voltage = Typed<Exponents<2, 1, -3, -1, 0, 0, 0>>;
/// fn accepted(v: &Voltage, other: &Voltage, registry: &Registry) {
///     let result = v.add(other, registry);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Typed<D> {
    /// Kind, role, measure and count semantics remain runtime descriptor data.
    descriptor: Descriptor,
    /// Integral dimension established by a checked lift, not by a public field.
    dimension: PhantomData<D>,
}

/// An explicitly checked result annotation for operations without inferred exponent arithmetic.
#[derive(Clone, Debug)]
pub struct ResultWitness<D> {
    /// Both static dimension and dynamic meaning of the annotated result.
    expected: Typed<D>,
}

impl<D: Encoding> ResultWitness<D> {
    /// Check a result annotation against the registry and the compiled dimension.
    ///
    /// # Errors
    /// Refuses CAS admission, fractional dimensions and mismatched exponents.
    pub fn new(descriptor: Descriptor, registry: &Registry) -> Result<Self, Error> {
        Ok(Self {
            expected: Typed::lift(descriptor, registry)?,
        })
    }
}

impl<D: Encoding> Typed<D> {
    /// Lift descriptor data only after CAS admission and exact exponent comparison.
    ///
    /// # Errors
    /// Refuses invalid meanings, fractional exponents and a different static dimension.
    pub fn lift(descriptor: Descriptor, registry: &Registry) -> Result<Self, Error> {
        registry.clone().admit(&descriptor)?;
        descriptor.dimension().admit(ExponentPolicy::Integral)?;
        if descriptor.dimension() != D::dimension() {
            return Err(Error::Shape);
        }
        Ok(Self {
            descriptor,
            dimension: PhantomData,
        })
    }

    /// The dynamic semantic contract retained beside the static dimension.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    /// Same-exponent addition still checks kinds, roles and count closure at runtime.
    ///
    /// # Errors
    /// Preserves CAS semantic refusals even when both operands have the same Rust type.
    pub fn add(&self, other: &Self, registry: &Registry) -> Result<Self, Error> {
        Self::lift(
            registry.binary(BinaryOp::Add, &self.descriptor, &other.descriptor)?,
            registry,
        )
    }

    /// Multiply with an explicit checked output annotation instead of inferred arithmetic.
    ///
    /// # Errors
    /// Refuses CAS operations or an annotation with incompatible result meaning.
    pub fn product<R: Encoding, O: Encoding>(
        &self,
        other: &Typed<R>,
        output: &ResultWitness<O>,
        registry: &Registry,
    ) -> Result<Typed<O>, Error> {
        finish(
            registry.binary(BinaryOp::Multiply, &self.descriptor, &other.descriptor)?,
            output,
            registry,
        )
    }

    /// Divide with an explicit checked output annotation.
    ///
    /// # Errors
    /// Refuses CAS operations or an incompatible output annotation.
    pub fn quotient<R: Encoding, O: Encoding>(
        &self,
        other: &Typed<R>,
        output: &ResultWitness<O>,
        registry: &Registry,
    ) -> Result<Typed<O>, Error> {
        finish(
            registry.binary(BinaryOp::Divide, &self.descriptor, &other.descriptor)?,
            output,
            registry,
        )
    }

    /// Take a square root through the CAS and a checked integral result annotation.
    ///
    /// # Errors
    /// Refuses unsupported kind rules, fractional results or wrong result annotations.
    pub fn root<O: Encoding>(
        &self,
        output: &ResultWitness<O>,
        registry: &Registry,
    ) -> Result<Typed<O>, Error> {
        finish(
            registry.power(&self.descriptor, Rational::new(1, 2)?)?,
            output,
            registry,
        )
    }
}

/// Check runtime meaning and then lift the CAS result into the annotated dimension.
fn finish<D: Encoding>(
    descriptor: Descriptor,
    output: &ResultWitness<D>,
    registry: &Registry,
) -> Result<Typed<D>, Error> {
    descriptor.conversion_to(output.expected.descriptor())?;
    Typed::lift(descriptor, registry)
}
