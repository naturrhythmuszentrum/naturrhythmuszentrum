// SPDX-License-Identifier: AGPL-3.0-only
//! Flat mathematical vocabulary without physical descriptors or evaluation.

use crate::number::Rational;

/// A coefficient or ambient scalar field.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Field {
    /// Exact rational coefficients.
    Rational,
    /// Real scalars.
    Real,
    /// Complex scalars.
    Complex,
    /// A declared positive-characteristic field, not an executed coefficient domain.
    PositiveCharacteristic(u32),
    /// A caller-declared field whose algorithms are unresolved.
    Named(String),
}

/// Explicit branch and zero-power convention.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Branch {
    /// Real operations, with zero to the zeroth power undefined.
    StrictReal,
    /// Principal complex operations with their retained branch conditions.
    PrincipalComplex,
    /// An explicit caller convention requiring a matching future theorem.
    Declared(String),
}

/// A mathematical extent, independent of physical axes or units.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Extent {
    /// A finite literal extent, including zero.
    Literal(u32),
    /// A declared integer parameter.
    Parameter(String),
}

/// A nonrecursive value sort, usable in function signatures and binder frames.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AtomicSort {
    /// A mathematical integer.
    Integer,
    /// A scalar in the specified field.
    Scalar(Field),
    /// A predicate truth value.
    Boolean,
    /// A rectangular mathematical matrix.
    Matrix {
        /// Entry field.
        field: Field,
        /// Row extent.
        rows: Extent,
        /// Column extent.
        columns: Extent,
    },
}

/// A flat mathematical sort; recursively nested type syntax is not accepted.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Sort {
    /// An ordinary scalar, integer, Boolean or matrix value.
    Value(AtomicSort),
    /// A function with an ordered, finite signature.
    Function {
        /// Ordered argument sorts.
        inputs: Vec<AtomicSort>,
        /// Result sort.
        output: AtomicSort,
    },
    /// A set of values, such as a solution set.
    Set(AtomicSort),
    /// Smooth compactly supported test value in an explicitly linked space.
    Test,
    /// Distribution value in an explicitly linked space.
    Distribution,
}

/// One declared identity; its display spelling does not determine identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Declaration {
    /// Stable, case-sensitive caller key.
    pub key: String,
    /// Optional human spelling; equal spellings may name different keys.
    pub display_name: String,
    /// Mathematical signature.
    pub sort: Sort,
}

/// The semantic header computed before context assumptions.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Environment {
    /// Ambient field.
    pub field: Field,
    /// Branch convention.
    pub branch: Branch,
    /// Declarations, canonicalized by key during admission.
    pub declarations: Vec<Declaration>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            field: Field::Real,
            branch: Branch::StrictReal,
            declarations: Vec::new(),
        }
    }
}

/// Ordered outer-to-inner frames of mathematical bound-variable sorts.
pub type Scope = Vec<Vec<AtomicSort>>;

/// A single added binder frame and the operand positions whose scopes enter it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Binding {
    /// Sorts in slot order; display names are occurrence metadata.
    pub sorts: Vec<AtomicSort>,
    /// Ordered body positions; all other operands see only the outer frames.
    pub body_operands: Vec<u32>,
}

/// Exact named constants with no floating approximation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Constant {
    /// The circle constant.
    Pi,
    /// The real exponential base.
    E,
}

/// Mathematical condition constructors; none implies physical validity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PredicateKind {
    /// All operands are defined on the specified domain.
    Defined,
    /// A scalar is nonzero.
    NonZero,
    /// A real scalar is positive.
    Positive,
    /// A real scalar is nonnegative.
    NonNegative,
    /// An integer belongs to a half-open interval.
    IntegerRange,
    /// A function has the requested differentiability.
    Differentiable,
    /// Ordered operands are mathematically equal.
    Equal,
    /// A matrix is symmetric.
    Symmetric,
    /// Logical conjunction.
    And,
    /// Logical disjunction.
    Or,
    /// Explicit logical negation.
    Not,
    /// Universally quantified body with explicit binder sorts.
    ForAll,
    /// A typed declaration whose truth remains a hypothesis.
    Named(String),
}

