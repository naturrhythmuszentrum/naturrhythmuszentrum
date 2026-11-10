// SPDX-License-Identifier: AGPL-3.0-only
//! Finite object computations retain their original operands and explicit precision.

use crate::graph::Handle;
use crate::number::Rational;
use crate::rule::Primitive;

/// Finite object computations and registered correspondence with an exact original source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SeriesStatement {
    /// Same-order finite inverse in the identical admitted rational frame.
    Reciprocal {
        /// Original admitted object.
        input: Handle,
        /// Proposed inverse at exactly the input order.
        output: Handle,
    },
    /// Polynomial interpretation of the complete original syntax at one checked finite subject.
    EvaluatedResidual {
        /// Exact original checked source or composition.
        subject: ResidualSubject,
        /// Original differential polynomial, including dead syntax.
        expression: Handle,
        /// Distinct declared Real formal dependent placeholder.
        dependent: Handle,
        /// Complete computed residual object, which may be nonzero.
        output: Handle,
        /// Exact ordered coefficient equations in the original coefficient region.
        problem: Handle,
        /// Maximum derivative-chain length in the original syntax.
        derivative_order: u32,
        /// Positive retained coefficient count.
        order: u32,
    },
    /// Exact original source correspondence, separate from object arithmetic.
    Source(super::source::SourceStatement),
    /// Formal object derivative with exactly one fewer known coefficient.
    Derive {
        /// Original object of order at least two.
        input: Handle,
        /// Proposed object of order one less than the input.
        output: Handle,
    },
    /// Formal object antiderivative with an independently specified rational constant.
    Integrate {
        /// Original admitted object.
        input: Handle,
        /// Coordinate-independent rational with ordinary definedness evidence in slot zero.
        constant: Handle,
        /// Proposed object of order one greater than the input.
        output: Handle,
    },
    /// Restrict a complete object to the output's positive coefficient count.
    Truncate {
        /// Original object, including all coefficients beyond the requested order.
        input: Handle,
        /// Proposed restricted object.
        output: Handle,
    },
    /// Add two original objects at an explicitly recorded common order.
    Add {
        /// Original first object.
        left: Handle,
        /// Original second object.
        right: Handle,
        /// Proposed common-order sum.
        output: Handle,
        /// Positive coefficient count shared by the restricted inputs and output.
        order: u32,
        /// Require the recorded order to be the minimum available input order.
        default_order: bool,
    },
    /// Multiply every coefficient by an exact rational scalar.
    Scale {
        /// Original object.
        input: Handle,
        /// Exact scalar whose ordinary definedness proof occupies premise slot zero.
        scalar: Handle,
        /// Proposed object at the unchanged order.
        output: Handle,
    },
    /// Convolve two original objects at an explicitly recorded common order.
    Mul {
        /// Original first object.
        left: Handle,
        /// Original second object.
        right: Handle,
        /// Proposed common-order product.
        output: Handle,
        /// Positive coefficient count shared by the restricted inputs and output.
        order: u32,
        /// Require the recorded order to be the minimum available input order.
        default_order: bool,
    },
    /// Compose two original objects at an explicitly recorded common order.
    Compose {
        /// Original first object.
        left: Handle,
        /// Original second object.
        right: Handle,
        /// Proposed common-order composition; left is outer and right is inner.
        output: Handle,
        /// Positive coefficient count shared by the restricted inputs and output.
        order: u32,
        /// Require the recorded order to be the minimum available input order.
        default_order: bool,
    },
    /// Extract a coefficient without asserting a source value or derivative.
    Coefficient {
        /// Original object with its complete available order.
        input: Handle,
        /// Zero-based coefficient index, strictly below the object's order.
        index: u32,
        /// Proposed exact rational value in the object's interpretation.
        value: Handle,
    },
}

/// One independently checked coefficientwise binary arithmetic row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesBinaryRow {
    /// Output coefficient index.
    pub index: u32,
    /// Exact first operand, checked against the original object or scaling scalar.
    pub left: Rational,
    /// Exact second operand, checked against the original coefficient.
    pub right: Rational,
    /// Proposed exact sum or product.
    pub result: Rational,
}

/// One ordered convolution pair and the exact partial sum after that pair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesProductRow {
    /// Output degree, reconstructed independently by the checker.
    pub degree: u32,
    /// First input index.
    pub left_index: u32,
    /// Second input index; the two indices must sum to the degree.
    pub right_index: u32,
    /// Exact first input coefficient.
    pub left: Rational,
    /// Exact second input coefficient.
    pub right: Rational,
    /// Proposed exact pair product.
    pub product: Rational,
    /// Proposed partial sum, starting from zero for each output degree.
    pub sum: Rational,
}

/// One complete power-convolution row, retaining its outer exponent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesPowerRow {
    /// Positive exponent of the power being reconstructed.
    pub exponent: u32,
    /// Exact pair and partial sum in this power.
    pub row: SeriesProductRow,
}

