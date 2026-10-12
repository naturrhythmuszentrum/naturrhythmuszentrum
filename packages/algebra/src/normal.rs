// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic non-expanding collection on common domains of definition.

use std::collections::BTreeMap;

use crate::{ArithmeticError, Error, Expr, Limits, Node, Rational, Symbol};

/// Canonical scalar structure, independent of source locations.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Form {
    /// Reduced coefficient.
    Constant(Rational),
    /// Opaque variable.
    Symbol(Symbol),
    /// Uninterpreted function with normalized arguments.
    Call(Symbol, Vec<Self>),
    /// Sorted coefficient-free terms with leading coefficient one.
    Sum(BTreeMap<Self, Rational>),
    /// Nonzero coefficient and sorted bases with nonzero exponents.
    Product(Rational, BTreeMap<Self, Rational>),
}

/// Equality under collection and integer-power rules, on common defined domains.
///
/// This form does not retain the original operations or their domain restrictions.
/// A client performing typed checks must retain and check its original `Expr` first.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NormalForm(Form);

impl NormalForm {
    /// Visit every original operation and produce its collection form.
    ///
    /// # Errors
    /// Refuses arithmetic failures, literal undefined powers, and budget exhaustion.
    pub fn new(expression: &Expr, limits: Limits) -> Result<Self, Error> {
        expression.validate_limits(limits)?;
        let mut budget = Budget(limits.work);
        Ok(Self(normalize(expression, &mut budget)?))
    }

    /// Express this form as a fresh algebraic tree, without original source evidence.
    ///
    /// This is not a substitute for the original expression in a typed client.
    ///
    /// # Errors
    /// Refuses a reconstructed tree exceeding the supplied limits.
    pub fn expression(&self, limits: Limits) -> Result<Expr, Error> {
        rebuild(&self.0, limits)
    }

    /// Return a rational only when the complete form is a constant.
    #[must_use]
    pub const fn constant(&self) -> Option<Rational> {
        if let Form::Constant(value) = self.0 {
            Some(value)
        } else {
            None
        }
    }
}

/// Remaining normalization steps.
struct Budget(usize);

impl Budget {
    /// Charge one visited node or collected term/factor.
    fn charge(&mut self) -> Result<(), Error> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or(Error::Limit("normalization work"))?;
        Ok(())
    }
}

/// Normalize bottom-up; even annihilated operands must be visited.
fn normalize(expression: &Expr, budget: &mut Budget) -> Result<Form, Error> {
    budget.charge()?;
    match expression.node() {
        Node::Constant(value) => Ok(Form::Constant(*value)),
        Node::Symbol(name) => Ok(Form::Symbol(name.clone())),
        Node::Neg(value) => {
            let value = normalize(value, budget)?;
            sum([(value, Rational::integer(-1))], budget)
        }
        Node::Add(a, b) | Node::Sub(a, b) => {
            let left = normalize(a, budget)?;
            let right = normalize(b, budget)?;
            let sign = if matches!(expression.node(), Node::Sub(_, _)) {
                Rational::integer(-1)
            } else {
                Rational::ONE
            };
            sum([(left, Rational::ONE), (right, sign)], budget)
        }
        Node::Mul(a, b) | Node::Div(a, b) => {
            let left = normalize(a, budget)?;
            let right = normalize(b, budget)?;
            let division = matches!(expression.node(), Node::Div(_, _));
            if division && right == Form::Constant(Rational::ZERO) {
                return Err(ArithmeticError::ZeroDenominator.into());
            }
            product(
                [
                    (left, Rational::ONE),
                    (right, Rational::integer(if division { -1 } else { 1 })),
                ],
                budget,
            )
        }
        Node::Power(base, exponent) => {
            let base = normalize(base, budget)?;
            product([(base, *exponent)], budget)
        }
        Node::Call(name, arguments) => Ok(Form::Call(
            name.clone(),
            arguments
                .iter()
                .map(|arg| normalize(arg, budget))
                .collect::<Result<_, _>>()?,
        )),
    }
}

/// Collect scaled terms without distributing across products.
fn sum(
    values: impl IntoIterator<Item = (Form, Rational)>,
    budget: &mut Budget,
) -> Result<Form, Error> {
    let mut terms = BTreeMap::new();
    for (form, scale) in values {
        collect_term(form, scale, &mut terms, budget)?;
    }
    if terms.is_empty() {
        return Ok(Form::Constant(Rational::ZERO));
    }
    if terms.len() == 1
        && let Some((term, coefficient)) = terms.pop_first()
    {
        return product(
            [
                (Form::Constant(coefficient), Rational::ONE),
                (term, Rational::ONE),
            ],
            budget,
        );
    }
    // Factoring a deterministic rational content preserves both additive flattening
    // and product associativity without distributing symbolic factors over sums.
    let content = terms
        .first_key_value()
        .map_or(Rational::ONE, |(_, value)| *value);
    for coefficient in terms.values_mut() {
        budget.charge()?;
        *coefficient = coefficient.checked_div(content)?;
    }
    product(
        [
            (Form::Constant(content), Rational::ONE),
            (Form::Sum(terms), Rational::ONE),
        ],
        budget,
    )
}

