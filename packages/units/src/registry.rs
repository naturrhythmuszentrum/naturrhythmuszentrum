// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit semantic operation declarations and conservative builtin rules.

use std::collections::BTreeMap;

use nrz_algebra::{Rational, Symbol};

use crate::{
    Descriptor, Dimension, ExponentPolicy, Kind, Measure, Role, UncertaintyMode, UnitError,
    uncertainty,
};

/// The original binary operation being checked.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BinaryOp {
    /// Addition of compatible kinds with admissible affine roles.
    Add,
    /// Subtraction of compatible kinds with admissible affine roles.
    Subtract,
    /// Multiplicative composition under an explicit kind rule.
    Multiply,
    /// Quotient composition under an explicit kind rule.
    Divide,
}

/// A data-time semantic contract; equal dimensions never manufacture a kind.
#[derive(Clone, Debug)]
pub struct Registry {
    /// Admitted dimension exponents, independent of static representation choices.
    policy: ExponentPolicy,
    /// Invariant dimensions of declared named kinds.
    names: BTreeMap<Symbol, Dimension>,
    /// Ordered multiplication and division rules.
    binary: BTreeMap<(BinaryOp, Kind, Kind), Kind>,
    /// Declared power rules beyond the small builtin set.
    powers: BTreeMap<(Kind, Rational), Kind>,
}

impl Registry {
    /// Create an empty registry with the standard ratio, affine and uncertainty rules.
    #[must_use]
    pub const fn new(policy: ExponentPolicy) -> Self {
        Self {
            policy,
            names: BTreeMap::new(),
            binary: BTreeMap::new(),
            powers: BTreeMap::new(),
        }
    }

    /// Admit a descriptor and bind all named kinds to invariant dimensions.
    ///
    /// # Errors
    /// Refuses conflicting names, unsupported dimensions and malformed kinds.
    pub fn admit(&mut self, descriptor: &Descriptor) -> Result<(), UnitError> {
        let mut names = self.names.clone();
        Self::bind(&mut names, descriptor.kind(), self.policy)?;
        self.names = names;
        Ok(())
    }

    /// Validate a kind and collect names without partially mutating the registry.
    fn bind(
        names: &mut BTreeMap<Symbol, Dimension>,
        kind: &Kind,
        policy: ExponentPolicy,
    ) -> Result<(), UnitError> {
        kind.dimension()?.admit(policy)?;
        match kind {
            Kind::Named(name, dimension) => {
                if names.get(name).is_some_and(|old| old != dimension) {
                    return Err(UnitError::ConflictingKind(name.clone()));
                }
                names.insert(name.clone(), *dimension);
            }
            Kind::Uncertainty(_, value, other) => {
                Self::bind(names, value, policy)?;
                if let Some(other) = other {
                    Self::bind(names, other, policy)?;
                }
            }
            Kind::Ratio | Kind::Angle | Kind::Count(_, _) => {}
        }
        Ok(())
    }

    /// Register one ordered multiplicative relation after checking its dimensions.
    ///
    /// Count results require domain proofs outside this registry. Count/count division
    /// remains the explicit `count_ratio` operation and cannot be registered here.
    ///
    /// TODO ´todo:code:register-explicit-sensitivity-and-covariance-pairing´: register explicit sensitivity and covariance pairing relations without inferring semantic coercions from dimensions under rule:nrz:shape-sensitivity-propagation.
    ///
    /// # Errors
    /// Refuses additive rules, duplicate keys, count results or inconsistent dimensions.
    pub fn register_binary(
        &mut self,
        operation: BinaryOp,
        left: Kind,
        right: Kind,
        result: Kind,
    ) -> Result<(), UnitError> {
        if operation == BinaryOp::Divide
            && left.count_domain().is_some()
            && right.count_domain().is_some()
        {
            return Err(UnitError::UnsupportedKindRule);
        }
        if result.is_relative() {
            return Err(UnitError::RelativeReference);
        }
        let dimension = match operation {
            BinaryOp::Multiply => left.dimension()?.product(right.dimension()?)?,
            BinaryOp::Divide => left.dimension()?.quotient(right.dimension()?)?,
            BinaryOp::Add | BinaryOp::Subtract => return Err(UnitError::UnsupportedKindRule),
        };
        if result.count_domain().is_some() {
            return Err(UnitError::CountDomain);
        }
        if dimension != result.dimension()? {
            return Err(UnitError::DimensionMismatch);
        }
        let key = (operation, left, right);
        if self.binary.contains_key(&key) {
            return Err(UnitError::DuplicateRule);
        }
        let mut names = self.names.clone();
        for kind in [&key.1, &key.2, &result] {
            Self::bind(&mut names, kind, self.policy)?;
        }
        self.names = names;
        self.binary.insert(key, result);
        Ok(())
    }