/// A retained limit approach, never an inferred path-independence statement.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Approach {
    /// Real two-sided approach.
    Both,
    /// Real approach from below.
    Below,
    /// Real approach from above.
    Above,
    /// Positive infinity.
    PositiveInfinity,
    /// Negative infinity.
    NegativeInfinity,
    /// An explicitly supplied path operand.
    Path,
}

/// The requested spectral information, without numerical evaluation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SpectralKind {
    /// Least real eigenvalue under the declared symmetry hypotheses.
    Least,
    /// Greatest real eigenvalue under the declared symmetry hypotheses.
    Greatest,
    /// Algebraic multiplicity at a specified eigenvalue.
    AlgebraicMultiplicity,
    /// Dimension of an eigenspace at a specified eigenvalue.
    GeometricMultiplicity,
    /// An eigenvector solution set.
    Eigenvectors,
}

/// A symbolic matrix map with its own shape/domain conditions.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MatrixOperation {
    /// Determinant.
    Determinant,
    /// Trace.
    Trace,
    /// Classical adjugate.
    Adjugate,
    /// Inverse, retaining invertibility.
    Inverse,
}

/// A scalar operation lifted entrywise without broadcasting.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScalarOperation {
    /// Entrywise addition.
    Add,
    /// Entrywise multiplication.
    Multiply,
    /// Entrywise division with every denominator retained.
    Divide,
}

/// Algebraic-root selection as a request, not an existence certificate.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RootSelection {
    /// All roots in the declared field.
    All,
    /// A zero-based requested order among real roots.
    Ordered(u32),
    /// A supplied selection-predicate key.
    Predicate(String),
}

/// Closed associations between independently declared symbols; never equations or proofs.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SymbolRelation {
    /// A selected homogeneous copy result and its exact source symbol.
    Copy,
    /// A deterministic difference-bound symbol and its exact value target.
    DeterministicBound,
    /// A numerical-error symbol and its exact value target.
    NumericalError,
}