/// One derivative coefficient and its independently reconstructed positive integer multiplier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesDerivativeRow {
    /// Ascending output index.
    pub index: u32,
    /// Original input coefficient at index plus one.
    pub input: Rational,
    /// Proposed exact embedding of index plus one in Q.
    pub multiplier: Rational,
    /// Proposed product.
    pub result: Rational,
}

/// One integral coefficient with finite checked integer-unit evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesIntegralRow {
    /// Ascending input index.
    pub index: u32,
    /// Proposed exact embedding of index plus one in Q.
    pub integer: Rational,
    /// Proposed inverse, checked by multiplication to exactly one.
    pub inverse: Rational,
    /// Original coefficient, including zero-valued entries.
    pub input: Rational,
    /// Proposed coefficient at output index plus one.
    pub result: Rational,
}

/// Untrusted finite evidence; only a registered checker can establish a judgment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SeriesCertificate {
    /// Mode seven: checked rational unit and complete identity convolution.
    Reciprocal {
        /// Only object form zero executes.
        form: u8,
        /// Exact original constant.
        constant: Rational,
        /// Proposed multiplicative inverse of the constant.
        inverse: Rational,
        /// Every product, including zeros, in degree and first-index order.
        rows: Vec<SeriesProductRow>,
    },
    /// Mode eleven, evaluated stage zero with a complete postorder occurrence trace.
    EvaluatedResidual {
        /// Only object form zero is supported.
        form: u8,
        /// Only evaluated stage zero is supported.
        stage: u8,
        /// Every occurrence and repeated-power round, in exact schedule order.
        rows: Vec<ResidualRow>,
    },
    /// Closed source theorem, polynomial seed or finite source inference.
    Source(super::source::SourceCertificate),
    /// Reserved mode eight, with no ordinary-proof slots.
    Derive {
        /// Only object form zero is executable.
        form: u8,
        /// Complete ascending derivative rows.
        rows: Vec<SeriesDerivativeRow>,
    },
    /// Reserved mode nine, with one ordinary constant-definedness proof.
    Integrate {
        /// Only object form zero is executable.
        form: u8,
        /// Complete ascending input rows and their checked integer units.
        rows: Vec<SeriesIntegralRow>,
    },
    /// Mode zero, comparing every retained coefficient.
    Truncate {
        /// Only object form zero is executable.
        form: u8,
        /// Ordered proposed coefficients of the restricted object.
        coefficients: Vec<Rational>,
    },
    /// Mode one, with one exact sum row per output coefficient.
    Add {
        /// Only object form zero is executable.
        form: u8,
        /// Recorded common coefficient count.
        order: u32,
        /// Whether the common order is the minimum input order.
        default_order: bool,
        /// Complete ascending coefficient rows.
        rows: Vec<SeriesBinaryRow>,
    },
    /// Mode two; premise zero is the ordinary checked scalar-definedness judgment.
    Scale {
        /// Only object form zero is executable.
        form: u8,
        /// Complete ascending scalar-product rows.
        rows: Vec<SeriesBinaryRow>,
    },
    /// Mode three, with every pair in increasing degree and first-index order.
    Mul {
        /// Only object form zero is executable.
        form: u8,
        /// Recorded common coefficient count.
        order: u32,
        /// Whether the common order is the minimum input order.
        default_order: bool,
        /// Complete finite convolution rows, including zero-valued products.
        rows: Vec<SeriesProductRow>,
    },
    /// Mode five, complete powers followed by coefficient accumulations.
    Compose {
        /// Only object form zero is executable.
        form: u8,
        /// Recorded common coefficient count.
        order: u32,
        /// Whether the common order is the minimum input order.
        default_order: bool,
        /// Exact inner coefficient at index zero, checked against zero in Q.
        zero: Rational,
        /// Complete length-order zeroth power.
        seed: Vec<Rational>,
        /// Rows in exponent, degree and first-index order.
        powers: Vec<SeriesPowerRow>,
        /// Rows in output-degree and outer-exponent order.
        sums: Vec<SeriesProductRow>,
    },
    /// Mode four, selecting exactly one available coefficient.
    Coefficient {
        /// Only object form zero is executable.
        form: u8,
        /// Original zero-based index.
        index: u32,
        /// Proposed exact coefficient value.
        value: Rational,
    },
}

impl SeriesStatement {
    /// The sole schema capable of checking this finite operation.
    #[must_use]
    pub const fn primitive(&self) -> Primitive {
        match self {
            Self::EvaluatedResidual { .. } => Primitive::SeriesResidual,
            Self::Source(value) => value.clause.primitive(),
            Self::Reciprocal { .. } => Primitive::SeriesReciprocal,
            Self::Derive { .. } => Primitive::SeriesDerive,
            Self::Integrate { .. } => Primitive::SeriesIntegrate,
            Self::Truncate { .. } => Primitive::SeriesTruncate,
            Self::Add { .. } => Primitive::SeriesAdd,
            Self::Scale { .. } => Primitive::SeriesScale,
            Self::Mul { .. } => Primitive::SeriesMul,
            Self::Compose { .. } => Primitive::SeriesCompose,
            Self::Coefficient { .. } => Primitive::SeriesCoefficient,
        }
    }

