// SPDX-License-Identifier: AGPL-3.0-only
//! Pure exact scalar interval laws and retained physical source definitions.

use crate::pi_bounds::PiBoundCertificate;
use crate::{Handle, Rational};

/// One exact scalar law; indices refer only to earlier complete records.
///
/// Each referenced record supplies its entire real interval, so all arithmetic
/// relations cover the full Cartesian input box. No floating value is accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarKernelLaw {
    /// The independently checked exact rational constant, before numerical import.
    Rational(Rational),
    /// Version-two closed input, including its entire center-plus-total-radius set.
    Input(Box<ScalarKernelInput>),
    /// Version-three physical family, conditional on two retained endpoint assumptions.
    Family(Box<ScalarKernelFamilyInput>),
    /// Version-four exact affine definition applied to an earlier physical family.
    Reading(Box<ScalarKernelReading>),
    /// Exact real addition of the ordered earlier records.
    Sum {
        /// Earlier left operand.
        left: u32,
        /// Earlier right operand.
        right: u32,
    },
    /// Exact real product of the ordered earlier records.
    Product {
        /// Earlier left operand.
        left: u32,
        /// Earlier right operand.
        right: u32,
    },
    /// Exact reciprocal of an earlier interval proved away from zero.
    Reciprocal {
        /// Earlier divisor record.
        input: u32,
    },
    /// Nonnegative square root of an earlier interval proved strictly positive.
    SquareRoot {
        /// Earlier positive radicand record.
        input: u32,
    },
}

/// An untrusted exact endpoint claim for one operation in a finite scalar trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKernelRecord {
    /// The mathematical operation and its complete operand references.
    pub law: ScalarKernelLaw,
    /// Included rational lower endpoint, exported from a scalar dyadic.
    pub lower: Rational,
    /// Included rational upper endpoint, exported from a scalar dyadic.
    pub upper: Rational,
}

/// Pure scalar trace under the exact original context and admitted input family.
///
/// Version one has rational leaves and closed sums, products, reciprocals and
/// positive square roots. Version two additionally checks complete native input
/// sets with closed rational or rational-times-pi sources. Version three additionally
/// retains explicit physical source-box assumptions. Version four adds exact scalar
/// affine reading definitions. The last record is the original
/// target. A quotient uses reciprocal then product; every record is checked before
/// any result. Closed leaves add no assumptions; family leaves retain both qualified
/// endpoint premises through mathematical selection and independent wire replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKernelCertificate {
    /// Independently versioned closed law and endpoint-reduction grammar.
    pub version: u32,
    /// Exact original expression, including every operand before normalization.
    pub expression: Handle,
    /// Original context; a nominal substitute cannot stand for another scope.
    pub context: Handle,
    /// The exact admitted region of that original context.
    pub input_family: Handle,
    /// Nonempty child-before-parent records; the final record is the result.
    pub records: Vec<ScalarKernelRecord>,
}

/// A closed nominal input and its entire native declared-bound set.
///
/// These untrusted numbers prove interval arithmetic only. The matrix owner must
/// bind them to the actual original source and complete live input versions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKernelInput {
    /// Exact binary center converted without floating arithmetic.
    pub center: Rational,
    /// Nonnegative total radius, already including numerical error exactly once.
    pub radius: Rational,
    /// Independently checked closed nominal source in coherent coordinates.
    pub value: ScalarKernelInputValue,
}

/// The closed source subset accepted by version-two native input records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarKernelInputValue {
    /// Exact coherent rational source.
    Rational(Rational),
    /// Exact rational multiple of pi, with an independently checked finite Machin argument.
    PiMultiple {
        /// Signed exact coherent rational coefficient.
        factor: Rational,
        /// Untrusted bounds; reuse requires the complete store-local checked certificate key.
        certificate: Box<PiBoundCertificate>,
    },
}

/// An exact declared source box whose qualified endpoint assumptions remain conditional.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKernelFamily {
    /// Original Real source expression, never inferred from a center or display name.
    pub source: Handle,
    /// Exact included lower endpoint of the declared physical family.
    pub lower: Rational,
    /// Exact included upper endpoint of the declared physical family.
    pub upper: Rational,
    /// Qualified assumptions for source-minus-lower and upper-minus-source being nonnegative.
    pub premises: [crate::domain::HypothesisRef; 2],
}

/// A complete native input set covering an explicitly assumed physical family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKernelFamilyInput {
    /// Exact binary center of the executed input sum.
    pub center: Rational,
    /// Nonnegative total radius, including numerical error exactly once.
    pub radius: Rational,
    /// Original source identity, declared box and retained endpoint assumptions.
    pub family: ScalarKernelFamily,
}

/// One original scalar affine definition, independently matched to its exact coefficients.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKernelReading {
    /// Actual transparent CAS reading definition, never its bare result declaration.
    pub definition: Handle,
    /// Earlier complete physical-family input record.
    pub input: u32,
    /// Exact rational scale declared by the reading.
    pub scale: Rational,
    /// Exact rational offset declared by the reading.
    pub offset: Rational,
}
