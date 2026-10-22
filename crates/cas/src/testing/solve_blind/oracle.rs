// SPDX-License-Identifier: AGPL-3.0-only
//! Independent small rational quotient arithmetic, invoked only after blind search returns.

use crate::solving::{CoefficientDomainData, CoefficientValueData, SolveView};
use crate::testing::common::TestResult;
use crate::{Cas, Handle};

/// Reduced rational with checked native integer arithmetic, independent of the CAS coefficient engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rational {
    /// Signed numerator.
    numerator: i128,
    /// Positive denominator.
    denominator: i128,
}

impl Rational {
    /// Canonical small integer.
    const fn integer(value: i128) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }

    /// Reduce by Euclid without calling any solver arithmetic.
    fn new(numerator: i128, denominator: i128) -> TestResult<Self> {
        if denominator <= 0 {
            return Err("nonpositive oracle denominator".into());
        }
        let (mut left, mut right) = (
            numerator.checked_abs().ok_or("oracle magnitude")?,
            denominator,
        );
        while right != 0 {
            (left, right) = (right, left % right);
        }
        Ok(Self {
            numerator: numerator / left,
            denominator: denominator / left,
        })
    }

    /// Independent Euclidean cancellation keeps native test arithmetic bounded before products.
    fn gcd(left: i128, right: i128) -> TestResult<i128> {
        let (mut first, mut second) = (left.checked_abs().ok_or("oracle gcd magnitude")?, right);
        while second != 0 {
            (first, second) = (second, first % second);
        }
        Ok(first)
    }

    /// Use the denominator lcm before addition; equality still compares canonical exact fractions.
    fn add(self, other: Self) -> TestResult<Self> {
        let common = Self::gcd(self.denominator, other.denominator)?;
        let left = self
            .numerator
            .checked_mul(other.denominator / common)
            .ok_or("oracle addition")?;
        let right = other
            .numerator
            .checked_mul(self.denominator / common)
            .ok_or("oracle addition")?;
        Self::new(
            left.checked_add(right).ok_or("oracle addition")?,
            self.denominator
                .checked_mul(other.denominator / common)
                .ok_or("oracle denominator")?,
        )
    }

    /// Cross-cancel exact integer factors before bounded native multiplication.
    fn mul(self, other: Self) -> TestResult<Self> {
        let first = Self::gcd(self.numerator, other.denominator)?;
        let second = Self::gcd(other.numerator, self.denominator)?;
        Self::new(
            (self.numerator / first)
                .checked_mul(other.numerator / second)
                .ok_or("oracle product")?,
            (self.denominator / second)
                .checked_mul(other.denominator / first)
                .ok_or("oracle denominator")?,
        )
    }

    /// Exact quotient normalizes the sign and rejects a specialized zero denominator.
    fn div(self, other: Self) -> TestResult<Self> {
        if other.numerator == 0 {
            return Err("oracle zero divisor".into());
        }
        let sign = if other.numerator < 0 { -1 } else { 1 };
        self.mul(Self::new(
            sign * other.denominator,
            other
                .numerator
                .checked_abs()
                .ok_or("oracle quotient magnitude")?,
        )?)
    }

    /// Negation is also checked, so malformed huge fixtures fail as values.
    fn neg(self) -> TestResult<Self> {
        Self::new(
            self.numerator.checked_neg().ok_or("oracle negation")?,
            self.denominator,
        )
    }
}

/// Two independent exact rational coordinates; no producer Gaussian operation is used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Complex {
    real: Rational,
    imaginary: Rational,
}

impl Complex {
    const fn integer(value: i128) -> Self {
        Self {
            real: Rational::integer(value),
            imaginary: Rational::integer(0),
        }
    }
    fn add(self, other: Self) -> TestResult<Self> {
        Ok(Self {
            real: self.real.add(other.real)?,
            imaginary: self.imaginary.add(other.imaginary)?,
        })
    }
    fn neg(self) -> TestResult<Self> {
        Ok(Self {
            real: self.real.neg()?,
            imaginary: self.imaginary.neg()?,
        })
    }
    fn mul(self, other: Self) -> TestResult<Self> {
        Ok(Self {
            real: self
                .real
                .mul(other.real)?
                .add(self.imaginary.mul(other.imaginary)?.neg()?)?,
            imaginary: self
                .real
                .mul(other.imaginary)?
                .add(self.imaginary.mul(other.real)?)?,
        })
    }
    fn div(self, other: Self) -> TestResult<Self> {
        let norm = other
            .real
            .mul(other.real)?
            .add(other.imaginary.mul(other.imaginary)?)?;
        let product = self.mul(Self {
            real: other.real,
            imaginary: other.imaginary.neg()?,
        })?;
        Ok(Self {
            real: product.real.div(norm)?,
            imaginary: product.imaginary.div(norm)?,
        })
    }
    fn pow(self, exponent: u32) -> TestResult<Self> {
        let mut result = Self::integer(1);
        for _ in 0..exponent {
            result = result.mul(self)?;
        }
        Ok(result)
    }
}

