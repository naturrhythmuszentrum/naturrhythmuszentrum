// SPDX-License-Identifier: AGPL-3.0-only
//! Supplied orbit certificates from a small independent integer-pair algebra.

use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use crate::test_api::solving::{
    CertificatePermutationOrbitCoverage, PolynomialData, PolynomialTerm, SolveCertificate,
    SolveDraft, SolveSchema, SolveStatement, StatementOrbit,
};
use crate::test_api::syntax::{Draft, Field};
use crate::test_api::{Handle, Reference};
use crate::testing::common::TestResult;
use crate::testing::solve_coefficients::Fixture as Coefficients;
use crate::testing::solve_graph::admit;
use std::collections::BTreeMap;

/// Exact integer coordinates in the supplied quadratic algebra.
type Pair = (i64, i64);
/// Homogeneous cubic-root polynomials for the independent public oracle.
type Polynomial = BTreeMap<[u32; 3], Pair>;

/// Independent enumeration rows kept aligned.
struct Enumeration {
    /// Independently multiplied distinct orbit values.
    members: Vec<Polynomial>,
    /// Generator images aligned with those members.
    actions: Vec<Vec<u32>>,
    /// Reaching words aligned with those members.
    words: Vec<Vec<u32>>,
}

/// Independent multiplication in Z[t]/(t²+t+1), without any CAS arithmetic.
const fn multiply(left: Pair, right: Pair) -> Pair {
    (
        left.0 * right.0 - left.1 * right.1,
        left.0 * right.1 + left.1 * right.0 - left.1 * right.1,
    )
}

/// Sparse distributivity is the independent supplied-certificate oracle.
fn product(left: &Polynomial, right: &Polynomial) -> Polynomial {
    let mut result: Polynomial = BTreeMap::new();
    for (a, first) in left {
        for (b, second) in right {
            let key = [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
            let pair = multiply(*first, *second);
            let entry = result.entry(key).or_insert((0, 0));
            entry.0 += pair.0;
            entry.1 += pair.1;
        }
    }
    result.retain(|_, coefficient| *coefficient != (0, 0));
    result
}

/// Both orientations come from powers of the same formal primitive unity.
fn seed(orientation: u32, exponent: u32) -> Polynomial {
    let mut unity = (1, 0);
    for _ in 0..orientation {
        unity = multiply(unity, (0, 1));
    }
    let mut character = Polynomial::new();
    let mut coefficient = (1, 0);
    for index in 0..3 {
        let mut powers = [0; 3];
        powers[index] = 1;
        character.insert(powers, coefficient);
        coefficient = multiply(coefficient, unity);
    }
    let mut result = Polynomial::from([([0; 3], (1, 0))]);
    for _ in 0..exponent {
        result = product(&result, &character);
    }
    result
}

/// Adjacent swaps act directly on integer keys, independently of the replay checker.
fn action(value: &Polynomial, slot: usize) -> Polynomial {
    value
        .iter()
        .map(|(powers, coefficient)| {
            let mut image = *powers;
            image.swap(slot, slot + 1);
            (image, *coefficient)
        })
        .collect()
}

/// Test-only BFS supplies finite words and a complete table without calling an orbit producer.
fn enumerate(value: Polynomial) -> TestResult<Enumeration> {
    let mut orbit = vec![value];
    let mut words = vec![Vec::new()];
    let mut actions = Vec::new();
    let mut cursor = 0;
    while cursor < orbit.len() {
        let mut row = Vec::new();
        for slot in 0..2 {
            let image = action(orbit.get(cursor).ok_or("missing orbit row")?, slot);
            let target = if let Some(index) = orbit.iter().position(|entry| *entry == image) {
                index
            } else {
                let mut word = words.get(cursor).ok_or("missing word")?.clone();
                word.push(u32::try_from(slot)?);
                words.push(word);
                orbit.push(image);
                orbit.len() - 1
            };
            row.push(u32::try_from(target)?);
        }
        actions.push(row);
        cursor += 1;
    }
    Ok(Enumeration {
        members: orbit,
        actions,
        words,
    })
}

/// Turn an independently computed pair polynomial into canonical public syntax.
fn polynomial(base: &mut Coefficients, domain: &Handle, value: &Polynomial) -> TestResult<Handle> {
    let mut terms = Vec::new();
    for (powers, pair) in value.iter().rev() {
        let mut entries = vec![base.integer(pair.0)?];
        if pair.1 != 0 {
            entries.push(base.integer(pair.1)?);
        }
        let coefficient = base.tower_value(domain, &entries)?;
        terms.push(PolynomialTerm {
            exponents: powers.to_vec(),
            coefficient: Reference::Existing(coefficient),
        });
    }
    admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
            domain: Reference::Existing(domain.clone()),
            variables: vec![Field::Complex; 3],
            terms,
        })),
    )
}

/// Public supplied evidence, with no production algorithm in its dependency path.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Exact graph and empty-assumption coefficient context.
    pub(crate) base: Coefficients,
    /// Untrusted orbit table and reaching words.
    pub(crate) certificate: CertificatePermutationOrbitCoverage,
}

impl Fixture {
    /// Construct either cubic character orientation and a positive power by multiplication.
    pub(crate) fn new(orientation: u32, exponent: u32) -> TestResult<Self> {
        let mut base = Coefficients::new()?;
        let modulus = base.polynomial(1, &[(vec![2], 1), (vec![1], 1), (vec![0], 1)])?;
        let rational = base.rational.clone();
        let (_, domain) = base.tower(&rational, &modulus)?;
        let enumeration = enumerate(seed(orientation, exponent))?;
        let mut members = Vec::new();
        for value in &enumeration.members {
            members.push(polynomial(&mut base, &domain, value)?);
        }
        let seed_handle = members.first().ok_or("missing seed")?.clone();
        Ok(Self {
            base,
            certificate: CertificatePermutationOrbitCoverage {
                seed: seed_handle,
                degree: 3,
                generators: vec![vec![1, 0, 2], vec![0, 2, 1]],
                members,
                actions: enumeration.actions,
                words: enumeration.words,
            },
        })
    }

    /// Repeat precisely the proposed certificate in its untrusted conclusion.
    pub(crate) fn step(&self) -> ApplicationDraft {
        ApplicationDraft {
            rule: Primitive::Solve(SolveSchema::PermutationOrbitCoverage),
            context: self.base.context.clone(),
            conclusion: Statement::Solve(Box::new(SolveStatement::Orbit(StatementOrbit {
                seed: self.certificate.seed.clone(),
                generators: self.certificate.generators.clone(),
                members: self.certificate.members.clone(),
                actions: self.certificate.actions.clone(),
            }))),
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Solve(Box::new(SolveCertificate::PermutationOrbitCoverage(
                self.certificate.clone(),
            ))),
        }
    }
}