/// Mathematical syntax tags; all operands remain explicitly ordered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operator {
    /// Checked finite entry projection retaining its exact declared source.
    Select(Box<crate::readings::EntrySelection>),
    /// Transparent definition with ordered operands: declared symbol, exact source.
    AffineReading(Box<crate::readings::AffineMap>),
    /// An association between two closed declared symbols, without truth or rewrite authority.
    SymbolRelation(SymbolRelation),
    /// Complex conjugation, preserving the complex scalar field.
    Conjugate,
    /// The real component of a complex scalar.
    RealPart,
    /// The imaginary component of a complex scalar.
    ImagPart,
    /// One explicitly linked root selection slot.
    RootValue {
        /// Ordered joint selection position.
        slot: u32,
    },
    /// An explicitly mapped exact coefficient literal.
    ExactCoefficient,
    /// A regular distribution; local integrability still requires checked evidence.
    RegularEmbed,
    /// Zero in the explicitly linked distribution space.
    DistributionZero,
    /// Delta at the sole scalar location operand.
    Delta {
        /// Nonnegative derivative order.
        order: u32,
    },
    /// Addition of two distributions in exactly the same space.
    DistributionAdd,
    /// A rational scalar operand followed by a distribution.
    DistributionScale,
    /// Formal iterated distribution derivative.
    DistributionDerivative {
        /// Positive derivative order.
        order: u32,
    },
    /// Smooth scalar operand followed by a distribution.
    SmoothMultiply,
    /// Action on an ordinary declared compact smooth test.
    Pair,
    /// Cutoff-independent action on a separately certified supported germ.
    PairNearSupport,
    /// Test derivative, which preserves smoothness and compact support.
    TestDerivative {
        /// Positive derivative order.
        order: u32,
    },
    /// Smooth scalar operand followed by a test.
    TestMultiply,
    /// Test operand followed by an interior scalar location.
    TestApply,
    /// Defining integral action of a regular representative on a test.
    RegularAction,
    /// Transparent canonical case definition retained as one operand.
    NamedCase(Box<crate::boundary_model::NamedCase>),
    /// Native cases use typed domain and arm links, not scalar operands.
    Cases,
    /// An exact reduced rational.
    Rational(Rational),
    /// A free declaration key.
    Free(String),
    /// A reference counted from the innermost binder frame.
    Bound {
        /// Zero denotes the innermost frame.
        depth: u32,
        /// Position within the frame.
        slot: u32,
    },
    /// An exact named constant.
    Constant(Constant),
    /// Scalar or integer negation.
    Neg,
    /// Ordered scalar or integer addition.
    Add,
    /// Ordered scalar or integer subtraction.
    Sub,
    /// Ordered scalar or integer multiplication.
    Mul,
    /// Scalar division with explicit definedness obligations.
    Div,
    /// A fixed signed integer power under the context convention.
    IntegerPower(i64),
    /// A general power with an expression-valued exponent.
    GeneralPower(Branch),
    /// A call to a declaration with a checked function signature.
    Call(String),
    /// An unevaluated derivative with respect to a free declaration.
    Derivative(String),
    /// An integral; the body enters one scalar binder frame.
    Integral {
        /// Whether the two outer-scope endpoints are supplied.
        definite: bool,
    },
    /// A finite half-open sum with one integer binder frame.
    Sum,
    /// A formal congruence or analytic series with an explicit center operand.
    Series {
        /// Requested order.
        order: u32,
        /// Whether regularity and a remainder are required.
        analytic: bool,
    },
    /// A limit with a body, approach point and optional path operand.
    Limit(Approach),
    /// A mathematical predicate as a Boolean expression.
    Predicate(PredicateKind),
    /// A represented solution set with its completeness request.
    Set {
        /// True requests complete coverage, rather than candidate checking alone.
        complete: bool,
    },
    /// A row-major explicit matrix.
    Matrix {
        /// Row count.
        rows: u32,
        /// Column count.
        columns: u32,
    },
    /// A matrix body under ordered row/column integer binder slots.
    Comprehension {
        /// Row extent.
        rows: Extent,
        /// Column extent.
        columns: Extent,
    },
    /// Matrix transpose.
    Transpose,
    /// Ordered matrix multiplication.
    MatrixMultiply,
    /// A spectral request on a checked matrix shape.
    Spectral(SpectralKind),
    /// An exact root request with a polynomial and selection convention.
    AlgebraicRoot {
        /// Declared polynomial variable.
        variable: String,
        /// Coefficient/selection field.
        field: Field,
        /// Requested multiplicity, absent when it is to be determined.
        multiplicity: Option<u32>,
        /// Selection predicate or requested order.
        selection: RootSelection,
    },
    /// A shape-checked symbolic matrix map.
    MatrixMap(MatrixOperation),
    /// Explicit integer-to-scalar embedding.
    IntegerEmbed,
    /// A scalar operation applied entrywise to equal shapes.
    Entrywise(ScalarOperation),
}

/// A half-open source-byte span whose UTF-8 boundaries are checked on admission.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Span {
    /// Inclusive byte offset.
    pub start: u64,
    /// Exclusive byte offset.
    pub end: u64,
}

/// A first-class task whose execution may remain unresolved.
#[derive(Clone, Debug, Eq, PartialEq)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "scalar producer intent remains private behind the typed task facade"
)]
pub(crate) enum Operation {
    /// Search for a simpler expression.
    Simplify,
    /// Prove equality to the supplied target root.
    Equal,
    /// Construct a finite coefficient representation.
    Expand,
    /// Differentiate with respect to a declared variable.
    Differentiate(String),
    /// Assemble partials in the supplied distinct variable order.
    Jacobian(Vec<String>),
    /// Decompose univariate rational polynomial multiplicity layers.
    SquareFree(String),
    /// Prove the requested spectral statement.
    Spectral,
}