/// A recursively defined monic quotient, represented by a flat rational basis.
struct Ring {
    /// Exact graph algebra whose basis is decoded independently.
    domain: Handle,
    /// Earlier coefficient ring, absent for the rational ground field.
    parent: Option<Box<Self>>,
    /// Coefficients of the monic defining polynomial, including its leading one.
    modulus: Vec<Vec<Rational>>,
    /// Total dimension of the flat rational basis.
    dimension: usize,
    /// Independent coordinate samples, present only for a rational function ground field.
    parameters: Vec<Complex>,
}

impl Ring {
    /// Decode only graph syntax; no normalization or solving routine supplies this oracle.
    fn read(cas: &Cas, domain: &Handle) -> TestResult<Self> {
        Self::specialized(cas, domain, &[], domain)
    }

    /// Bind parameters by declaration identity and follow every actual quotient parent.
    fn specialized(
        cas: &Cas,
        domain: &Handle,
        bindings: &[(&str, Complex)],
        original: &Handle,
    ) -> TestResult<Self> {
        match cas.solve_view(domain)? {
            SolveView::CoefficientDomain(
                ground @ (CoefficientDomainData::Rational | CoefficientDomainData::Gaussian),
            ) => Ok(Self {
                domain: domain.clone(),
                parent: None,
                parameters: Vec::new(),
                modulus: Vec::new(),
                dimension: if *ground == CoefficientDomainData::Gaussian {
                    2
                } else {
                    1
                },
            }),
            SolveView::CoefficientDomain(CoefficientDomainData::Fraction { base, coordinates }) => {
                assert_eq!(domain, original);
                Self::fraction(cas, domain, base, coordinates, bindings)
            }
            SolveView::CoefficientDomain(CoefficientDomainData::Quotient { level }) => {
                Self::extension(cas, domain, level, bindings, original)
            }
            _ => Err("unsupported oracle algebra".into()),
        }
    }

    /// The only specialized ground is the original parameter frame; no unrelated fraction descends.
    fn fraction(
        cas: &Cas,
        domain: &Handle,
        base: &Handle,
        coordinates: &Handle,
        bindings: &[(&str, Complex)],
    ) -> TestResult<Self> {
        assert!(matches!(
            cas.solve_view(base)?,
            SolveView::CoefficientDomain(CoefficientDomainData::Rational)
        ));
        let SolveView::CoordinateSpace(frame) = cas.solve_view(coordinates)? else {
            return Err("oracle parameter frame".into());
        };
        let parameters = frame
            .coordinates
            .iter()
            .map(|coordinate| {
                bindings
                    .iter()
                    .find(|(key, _)| *key == coordinate.key)
                    .map(|(_, value)| *value)
                    .ok_or_else(|| "missing oracle parameter".into())
            })
            .collect::<TestResult<Vec<_>>>()?;
        Ok(Self {
            domain: domain.clone(),
            parent: None,
            parameters,
            modulus: Vec::new(),
            dimension: 2,
        })
    }

    /// Decode the defining monic polynomial directly in its exact earlier parent basis.
    fn extension(
        cas: &Cas,
        domain: &Handle,
        level: &Handle,
        bindings: &[(&str, Complex)],
        original: &Handle,
    ) -> TestResult<Self> {
        let SolveView::ExtensionLevel(level) = cas.solve_view(level)? else {
            return Err("oracle extension".into());
        };
        let parent = Self::specialized(cas, &level.parent, bindings, original)?;
        let SolveView::Polynomial(polynomial) = cas.solve_view(&level.polynomial)? else {
            return Err("oracle modulus".into());
        };
        let degree = polynomial
            .terms
            .iter()
            .filter_map(|term| term.exponents.first())
            .max()
            .copied()
            .ok_or("empty modulus")?;
        let degree = usize::try_from(degree)?;
        let mut modulus = vec![parent.zero(); degree + 1];
        for term in &polynomial.terms {
            let [power] = term.exponents.as_slice() else {
                return Err("multivariate modulus".into());
            };
            modulus[usize::try_from(*power)?] = parent.value(cas, &term.coefficient)?;
        }
        if modulus.last() != Some(&parent.one()) {
            return Err("nonmonic modulus".into());
        }
        let dimension = degree
            .checked_mul(parent.dimension)
            .ok_or("oracle dimension")?;
        Ok(Self {
            domain: domain.clone(),
            parent: Some(Box::new(parent)),
            parameters: Vec::new(),
            modulus,
            dimension,
        })
    }

    /// The rational zero vector in the declared finite basis.
    fn zero(&self) -> Vec<Rational> {
        vec![Rational::integer(0); self.dimension]
    }

