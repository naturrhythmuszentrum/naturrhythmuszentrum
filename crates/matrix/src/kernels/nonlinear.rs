// SPDX-License-Identifier: AGPL-3.0-only
//! Native direct interval images discharge deterministic domains over the entire input box.
//!
//! # Whole-box nonlinear domains
//!
//! `OperationDraft::map_elementwise` admits `Elementwise::Square`, `Reciprocal` and `SquareRoot` with a separately checked output shape. Bindings derives the required scalar power from descriptor metadata, and the input and output retain one frozen declaration owner and identical extents. Each entry's complete deterministic box is formed outward from its represented center and total radius. Native scalar interval images enclose the entire box, including the quadratic term for square and zero-crossing square intervals. No sampled derivative or first-order approximation substitutes for that enclosure.
//!
//! The new evaluation region encloses the exact function at the represented center. The total encloses the full image around the candidate and includes that evaluation region once. `KernelError::Domain` identifies the function, logical entry and `DomainReason`: a reciprocal box including zero, a root box containing negative values, or an image that cannot be enclosed in the finite execution format. A late entry refusal discards the entire candidate and all companion reservations. Deterministic square root at zero is supported; a statistical operand at zero still refuses through the statistics boundary. Exact rational endpoint oracles and independent squared-endpoint comparisons test both binary formats, including irrational roots and physical squared-voltage descriptors.

use super::KernelError;
use super::arithmetic::{Entry, operand, singleton};

use crate::binding::buffer;
use crate::{NormalizedView, OperationError};
use nrz_accuracy::Nonnegative;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Closed native elementwise functions with independently checked deterministic domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Elementwise {
    /// Square on the real line; its finite enclosure must remain representable.
    Square,
    /// Reciprocal on an interval separated from zero.
    Reciprocal,
    /// Nonnegative square root, including a deterministic singleton zero.
    SquareRoot,
}

/// A numerical domain premise that was not established for the entire selected entry box.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainReason {
    /// The reciprocal input enclosure includes zero.
    ZeroInDivisor,
    /// The square-root input enclosure contains negative values.
    NegativeRoot,
    /// The finite-format enclosure overflows, even though the real function may exist.
    UnboundedImage,
}

/// Enclose every entry's full bounded family before issuing any candidate.
pub(crate) fn evaluate<P: Scalar + 'static>(
    input: &NormalizedView<'_, P>,
    function: Elementwise,
    context: &ArithmeticContext,
) -> Result<Vec<Entry<P>>, OperationError> {
    let mut result = buffer(input.extent().entries())?;
    for index in 0..input.extent().entries() {
        let (center, radius) = operand(input, index)?;
        result.push(
            evaluate_entry(center, radius, function, index, context).map_err(
                |cause| match cause {
                    KernelError::Arithmetic(nrz_scalar::ArithmeticFailure::Overflow) => {
                        KernelError::Domain {
                            function,
                            entry: index,
                            reason: DomainReason::UnboundedImage,
                        }
                    }
                    other => other,
                },
            )?,
        );
    }
    Ok(result)
}

/// Direct inclusion avoids a sampled derivative or a first-order-only nonlinear radius.
fn evaluate_entry<P: Scalar>(
    center: P,
    radius: Nonnegative<P>,
    function: Elementwise,
    index: usize,
    context: &ArithmeticContext,
) -> Result<Entry<P>, KernelError> {
    check_center_domain(center, radius.get(), function, index)?;
    let nominal = singleton(center)?;
    let bound = singleton(radius.get())?;
    let whole = Enclosure::new(
        nominal.sub(bound, context)?.lower().get(),
        nominal.add(bound, context)?.upper().get(),
    )?;
    let enclosed_image = image(whole, function, index, context)?;
    let exact_center = image_nominal(nominal, function, context)?;
    let candidate = Finite::new(match function {
        Elementwise::Square => center * center,
        Elementwise::Reciprocal => P::ONE / center,
        Elementwise::SquareRoot => center.sqrt(),
    })?;
    let error = exact_center.displacement_from(candidate, context)?.get();
    let radius = enclosed_image.displacement_from(candidate, context)?.get();
    Ok(Entry {
        center: candidate.get(),
        total: if radius < error { error } else { radius },
        error,
    })
}

/// Preserve specific full-box domain refusals instead of accepting a valid nominal center alone.
fn image<P: Scalar>(
    whole: Enclosure<P>,
    function: Elementwise,
    entry: usize,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, KernelError> {
    let reason = match function {
        Elementwise::Reciprocal
            if whole.lower().get() <= P::ZERO && whole.upper().get() >= P::ZERO =>
        {
            Some(DomainReason::ZeroInDivisor)
        }
        Elementwise::SquareRoot if whole.lower().get() < P::ZERO => {
            Some(DomainReason::NegativeRoot)
        }
        _ => None,
    };
    if let Some(reason) = reason {
        return Err(KernelError::Domain {
            function,
            entry,
            reason,
        });
    }
    image_nominal(whole, function, context).map_err(|cause| match cause {
        KernelError::Arithmetic(nrz_scalar::ArithmeticFailure::Overflow) => KernelError::Domain {
            function,
            entry,
            reason: DomainReason::UnboundedImage,
        },
        other => other,
    })
}

/// Monotone root/reciprocal and zero-aware square use the admitted scalar enclosure substrate.
fn image_nominal<P: Scalar>(
    input: Enclosure<P>,
    function: Elementwise,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, KernelError> {
    Ok(match function {
        Elementwise::Square => input.square(context)?,
        Elementwise::Reciprocal => singleton(P::ONE)?.div(input, context)?,
        Elementwise::SquareRoot => input.sqrt(context)?,
    })
}

/// Exact represented comparisons identify invalid real domains before endpoint overflow can hide them.
fn check_center_domain<P: Scalar>(
    center: P,
    radius: P,
    function: Elementwise,
    entry: usize,
) -> Result<(), KernelError> {
    let reason = match function {
        Elementwise::Reciprocal if center.abs() <= radius => Some(DomainReason::ZeroInDivisor),
        Elementwise::SquareRoot if center < radius => Some(DomainReason::NegativeRoot),
        _ => None,
    };
    if let Some(reason) = reason {
        return Err(KernelError::Domain {
            function,
            entry,
            reason,
        });
    }
    Ok(())
}
