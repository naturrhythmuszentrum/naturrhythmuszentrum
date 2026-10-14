// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted exact multiplicity certificates and their explicit portable encoding.

use std::fmt;

use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, Location};
use crate::graph::size;
use crate::number::Rational;
use crate::univariate;
use crate::wire_read::Reader;

/// Divisibility and a Bezout combination certifying the monic gcd of two polynomials.
#[derive(Clone, Eq, PartialEq)]
pub struct GcdCertificate {
    /// Claimed monic greatest common divisor G.
    pub gcd: Vec<Rational>,
    /// U1 in U=G U1.
    pub left_quotient: Vec<Rational>,
    /// V1 in V=G V1.
    pub right_quotient: Vec<Rational>,
    /// A in A U+B V=G.
    pub bezout_left: Vec<Rational>,
    /// B in A U+B V=G.
    pub bezout_right: Vec<Rational>,
}

/// One monic, nonconstant multiplicity layer over the rational coefficient field.
#[derive(Clone, Eq, PartialEq)]
pub struct SquareFreeLayer {
    /// Strictly positive multiplicity; layers appear in increasing multiplicity order.
    pub multiplicity: u64,
    /// Ascending-degree exact coefficients, with no trailing zero.
    pub coefficients: Vec<Rational>,
}

/// A complete untrusted square-free product certificate, checked without the producer.
#[derive(Clone, Eq, PartialEq)]
pub struct SquareFreeCertificate {
    /// Nonzero leading scalar a in f=aF for monic F.
    pub scalar: Rational,
    /// Certificate for gcd(F,F'), with the derivative recomputed by the checker.
    pub initial_gcd: GcdCertificate,
    /// Certificates for successive gcd(W,C), including iterations with unit layers.
    pub rounds: Vec<GcdCertificate>,
    /// Nonconstant output factors; unit layers are omitted.
    pub layers: Vec<SquareFreeLayer>,
    /// One gcd(Z,Z')=1 certificate per layer, in the same order.
    pub square_free: Vec<GcdCertificate>,
    /// Pairwise gcd=1 certificates, ordered by increasing first and then second layer index.
    pub coprime: Vec<GcdCertificate>,
}

impl fmt::Debug for GcdCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GcdCertificate")
            .field("gcd_coefficients", &self.gcd.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for SquareFreeLayer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SquareFreeLayer")
            .field("multiplicity", &self.multiplicity)
            .field("coefficients", &self.coefficients.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for SquareFreeCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SquareFreeCertificate")
            .field("layers", &self.layers.len())
            .field("rounds", &self.rounds.len())
            .field("coprime", &self.coprime.len())
            .finish_non_exhaustive()
    }
}

/// Certificate accounting location.
const AT: Location = Location::new("multiplicity certificate");

impl GcdCertificate {
    /// Borrow the five ordered polynomial records without allocating a view.
    fn polynomials(&self) -> [&[Rational]; 5] {
        [
            &self.gcd,
            &self.left_quotient,
            &self.right_quotient,
            &self.bezout_left,
            &self.bezout_right,
        ]
    }

    /// Bound every polynomial component and all retained coefficient records before copying.
    pub(crate) fn payload(&self, budget: &mut Budget) -> Result<u64, Fault> {
        let mut bytes = 128_u64;
        for values in self.polynomials() {
            univariate::validate(values, budget)?;
            bytes = bytes
                .checked_add(univariate::payload(values, budget)?)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        }
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        Ok(bytes)
    }

    /// Emit all divisibility and Bezout evidence in a fixed order.
    fn encode(&self, out: &mut Encoder<'_>) -> Result<(), Fault> {
        for values in self.polynomials() {
            encode_polynomial(values, out)?;
        }
        Ok(())
    }

    /// Decode canonical vectors with independent count, degree, bit and allocation ceilings.
    fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        Ok(Self {
            gcd: decode_polynomial(reader)?,
            left_quotient: decode_polynomial(reader)?,
            right_quotient: decode_polynomial(reader)?,
            bezout_left: decode_polynomial(reader)?,
            bezout_right: decode_polynomial(reader)?,
        })
    }
}