    /// The rational one vector in the declared finite basis.
    fn one(&self) -> Vec<Rational> {
        let mut value = self.zero();
        value[0] = Rational::integer(1);
        value
    }

    /// Read canonical coefficient payloads without using their producer's arithmetic.
    fn value(&self, cas: &Cas, handle: &Handle) -> TestResult<Vec<Rational>> {
        let SolveView::CoefficientValue(value) = cas.solve_view(handle)? else {
            return Err("oracle coefficient".into());
        };
        assert_eq!(value.domain, self.domain);
        match (&self.parent, &value.value) {
            (None, CoefficientValueData::Rational { value }) => Ok(vec![number(value)?]),
            (None, CoefficientValueData::Gaussian { real, imaginary }) => {
                Ok(vec![number(real)?, number(imaginary)?])
            }
            (
                None,
                CoefficientValueData::Fraction {
                    numerator,
                    denominator,
                },
            ) => {
                let value = self
                    .evaluate(cas, numerator)?
                    .div(self.evaluate(cas, denominator)?)?;
                Ok(vec![value.real, value.imaginary])
            }
            (Some(parent), CoefficientValueData::Tower { coefficients }) => {
                let mut value = self.zero();
                for (power, coefficient) in coefficients.iter().enumerate() {
                    let offset = power.checked_mul(parent.dimension).ok_or("oracle offset")?;
                    let target = value
                        .get_mut(offset..offset + parent.dimension)
                        .ok_or("oracle residue degree")?;
                    target.copy_from_slice(&parent.value(cas, coefficient)?);
                }
                Ok(value)
            }
            _ => Err("oracle coefficient shape".into()),
        }
    }

    /// Direct monomial substitution uses only this independent small rational arithmetic.
    fn evaluate(&self, cas: &Cas, handle: &Handle) -> TestResult<Complex> {
        let SolveView::Polynomial(polynomial) = cas.solve_view(handle)? else {
            return Err("oracle polynomial".into());
        };
        let mut sum = Complex::integer(0);
        for term in &polynomial.terms {
            let SolveView::CoefficientValue(coefficient) = cas.solve_view(&term.coefficient)?
            else {
                return Err("oracle polynomial scalar".into());
            };
            let CoefficientValueData::Rational { value } = &coefficient.value else {
                return Err("oracle rational scalar".into());
            };
            assert_eq!(term.exponents.len(), self.parameters.len());
            let mut product = Complex {
                real: number(value)?,
                imaginary: Rational::integer(0),
            };
            for (exponent, image) in term.exponents.iter().zip(&self.parameters) {
                product = product.mul(image.pow(*exponent)?)?;
            }
            sum = sum.add(product)?;
        }
        Ok(sum)
    }

    /// Addition is componentwise in the finite monomial basis.
    fn add(&self, left: &[Rational], right: &[Rational]) -> TestResult<Vec<Rational>> {
        if (left.len(), right.len()) != (self.dimension, self.dimension) {
            return Err("oracle addition shape".into());
        }
        left.iter()
            .zip(right)
            .map(|(left, right)| left.add(*right))
            .collect()
    }

    /// Multiply in the parent ring, then independently reduce by the defining monic polynomial.
    fn mul(&self, left: &[Rational], right: &[Rational]) -> TestResult<Vec<Rational>> {
        if (left.len(), right.len()) != (self.dimension, self.dimension) {
            return Err("oracle product shape".into());
        }
        let Some(parent) = &self.parent else {
            return match (left, right) {
                ([first], [second]) => Ok(vec![first.mul(*second)?]),
                ([real, imaginary], [other_real, other_imaginary]) => Ok(vec![
                    real.mul(*other_real)?
                        .add(imaginary.mul(*other_imaginary)?.neg()?)?,
                    real.mul(*other_imaginary)?
                        .add(imaginary.mul(*other_real)?)?,
                ]),
                _ => Err("oracle ground shape".into()),
            };
        };
        let degree = self.modulus.len() - 1;
        let mut product = vec![parent.zero(); degree * 2 - 1];
        for (row, coefficient) in left.chunks(parent.dimension).enumerate() {
            for (column, other) in right.chunks(parent.dimension).enumerate() {
                product[row + column] =
                    parent.add(&product[row + column], &parent.mul(coefficient, other)?)?;
            }
        }
        for power in (degree..product.len()).rev() {
            for (index, modulus) in self.modulus.iter().take(degree).enumerate() {
                let multiple = parent.mul(&product[power], modulus)?;
                let negative = multiple
                    .into_iter()
                    .map(Rational::neg)
                    .collect::<TestResult<Vec<_>>>()?;
                let target = power - degree + index;
                product[target] = parent.add(&product[target], &negative)?;
            }
        }
        Ok(product.into_iter().take(degree).flatten().collect())
    }
}

