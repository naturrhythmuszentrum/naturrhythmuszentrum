// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic discovery and cost observations carry no mathematical authority.

use super::discovery::Attempt;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Handle;
use crate::solving::coefficients::model::Element;
use crate::solving::coefficients::storage::reserve;
use crate::solving::symmetric::cursor::Seed;

/// Phase, region, recursion depth, optional seed, and cumulative coefficient, gcd and work counters.
type Measurement = (
    (&'static str, Option<usize>),
    Handle,
    u64,
    Option<Seed>,
    u64,
    u64,
    u64,
);

/// Exact scalar diagnostic with its phase and outer and parameter monomial coordinates.
type CoefficientTerm = (&'static str, Vec<u32>, bool, Vec<u32>, String);

/// Audit data never supplies a premise or a completeness flag.
#[derive(Debug, Default)]
pub(crate) struct Audit {
    /// Seeds visited by recursive discovery, in deterministic order.
    pub(crate) attempts: Vec<Attempt>,
    /// Exponents of the last discovered cross-character invariant.
    pub(crate) pairing_powers: Vec<u32>,
    /// First unresolved base coefficient offered for a regional split.
    pub(crate) pending: Option<Element>,
    /// Consecutive cumulative snapshots attribute actual costs without estimating arithmetic.
    pub(crate) phases: Vec<Measurement>,
    /// Stage, maximum retained rational component bits, and its exact scalar spelling.
    pub(crate) components: Vec<(&'static str, u64, String)>,
    /// Exact retained rational terms: stage, outer slots, denominator flag, parameter powers, scalar.
    pub(crate) coefficient_terms: Vec<CoefficientTerm>,
}

impl Audit {
    /// Mark the start of a phase; the final ledger also measures a phase that returns an error.
    pub(crate) fn mark(
        &mut self,
        phase: &'static str,
        region: &Handle,
        depth: u64,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        self.record((phase, None), region, depth, None, budget)
    }

    /// Indexed owned proof phases preserve the coefficient that was being prepared.
    pub(crate) fn indexed(
        &mut self,
        phase: &'static str,
        index: usize,
        region: &Handle,
        depth: u64,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        self.record((phase, Some(index)), region, depth, None, budget)
    }

    /// Seed observations preserve the actual cursor choice even when expansion exhausts.
    pub(crate) fn seed(
        &mut self,
        phase: (&'static str, Option<usize>),
        region: &Handle,
        depth: u64,
        seed: &Seed,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let seed = match seed {
            Seed::Cyclic { index, power } => Seed::Cyclic {
                index: *index,
                power: *power,
            },
            Seed::Partition(parts) => Seed::Partition(
                crate::solving::coefficients::storage::copied(parts, budget)?,
            ),
        };
        self.record(phase, region, depth, Some(seed), budget)
    }

    /// Reserve each retained snapshot before observing its cumulative counters.
    fn record(
        &mut self,
        phase: (&'static str, Option<usize>),
        region: &Handle,
        depth: u64,
        seed: Option<Seed>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        reserve(&mut self.phases, 1, budget)?;
        let usage = budget.usage();
        self.phases.push((
            phase,
            region.clone(),
            depth,
            seed,
            usage.coefficient_ops,
            usage.gcd_steps,
            usage.work,
        ));
        Ok(())
    }
}

/// Largest retained rational component, measured without normalizing any coefficient.
#[derive(Default)]
struct Largest {
    /// Maximum numerator or denominator bit width seen so far.
    bits: u64,
    /// Exact rational attaining that width, for reproducible arithmetic diagnosis.
    rational: String,
}

impl Largest {
    /// Record only a new maximum; decimal formatting itself uses the ordinary ledger.
    fn rational(
        &mut self,
        value: &crate::number::Rational,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        budget.work(1, super::AT)?;
        let (numerator, denominator) = value.component_bits();
        let bits = numerator.max(denominator);
        if bits > self.bits {
            self.rational = value.exact_string(budget)?;
            self.bits = bits;
        }
        Ok(())
    }

    /// Gaussian components are observed separately after their metered copies.
    fn scalar(
        &mut self,
        value: &crate::solving::coefficients::scalar::Scalar,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        use crate::solving::coefficients::scalar::Scalar;
        match value {
            Scalar::Rational(value) => self.rational(value, budget),
            Scalar::Gaussian(value) => {
                self.rational(&value.real(budget)?, budget)?;
                self.rational(&value.imaginary(budget)?, budget)
            }
        }
    }

    /// A bounded explicit stack visits every tower entry and fraction coefficient.
    fn element(&mut self, value: &Element, budget: &mut Budget) -> Result<(), Fault> {
        use crate::solving::coefficients::model::Value;
        let mut pending = crate::solving::coefficients::storage::vector(1, budget)?;
        pending.push(value);
        while let Some(value) = pending.pop() {
            budget.work(1, super::AT)?;
            match &value.value {
                Value::Scalar(scalar) => self.scalar(scalar, budget)?,
                Value::Fraction(fraction) => {
                    for term in fraction
                        .numerator
                        .terms
                        .iter()
                        .chain(&fraction.denominator.terms)
                    {
                        self.scalar(&term.coefficient, budget)?;
                    }
                }
                Value::Tower(coefficients) => {
                    reserve(&mut pending, coefficients.len(), budget)?;
                    pending.extend(coefficients);
                }
            }
        }
        Ok(())
    }
}

impl Audit {
    /// Stage observations carry no theorem authority and never alter arithmetic representations.
    pub(crate) fn observe(
        &mut self,
        stage: &'static str,
        polynomials: &[&crate::solving::symmetric::algebra::Polynomial],
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut largest = Largest::default();
        for polynomial in polynomials {
            for term in &polynomial.form.terms {
                largest.element(&term.coefficient, budget)?;
            }
        }
        reserve(&mut self.components, 1, budget)?;
        self.components
            .push((stage, largest.bits, largest.rational));
        Ok(())
    }
}

impl Audit {
    /// Record actual rational data before a failed core multiplication can discard its intermediate.
    pub(crate) fn describe(
        &mut self,
        stage: &'static str,
        polynomial: &crate::solving::symmetric::algebra::Polynomial,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        use crate::solving::coefficients::{model::Value, scalar::Scalar, storage::copied};
        for term in &polynomial.form.terms {
            let slots = polynomial.exponents(term, budget)?;
            match &term.coefficient.value {
                Value::Fraction(fraction) => {
                    for (denominator, value) in
                        [(false, &fraction.numerator), (true, &fraction.denominator)]
                    {
                        for entry in &value.terms {
                            budget.work(1, super::AT)?;
                            if let Scalar::Rational(value) = &entry.coefficient {
                                reserve(&mut self.coefficient_terms, 1, budget)?;
                                self.coefficient_terms.push((
                                    stage,
                                    copied(&slots, budget)?,
                                    denominator,
                                    copied(&entry.powers, budget)?,
                                    value.exact_string(budget)?,
                                ));
                            }
                        }
                    }
                }
                Value::Scalar(Scalar::Rational(value)) => {
                    reserve(&mut self.coefficient_terms, 1, budget)?;
                    self.coefficient_terms.push((
                        stage,
                        slots,
                        false,
                        Vec::new(),
                        value.exact_string(budget)?,
                    ));
                }
                Value::Scalar(Scalar::Gaussian(_)) | Value::Tower(_) => {}
            }
        }
        Ok(())
    }
}