    /// Every original expression, domain, object and result, in semantic role order.
    pub(crate) fn objects(&self) -> Vec<&Handle> {
        match self {
            Self::EvaluatedResidual {
                subject,
                expression,
                dependent,
                output,
                problem,
                ..
            } => {
                let mut objects = subject.objects();
                objects.extend([expression, dependent, output, problem]);
                objects
            }
            Self::Source(value) => vec![&value.source, &value.domain, &value.output],
            Self::Reciprocal { input, output }
            | Self::Truncate { input, output }
            | Self::Derive { input, output } => {
                vec![input, output]
            }
            Self::Integrate {
                input,
                constant,
                output,
            } => vec![input, constant, output],
            Self::Add {
                left,
                right,
                output,
                ..
            }
            | Self::Compose {
                left,
                right,
                output,
                ..
            }
            | Self::Mul {
                left,
                right,
                output,
                ..
            } => vec![left, right, output],
            Self::Scale {
                input,
                scalar,
                output,
            } => vec![input, scalar, output],
            Self::Coefficient { input, value, .. } => vec![input, value],
        }
    }

    /// The first original operand; source clauses obtain their interpretation from the output.
    pub(crate) const fn input(&self) -> &Handle {
        match self {
            Self::EvaluatedResidual { subject, .. } => subject.output(),
            Self::Source(value) => &value.source,
            Self::Reciprocal { input, .. }
            | Self::Truncate { input, .. }
            | Self::Derive { input, .. }
            | Self::Integrate { input, .. }
            | Self::Scale { input, .. }
            | Self::Coefficient { input, .. } => input,
            Self::Add { left, .. } | Self::Compose { left, .. } | Self::Mul { left, .. } => left,
        }
    }
}

impl SeriesCertificate {
    /// Closed finite certificate discriminator, separate from graph and record kinds.
    #[must_use]
    pub const fn mode(&self) -> u8 {
        match self {
            Self::EvaluatedResidual { .. } => 11,
            Self::Source(value) => value.clause.mode(),
            Self::Reciprocal { .. } => 7,
            Self::Derive { .. } => 8,
            Self::Integrate { .. } => 9,
            Self::Truncate { .. } => 0,
            Self::Add { .. } => 1,
            Self::Scale { .. } => 2,
            Self::Mul { .. } => 3,
            Self::Compose { .. } => 5,
            Self::Coefficient { .. } => 4,
        }
    }

    /// Only form zero denotes executable object arithmetic.
    #[must_use]
    pub const fn form(&self) -> u8 {
        match self {
            Self::Source(value) => value.clause.form(),
            Self::EvaluatedResidual { form, .. }
            | Self::Reciprocal { form, .. }
            | Self::Truncate { form, .. }
            | Self::Derive { form, .. }
            | Self::Integrate { form, .. }
            | Self::Add { form, .. }
            | Self::Scale { form, .. }
            | Self::Mul { form, .. }
            | Self::Compose { form, .. }
            | Self::Coefficient { form, .. } => *form,
        }
    }
}

/// Closed original provenance; residual computations cannot be nested as subjects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResidualSubject {
    /// Exact source correspondence with its original clause and domain.
    Source(super::SourceStatement),
    /// Exact object composition, retaining both original inputs and precision choice.
    Compose {
        /// Original outer object.
        left: Handle,
        /// Original inner object.
        right: Handle,
        /// Checked candidate object.
        output: Handle,
        /// Original common coefficient count.
        order: u32,
        /// Original minimum-order choice.
        default_order: bool,
    },
}

impl ResidualSubject {
    /// Checked candidate object without any provenance erasure.
    pub(crate) const fn output(&self) -> &Handle {
        match self {
            Self::Source(value) => &value.output,
            Self::Compose { output, .. } => output,
        }
    }

    /// Retain every original subject reference in wire role order.
    pub(crate) fn objects(&self) -> Vec<&Handle> {
        match self {
            Self::Source(value) => vec![&value.source, &value.domain, &value.output],
            Self::Compose {
                left,
                right,
                output,
                ..
            } => vec![left, right, output],
        }
    }

    /// The unique ordinary statement required in premise slot zero.
    pub(crate) fn statement(&self) -> SeriesStatement {
        match self {
            Self::Source(value) => SeriesStatement::Source(value.clone()),
            Self::Compose {
                left,
                right,
                output,
                order,
                default_order,
            } => SeriesStatement::Compose {
                left: left.clone(),
                right: right.clone(),
                output: output.clone(),
                order: *order,
                default_order: *default_order,
            },
        }
    }
}

/// One dense vector in the deterministic postorder occurrence schedule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidualRow {
    /// Zero-based exact schedule position.
    pub ordinal: u32,
    /// Complete ascending vector at the uniform retained order.
    pub coefficients: Vec<Rational>,
}
