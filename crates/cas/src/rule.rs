// SPDX-License-Identifier: AGPL-3.0-only
//! Closed version-one primitive registry, without user-defined trusted callbacks.

/// A compiled checker whose mathematical meaning is fixed by semantic version one.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Primitive {
    /// Equality of an expression with itself on its defined domain.
    Reflexivity,
    /// Reverse one equality premise.
    Symmetry,
    /// Compose two equality premises with an identical middle term.
    Transitivity,
    /// Replace a checked operand path using one equality premise.
    Congruence,
    /// Scalar additive commutativity.
    AddCommute,
    /// Scalar multiplicative commutativity.
    MulCommute,
    /// Reassociate an ordered scalar sum to the right.
    AddAssociate,
    /// Reassociate an ordered scalar product to the right.
    MulAssociate,
    /// Distribute a left scalar factor over a sum.
    Distribute,
    /// Remove a right additive zero.
    AddZero,
    /// Remove a right multiplicative one.
    MulOne,
    /// Add an expression to its negation.
    AddInverse,
    /// Multiply by a right zero while retaining the other operand's domain.
    MulZero,
    /// Cancel equal numerator and denominator under a nonzero guard.
    Cancel,
    /// Strict integer-power recurrence.
    Power,
    /// Recompute an operation on rational literal operands.
    Exact,
    /// Equality of finite polynomial coefficient jets.
    Jets,
    /// Clear denominators and compare finite jets with all original guards.
    RationalIdentity,
    /// Derivative of a rational or named constant.
    DifferentiateConstant,
    /// Derivative of a declared real scalar variable.
    DifferentiateVariable,
    /// Derivative of a negation from its child premise.
    DifferentiateNeg,
    /// Derivative of an ordered sum from both child premises.
    DifferentiateAdd,
    /// Derivative of an ordered difference from both child premises.
    DifferentiateSub,
    /// Product rule with ordered child derivative premises.
    DifferentiateProduct,
    /// Quotient rule with ordered child derivative premises.
    DifferentiateQuotient,
    /// Strict fixed-integer power rule with a child derivative premise.
    DifferentiatePower,
    /// Transport a derivative result along an equality premise.
    DerivativeTransport,
    /// Simultaneously substitute an equality and all its domain requirements.
    Substitute,
    /// Assemble an ordered matrix from a derivative premise for every cell.
    Jacobian,
}

/// Immutable public metadata for one compiled schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleSpec {
    /// Closed checker identity.
    pub primitive: Primitive,
    /// Stable ASCII rule key.
    pub key: &'static str,
    /// Exact semantic version; no implicit upgrades occur during replay.
    pub version: u32,
    /// Positive transition cost in the logical-v1 policy.
    pub weight: u64,
}

impl Primitive {
    /// Read the pinned version-one schema metadata.
    #[must_use]
    pub const fn specification(self) -> RuleSpec {
        let (key, weight) = match self {
            Self::Reflexivity => ("eq.refl", 1),
            Self::Symmetry => ("eq.sym", 1),
            Self::Transitivity => ("eq.trans", 1),
            Self::Congruence => ("eq.congruence", 1),
            Self::AddCommute => ("eq.add-commute", 2),
            Self::MulCommute => ("eq.mul-commute", 2),
            Self::AddAssociate => ("eq.add-associate", 2),
            Self::MulAssociate => ("eq.mul-associate", 2),
            Self::Distribute => ("eq.distribute", 2),
            Self::AddZero => ("eq.add-zero", 2),
            Self::MulOne => ("eq.mul-one", 2),
            Self::AddInverse => ("eq.add-inverse", 2),
            Self::MulZero => ("eq.mul-zero", 2),
            Self::Cancel => ("eq.cancel", 2),
            Self::Power => ("eq.integer-power", 2),
            Self::Exact => ("exact.rational", 1),
            Self::Jets => ("poly.jets-equal", 4),
            Self::RationalIdentity => ("poly.rational-equal", 4),
            Self::DifferentiateConstant => ("diff.constant", 2),
            Self::DifferentiateVariable => ("diff.variable", 2),
            Self::DifferentiateNeg => ("diff.neg", 2),
            Self::DifferentiateAdd => ("diff.add", 2),
            Self::DifferentiateSub => ("diff.sub", 2),
            Self::DifferentiateProduct => ("diff.product", 2),
            Self::DifferentiateQuotient => ("diff.quotient", 2),
            Self::DifferentiatePower => ("diff.integer-power", 2),
            Self::DerivativeTransport => ("diff.transport", 1),
            Self::Substitute => ("eq.substitute", 1),
            Self::Jacobian => ("diff.jacobian", 2),
        };
        RuleSpec {
            primitive: self,
            key,
            version: 1,
            weight,
        }
    }
}

/// Every admitted primitive, in lexical key order; no square-free checker is installed yet.
pub const PRIMITIVES: [Primitive; 29] = [
    Primitive::DifferentiateAdd,
    Primitive::DifferentiateConstant,
    Primitive::DifferentiatePower,
    Primitive::Jacobian,
    Primitive::DifferentiateNeg,
    Primitive::DifferentiateProduct,
    Primitive::DifferentiateQuotient,
    Primitive::DifferentiateSub,
    Primitive::DerivativeTransport,
    Primitive::DifferentiateVariable,
    Primitive::AddAssociate,
    Primitive::AddCommute,
    Primitive::AddInverse,
    Primitive::AddZero,
    Primitive::Cancel,
    Primitive::Congruence,
    Primitive::Distribute,
    Primitive::Power,
    Primitive::MulAssociate,
    Primitive::MulCommute,
    Primitive::MulOne,
    Primitive::MulZero,
    Primitive::Reflexivity,
    Primitive::Substitute,
    Primitive::Symmetry,
    Primitive::Transitivity,
    Primitive::Exact,
    Primitive::Jets,
    Primitive::RationalIdentity,
];