/// Serialization to small integers is the only numeric service shared with the checked implementation.
fn number(value: &crate::Rational) -> TestResult<Rational> {
    let mut budget = crate::limits::Budget::new(crate::limits::Limits::blind_cubic())?;
    let (numerator, denominator) = value
        .small_ratio(&mut budget)?
        .ok_or("oracle scalar too large")?;
    Rational::new(i128::from(numerator), i128::from(denominator))
}

/// Recover the actual retained factors, including multiplicity, from the sealed coverage application.
pub(crate) fn product(
    cas: &Cas,
    witness: &crate::proof::VerifiedWitness,
    original: &[i64],
) -> TestResult {
    let coefficients: Vec<_> = original.iter().map(|value| (*value, 0)).collect();
    gaussian_product(cas, witness, &coefficients)
}

/// Both Gaussian coordinates survive the independent finite-basis multiplication.
pub(crate) fn gaussian_product(
    cas: &Cas,
    witness: &crate::proof::VerifiedWitness,
    original: &[(i64, i64)],
) -> TestResult {
    use crate::graph::{ExpressionAttachment, Vertex};
    use crate::proof_model::Certificate;
    use crate::solving::SolveCertificate;
    let Vertex::Proof(application) = cas.state.node(witness.proof.logical_id())? else {
        return Err("oracle proof root".into());
    };
    let Vertex::ScopedApplication(application) = cas.state.node(*application)? else {
        return Err("oracle coverage application".into());
    };
    let Certificate::Solve(certificate) = &application.certificate else {
        return Err("oracle solve certificate".into());
    };
    let SolveCertificate::ProductCompleteness(certificate) = certificate.as_ref() else {
        return Err("oracle complete product".into());
    };
    let coefficient = |handle: &Handle| -> TestResult<Handle> {
        let expression = cas.state.expression(handle.logical_id())?;
        let Some(ExpressionAttachment::ExactCoefficient { value, .. }) = &expression.attachment
        else {
            return Err("oracle exact factor".into());
        };
        Ok(cas.handle(*value))
    };
    let scalar = coefficient(&certificate.scalar)?;
    let SolveView::CoefficientValue(value) = cas.solve_view(&scalar)? else {
        return Err("oracle scalar".into());
    };
    let ring = Ring::read(cas, &value.domain)?;
    let mut polynomial = vec![ring.value(cas, &scalar)?];
    for factor in &certificate.factors {
        let root = ring.value(cas, &coefficient(&factor.root)?)?;
        for _ in 0..factor.multiplicity {
            polynomial = multiply_factor(&ring, &polynomial, &root)?;
        }
    }
    compare(&ring, &polynomial, original)
}

/// Compare every original coefficient and its independently differentiated counterpart.
fn compare(ring: &Ring, polynomial: &[Vec<Rational>], original: &[(i64, i64)]) -> TestResult {
    if polynomial.len() != original.len() {
        return Err("oracle full degree".into());
    }
    for (power, (actual, expected)) in polynomial.iter().zip(original).enumerate() {
        let mut target = ring.zero();
        target[0] = Rational::integer(i128::from(expected.0));
        if expected.1 != 0 {
            let mut ground = ring;
            while let Some(parent) = &ground.parent {
                ground = parent;
            }
            assert_eq!(ground.dimension, 2);
            *target.get_mut(1).ok_or("oracle imaginary coordinate")? =
                Rational::integer(i128::from(expected.1));
        }
        assert_eq!(*actual, target);
        let derivative = actual
            .iter()
            .map(|value| value.mul(Rational::integer(i128::try_from(power)?)))
            .collect::<TestResult<Vec<_>>>()?;
        for coefficient in &mut target {
            *coefficient = coefficient.mul(Rational::integer(i128::try_from(power)?))?;
        }
        assert_eq!(derivative, target);
    }
    Ok(())
}

/// Convolution by the actual returned factor, independent of the CAS dense-polynomial implementation.
fn multiply_factor(
    ring: &Ring,
    polynomial: &[Vec<Rational>],
    root: &[Rational],
) -> TestResult<Vec<Vec<Rational>>> {
    let negative = root
        .iter()
        .copied()
        .map(Rational::neg)
        .collect::<TestResult<Vec<_>>>()?;
    let mut product = vec![ring.zero(); polynomial.len() + 1];
    for (power, coefficient) in polynomial.iter().enumerate() {
        product[power] = ring.add(&product[power], &ring.mul(coefficient, &negative)?)?;
        product[power + 1] = ring.add(&product[power + 1], coefficient)?;
    }
    Ok(product)
}

#[path = "specialize.rs"]
mod specialize;
pub(crate) use specialize::{guards, samples};

#[path = "real_oracle.rs"]
mod real;
pub(crate) use real::samples as real_samples;
