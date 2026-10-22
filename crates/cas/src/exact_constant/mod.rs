// SPDX-License-Identifier: AGPL-3.0-only
//! Owned finite constant expressions with checked sign evidence and exact CAS normalization.

mod arithmetic;
mod graph;
mod sign;

use std::rc::Rc;

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::{Constant, Operator};
use crate::number::Rational;

/// Exact source-free scalar storage; rational literals need no separate graph allocation.
#[derive(Clone, Debug)]
enum Value {
    /// An already checked CAS rational literal.
    Rational(Rational),
    /// An immutable CAS graph and its reconstructed finite/sign evidence.
    Expression(Rc<graph::CheckedConstant>),
}

/// A finite real constant expression, never a floating approximation or a free parameter.
///
/// Admission checks every original operand before normalization. The closed grammar
/// contains rational literals, `Pi`, `E`, negation, arithmetic and integer powers.
/// Divisors and nonpositive powers require an independently established nonzero sign.
/// Equality compares normalized CAS structure; it is not a general identity decision.
#[derive(Clone, Debug)]
pub struct ExactConstant(Value);

impl PartialEq for ExactConstant {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Value::Rational(left), Value::Rational(right)) => left == right,
            (Value::Expression(left), Value::Expression(right)) => left.key == right.key,
            _ => false,
        }
    }
}

impl Eq for ExactConstant {}

impl PartialEq<Rational> for ExactConstant {
    fn eq(&self, other: &Rational) -> bool {
        self.as_rational() == Some(other)
    }
}

impl From<Rational> for ExactConstant {
    fn from(value: Rational) -> Self {
        Self(Value::Rational(value))
    }
}

impl ExactConstant {
    /// Literal zero, without an allocation or a numerical evaluation.
    #[must_use]
    pub fn zero() -> Self {
        Rational::zero().into()
    }

    /// Literal one, without an allocation or a numerical evaluation.
    #[must_use]
    pub fn one() -> Self {
        Rational::one().into()
    }

    /// Admit a named mathematical constant through its typed CAS constructor.
    ///
    /// # Errors
    /// Refuses exhausted graph, arithmetic or proof budgets.
    pub fn named(value: Constant, budget: &mut Budget) -> Result<Self, Fault> {
        graph::construct(Operator::Constant(value), &[], budget)
    }

    /// Retain a checked closed expression independently of the caller's mutable store.
    ///
    /// # Errors
    /// Refuses foreign handles, non-real or open syntax, unsupported operators,
    /// unproved divisor domains and exhausted construction/proof budgets.
    pub fn from_expression(cas: &Cas, root: &Handle, budget: &mut Budget) -> Result<Self, Fault> {
        graph::admit(cas, root, budget)
    }

    /// Extract a rational only when exact CAS normalization produced a rational literal.
    #[must_use]
    pub const fn as_rational(&self) -> Option<&Rational> {
        match &self.0 {
            Value::Rational(value) => Some(value),
            Value::Expression(_) => None,
        }
    }

    /// Inspect the immutable normalized CAS graph of a non-rational expression.
    #[must_use]
    pub fn graph(&self) -> Option<(&Cas, &Handle)> {
        match &self.0 {
            Value::Expression(value) => Some((&value.cas, &value.root)),
            Value::Rational(_) => None,
        }
    }

    /// Whether normalization established the exact additive identity.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.as_rational().is_some_and(Rational::is_zero)
    }

    /// Whether the closed sign checker established strict negativity.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.sign() == sign::Sign::Negative
    }

    /// Export exact expression drafts in child-before-parent order for a receiving header.
    ///
    /// Local operands index this returned batch; the root is its last expression.
    /// # Errors
    /// Refuses exhausted traversal, payload or work budgets.
    pub fn expression_drafts(
        &self,
        header: crate::graph::Reference,
        budget: &mut Budget,
    ) -> Result<Vec<crate::graph::ExpressionDraft>, Fault> {
        graph::drafts(self, header, budget)
    }

    /// Obtain a positive finite witness only from the checked mathematical sign.
    #[must_use]
    pub fn positive(&self) -> Option<PositiveConstant> {
        (self.sign() == sign::Sign::Positive).then(|| PositiveConstant(self.clone()))
    }

    /// Read the sign established by the closed checker, never by a display name.
    fn sign(&self) -> sign::Sign {
        match &self.0 {
            Value::Rational(value) => sign::Sign::rational(value),
            Value::Expression(value) => value.sign,
        }
    }
}

/// Private-construction witness that this exact constant is finite and strictly positive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositiveConstant(ExactConstant);

impl PositiveConstant {
    /// The literal-one axiom is positive without a graph traversal.
    #[must_use]
    pub fn one() -> Self {
        Self(ExactConstant::one())
    }

    /// Exact expression certified by this witness.
    #[must_use]
    pub const fn value(&self) -> &ExactConstant {
        &self.0
    }
}