    /// Register a power relation without inferring semantic identity from its dimension.
    ///
    /// TODO ´todo:code:keep-scalar-exponent-admission-distinct-from´: keep scalar exponent admission distinct from matrix branch and composition evidence under rule:nrz:shape-powers-and-functions.
    ///
    /// # Errors
    /// Refuses duplicate rules, count rules and incompatible dimensions or policies.
    pub fn register_power(
        &mut self,
        base: Kind,
        exponent: Rational,
        result: Kind,
    ) -> Result<(), UnitError> {
        if base == Kind::Ratio || exponent == Rational::ONE {
            return Err(UnitError::UnsupportedKindRule);
        }
        if result.is_relative() {
            return Err(UnitError::RelativeReference);
        }
        if base.count_domain().is_some() || result.count_domain().is_some() {
            return Err(UnitError::CountDomain);
        }
        if base.dimension()?.power(exponent, self.policy)? != result.dimension()? {
            return Err(UnitError::DimensionMismatch);
        }
        let key = (base, exponent);
        if self.powers.contains_key(&key) {
            return Err(UnitError::DuplicateRule);
        }
        let mut names = self.names.clone();
        Self::bind(&mut names, &key.0, self.policy)?;
        Self::bind(&mut names, &result, self.policy)?;
        self.names = names;
        self.powers.insert(key, result);
        Ok(())
    }

    /// Check a binary operation and return its descriptor in coherent coordinates.
    ///
    /// Ratio factors preserve existing uncertainty kinds. Relative conversion is an
    /// explicit operation; standard-uncertainty products need original identity evidence.
    ///
    /// TODO ´todo:code:check-each-contracted-entry-kind-before´: check each contracted entry kind before admitting a shaped operator product under rule:nrz:shape-composition.
    ///
    /// # Errors
    /// Refuses invalid affine roles, kinds, count closure or dimension arithmetic.
    pub fn binary(
        &self,
        operation: BinaryOp,
        left: &Descriptor,
        right: &Descriptor,
    ) -> Result<Descriptor, UnitError> {
        self.validate(left)?;
        self.validate(right)?;
        if matches!(operation, BinaryOp::Add | BinaryOp::Subtract) {
            return Self::additive(operation, left, right);
        }
        if left.role() != Role::Difference || right.role() != Role::Difference {
            return Err(UnitError::AffineRole);
        }
        if operation == BinaryOp::Divide
            && left.kind().count_domain().is_some()
            && right.kind().count_domain().is_some()
        {
            return Err(UnitError::CountDomain);
        }
        let kind = self.multiplicative_kind(operation, left.kind(), right.kind())?;
        if kind == *left.kind() {
            return Ok(left.coherent());
        }
        if kind == *right.kind() {
            return Ok(right.coherent());
        }
        Descriptor::new(kind, Measure::coherent(), Role::Difference)
    }

    /// Check a descriptor against the registry without changing the name inventory.
    pub(super) fn validate(&self, descriptor: &Descriptor) -> Result<(), UnitError> {
        Self::bind(&mut self.names.clone(), descriptor.kind(), self.policy)
    }

    /// Apply the affine table and whole-domain count closure.
    fn additive(
        operation: BinaryOp,
        left: &Descriptor,
        right: &Descriptor,
    ) -> Result<Descriptor, UnitError> {
        left.compatible_kind(right)?;
        let role = match (operation, left.role(), right.role()) {
            (BinaryOp::Add | BinaryOp::Subtract, Role::Point, Role::Difference)
            | (BinaryOp::Add, Role::Difference, Role::Point) => Role::Point,
            (BinaryOp::Subtract, Role::Point, Role::Point)
            | (BinaryOp::Add | BinaryOp::Subtract, Role::Difference, Role::Difference) => {
                Role::Difference
            }
            _ => return Err(UnitError::AffineRole),
        };
        if let Some(domain) = left.kind().count_domain() {
            let admitted = if operation == BinaryOp::Add {
                domain.admits_addition()
            } else {
                domain.admits_subtraction()
            };
            if !admitted {
                return Err(UnitError::CountDomain);
            }
        }
        if role == left.role() {
            Ok(left.coherent())
        } else {
            Descriptor::new(left.kind().clone(), Measure::coherent(), role)
        }
    }

    /// Select a declared rule or one of the identity-independent standard relationships.
    fn multiplicative_kind(
        &self,
        operation: BinaryOp,
        left: &Kind,
        right: &Kind,
    ) -> Result<Kind, UnitError> {
        if let Some(result) = self.binary.get(&(operation, left.clone(), right.clone())) {
            return Ok(result.clone());
        }
        if left.count_domain().is_some() || right.count_domain().is_some() {
            return Err(UnitError::CountDomain);
        }
        if right == &Kind::Ratio || (operation == BinaryOp::Multiply && left == &Kind::Ratio) {
            return Ok(if left == &Kind::Ratio {
                right.clone()
            } else {
                left.clone()
            });
        }
        if operation == BinaryOp::Divide && left == right && !matches!(left, Kind::Uncertainty(..))
        {
            return Ok(Kind::Ratio);
        }
        Err(UnitError::UnsupportedKindRule)
    }