/// Insert one term and flatten an existing sum.
fn collect_term(
    form: Form,
    scale: Rational,
    terms: &mut BTreeMap<Form, Rational>,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.charge()?;
    let (key, coefficient) = match form {
        Form::Sum(children) => {
            for (child, coefficient) in children {
                collect_term(child, coefficient.checked_mul(scale)?, terms, budget)?;
            }
            return Ok(());
        }
        Form::Constant(value) => (Form::Constant(Rational::ONE), value.checked_mul(scale)?),
        Form::Product(coefficient, factors) => (
            finish_product(Rational::ONE, factors),
            coefficient.checked_mul(scale)?,
        ),
        other => (other, scale),
    };
    if matches!(key, Form::Sum(_)) {
        return collect_term(key, coefficient, terms, budget);
    }
    let total = terms
        .get(&key)
        .copied()
        .unwrap_or(Rational::ZERO)
        .checked_add(coefficient)?;
    if total == Rational::ZERO {
        terms.remove(&key);
    } else {
        terms.insert(key, total);
    }
    Ok(())
}

/// Collect factor exponents and an exact scalar coefficient.
fn product(
    values: impl IntoIterator<Item = (Form, Rational)>,
    budget: &mut Budget,
) -> Result<Form, Error> {
    let mut coefficient = Rational::ONE;
    let mut factors = BTreeMap::new();
    for (base, exponent) in values {
        collect_factor(base, exponent, &mut coefficient, &mut factors, budget)?;
    }
    Ok(finish_product(coefficient, factors))
}

/// Insert a power, distributing only integer outer powers over products.
fn collect_factor(
    base: Form,
    exponent: Rational,
    coefficient: &mut Rational,
    factors: &mut BTreeMap<Form, Rational>,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.charge()?;
    if base == Form::Constant(Rational::ZERO) {
        if exponent <= Rational::ZERO {
            return Err(ArithmeticError::UndefinedPower.into());
        }
        *coefficient = Rational::ZERO;
        return Ok(());
    }
    if exponent == Rational::ZERO || base == Form::Constant(Rational::ONE) {
        return Ok(());
    }
    if exponent.is_integer() {
        match base {
            Form::Constant(value) => {
                *coefficient = coefficient.checked_mul(value.checked_pow(exponent.numerator())?)?;
                return Ok(());
            }
            Form::Product(value, children) => {
                *coefficient = coefficient.checked_mul(value.checked_pow(exponent.numerator())?)?;
                for (child, power) in children {
                    collect_factor(
                        child,
                        power.checked_mul(exponent)?,
                        coefficient,
                        factors,
                        budget,
                    )?;
                }
                return Ok(());
            }
            _ => {}
        }
    }
    let total = factors
        .get(&base)
        .copied()
        .unwrap_or(Rational::ZERO)
        .checked_add(exponent)?;
    factors.remove(&base);
    if total != Rational::ZERO {
        // A newly integral outer power must expose the underlying product again.
        if matches!(base, Form::Product(_, _)) && total.is_integer() {
            collect_factor(base, total, coefficient, factors, budget)?;
        } else if let Form::Constant(value) = base {
            let integer = total.numerator().div_euclid(total.denominator());
            let remainder = total.numerator().rem_euclid(total.denominator());
            *coefficient = coefficient.checked_mul(value.checked_pow(integer)?)?;
            if remainder != 0 {
                factors.insert(
                    Form::Constant(value),
                    Rational::new(remainder, total.denominator())?,
                );
            }
        } else {
            factors.insert(base, total);
        }
    }
    Ok(())
}

/// Remove redundant product structure.
fn finish_product(coefficient: Rational, mut factors: BTreeMap<Form, Rational>) -> Form {
    if coefficient == Rational::ZERO || factors.is_empty() {
        return Form::Constant(coefficient);
    }
    if coefficient == Rational::ONE
        && factors.len() == 1
        && let Some((base, exponent)) = factors.pop_first()
    {
        if exponent == Rational::ONE {
            return base;
        }
        factors.insert(base, exponent);
    }
    Form::Product(coefficient, factors)
}

/// Reconstruct a canonical expression using the ordinary checked builders.
fn rebuild(form: &Form, limits: Limits) -> Result<Expr, Error> {
    let node = match form {
        Form::Constant(value) => Node::Constant(*value),
        Form::Symbol(name) => Node::Symbol(name.clone()),
        Form::Call(name, arguments) => Node::Call(
            name.clone(),
            arguments
                .iter()
                .map(|arg| rebuild(arg, limits))
                .collect::<Result<_, _>>()?,
        ),
        Form::Sum(terms) => {
            let mut result = None;
            for (term, coefficient) in terms {
                let item = Expr::new(
                    Node::Mul(
                        Box::new(Expr::new(Node::Constant(*coefficient), limits)?),
                        Box::new(rebuild(term, limits)?),
                    ),
                    limits,
                )?;
                result = Some(match result {
                    None => item,
                    Some(left) => Expr::new(Node::Add(Box::new(left), Box::new(item)), limits)?,
                });
            }
            return result.ok_or(Error::Limit("empty canonical sum"));
        }
        Form::Product(coefficient, factors) => {
            let mut result = Expr::new(Node::Constant(*coefficient), limits)?;
            for (base, exponent) in factors {
                let item = Expr::new(
                    Node::Power(Box::new(rebuild(base, limits)?), *exponent),
                    limits,
                )?;
                result = Expr::new(Node::Mul(Box::new(result), Box::new(item)), limits)?;
            }
            return Ok(result);
        }
    };
    Expr::new(node, limits)
}