impl SquareFreeCertificate {
    /// Measure and bound all caller-owned certificate data before clone or arithmetic.
    pub(crate) fn payload(&self, budget: &mut Budget) -> Result<u64, Fault> {
        self.scalar.check(budget, AT)?;
        budget.check(
            BudgetKind::PolynomialDegree,
            size(self.layers.len(), AT)?,
            AT,
        )?;
        budget.check(
            BudgetKind::PolynomialDegree,
            size(self.rounds.len(), AT)?,
            AT,
        )?;
        budget.check(
            BudgetKind::PolynomialDegree,
            size(self.square_free.len(), AT)?,
            AT,
        )?;
        budget.check(BudgetKind::ProofNodes, size(self.coprime.len(), AT)?, AT)?;
        let mut bytes = univariate::payload(std::slice::from_ref(&self.scalar), budget)?;
        bytes = bytes
            .checked_add(self.initial_gcd.payload(budget)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        for layer in &self.layers {
            budget.check(BudgetKind::PolynomialDegree, layer.multiplicity, AT)?;
            univariate::validate(&layer.coefficients, budget)?;
            bytes = bytes
                .checked_add(univariate::payload(&layer.coefficients, budget)?)
                .and_then(|n| n.checked_add(64))
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        }
        for certificate in self
            .rounds
            .iter()
            .chain(&self.square_free)
            .chain(&self.coprime)
        {
            bytes = bytes
                .checked_add(certificate.payload(budget)?)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        }
        let peak = bytes
            .checked_mul(2)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, peak, AT)?;
        budget.work(bytes, AT)?;
        Ok(bytes)
    }

    /// The new primitive alone appends this payload; older witness records remain byte-identical.
    pub(crate) fn encode(&self, out: &mut Encoder<'_>) -> Result<(), Fault> {
        out.number(&self.scalar)?;
        self.initial_gcd.encode(out)?;
        encode_gcd_list(&self.rounds, out)?;
        out.count(self.layers.len())?;
        for layer in &self.layers {
            out.unsigned(layer.multiplicity)?;
            encode_polynomial(&layer.coefficients, out)?;
        }
        encode_gcd_list(&self.square_free, out)?;
        encode_gcd_list(&self.coprime, out)
    }

    /// Decode the exact square-free schema without interpreting a producer truth flag.
    pub(crate) fn decode(reader: &mut Reader<'_, '_>) -> Result<Self, Fault> {
        let scalar = reader.rational()?;
        let initial_gcd = GcdCertificate::decode(reader)?;
        let rounds = decode_gcd_list(reader, BudgetKind::PolynomialDegree)?;
        let count = reader.count(BudgetKind::PolynomialDegree, 12, 64)?;
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            let multiplicity = reader.unsigned()?;
            reader
                .budget
                .check(BudgetKind::PolynomialDegree, multiplicity, AT)?;
            layers.push(SquareFreeLayer {
                multiplicity,
                coefficients: decode_polynomial(reader)?,
            });
        }
        let square_free = decode_gcd_list(reader, BudgetKind::PolynomialDegree)?;
        let coprime = decode_gcd_list(reader, BudgetKind::ProofNodes)?;
        Ok(Self {
            scalar,
            initial_gcd,
            rounds,
            layers,
            square_free,
            coprime,
        })
    }
}

/// Canonical dense polynomial vectors have an explicit count and exact rational components.
fn encode_polynomial(values: &[Rational], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.count(values.len())?;
    for value in values {
        out.number(value)?;
    }
    Ok(())
}

/// Decode before allocating, then reject a redundant zero tail rather than normalizing it.
fn decode_polynomial(reader: &mut Reader<'_, '_>) -> Result<Vec<Rational>, Fault> {
    // The semantic term ceiling counts nonzero coefficients, not dense zero padding.
    let count = reader.count(BudgetKind::WitnessBytes, 10, 128)?;
    reader.budget.check(
        BudgetKind::PolynomialDegree,
        size(count.saturating_sub(1), AT)?,
        AT,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        values.push(reader.rational()?);
    }
    univariate::validate(&values, reader.budget)?;
    Ok(values)
}

/// Encode an ordered sequence of exact gcd witnesses.
fn encode_gcd_list(values: &[GcdCertificate], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.count(values.len())?;
    for value in values {
        value.encode(out)?;
    }
    Ok(())
}

/// Bound the sequence and every nested vector before materializing a certificate.
fn decode_gcd_list(
    reader: &mut Reader<'_, '_>,
    kind: BudgetKind,
) -> Result<Vec<GcdCertificate>, Fault> {
    let count = reader.count(kind, 20, 256)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        values.push(GcdCertificate::decode(reader)?);
    }
    Ok(values)
}