    /// Explicitly divide an absolute uncertainty by a compatible nonzero reference.
    ///
    /// The expression client additionally establishes value and reference identities.
    ///
    /// TODO ´todo:code:validate-indexed-nonzero-references-and-preserve´: validate indexed nonzero references and preserve paired relative covariance interpretation under rule:nrz:correlation-and-relative-shapes.
    ///
    /// # Errors
    /// Refuses non-absolute uncertainties, incompatible references and missing nonzero evidence.
    pub fn relative(
        &self,
        absolute: &Descriptor,
        reference: &Descriptor,
        nonzero: bool,
    ) -> Result<Descriptor, UnitError> {
        self.validate(absolute)?;
        self.validate(reference)?;
        let Kind::Uncertainty(mode, value, None) = absolute.kind() else {
            return Err(UnitError::UncertaintyRelationship);
        };
        let mode = match mode {
            UncertaintyMode::Standard => UncertaintyMode::RelativeStandard,
            UncertaintyMode::Radius => UncertaintyMode::RelativeRadius,
            _ => return Err(UnitError::UncertaintyRelationship),
        };
        let value = Descriptor::new(
            value.as_ref().clone(),
            Measure::coherent(),
            Role::Difference,
        )?;
        uncertainty(mode, &value, None, Some((reference, nonzero))).map(|value| value.coherent())
    }

    /// Check an exact power under the selected exponent policy and semantic rules.
    ///
    /// TODO ´todo:code:require-factor-and-eigenvalue-kind-contracts´: require factor and eigenvalue kind contracts beyond scalar dimensional powers under rule:nrz:shape-factorizations and rule:nrz:shape-eigenproblems.
    ///
    /// # Errors
    /// Refuses points, counts, unsupported kind rules and unadmitted dimensions.
    pub fn power(&self, base: &Descriptor, exponent: Rational) -> Result<Descriptor, UnitError> {
        self.validate(base)?;
        if base.role() != Role::Difference {
            return Err(UnitError::AffineRole);
        }
        if base.kind().count_domain().is_some() {
            return Err(UnitError::CountDomain);
        }
        base.dimension().power(exponent, self.policy)?;
        if exponent == Rational::ONE || base.kind() == &Kind::Ratio {
            return Ok(base.coherent());
        }
        let kind = if let Some(result) = self.powers.get(&(base.kind().clone(), exponent)) {
            result.clone()
        } else {
            match base.kind() {
                Kind::Uncertainty(UncertaintyMode::Standard, value, None)
                    if exponent == Rational::integer(2) =>
                {
                    Kind::Uncertainty(UncertaintyMode::Variance, value.clone(), None)
                }
                Kind::Uncertainty(UncertaintyMode::Variance, value, None)
                    if exponent == Rational::new(1, 2)? =>
                {
                    Kind::Uncertainty(UncertaintyMode::Standard, value.clone(), None)
                }
                _ => return Err(UnitError::UnsupportedKindRule),
            }
        };
        Descriptor::new(kind, Measure::coherent(), Role::Difference)
    }

    /// Negate a linear continuous value or a signed count domain.
    ///
    /// # Errors
    /// Refuses affine points and unproved count closure.
    pub fn negate(&self, value: &Descriptor) -> Result<Descriptor, UnitError> {
        self.validate(value)?;
        if value.role() != Role::Difference {
            return Err(UnitError::AffineRole);
        }
        if let Some(domain) = value.kind().count_domain() {
            domain.admits_scale(Rational::integer(-1))?;
        }
        Ok(value.coherent())
    }

    /// Multiply a count by a known whole-number coefficient with proven domain closure.
    ///
    /// # Errors
    /// Refuses a non-count, fractional scaling or an unproved range condition.
    pub fn scale_count(
        &self,
        count: &Descriptor,
        scale: Rational,
    ) -> Result<Descriptor, UnitError> {
        self.validate(count)?;
        count
            .kind()
            .count_domain()
            .ok_or(UnitError::KindMismatch)?
            .admits_scale(scale)?;
        Ok(count.coherent())
    }

    /// Form an explicit ratio of compatible counts; divisor definedness stays in the tree.
    ///
    /// # Errors
    /// Refuses non-counts or different count meanings or domains.
    pub fn count_ratio(
        &self,
        left: &Descriptor,
        right: &Descriptor,
    ) -> Result<Descriptor, UnitError> {
        self.validate(left)?;
        self.validate(right)?;
        left.compatible_kind(right)?;
        if left.kind().count_domain().is_none() {
            return Err(UnitError::KindMismatch);
        }
        Ok(Descriptor::ratio())
    }
}
