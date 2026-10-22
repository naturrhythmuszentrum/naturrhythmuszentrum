// SPDX-License-Identifier: AGPL-3.0-only
//! Defining equations of existing joint selections and explicit scalar coefficient maps.

use super::evaluate::Evaluation;
use super::proof::{AT, invalid};
use super::value::Value;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{ExpressionAttachment, Handle, LogicalId};
use crate::model::Operator;
use crate::solving::coefficients::forms::{Form, Variable};
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{polynomial, reserve, vector};
use crate::solving::{
    CoefficientMapData, CoefficientValueData, CoefficientValueObject, MapTarget, RootRelationData,
    RootSelectionData, SolveDraft,
};

/// Borrow an ordered joint selection.
pub(crate) fn selection<'a>(
    cas: &'a crate::graph::Cas,
    handle: &Handle,
) -> Result<&'a RootSelectionData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::RootSelection(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Borrow a positive-degree root relation without inferring existence.
pub(crate) fn relation<'a>(
    cas: &'a crate::graph::Cas,
    handle: &Handle,
) -> Result<&'a RootRelationData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::RootRelation(value) => Ok(value),
        _ => Err(invalid()),
    }
}

impl Evaluation<'_> {
    /// Match only the actual coefficient declaration frame; ground and unrelated declarations keep formal slots.
    fn parameter_frame(
        &self,
    ) -> Result<Option<&crate::solving::CoordinateSpaceData<Handle>>, Fault> {
        let crate::solving::CoefficientDomainData::Fraction { coordinates, .. } =
            crate::solving::coefficients::storage::domain(&self.cas.state, self.engine.domain)?
        else {
            return Ok(None);
        };
        let SolveDraft::CoordinateSpace(frame) =
            crate::solving::graph::object(&self.cas.state, coordinates.logical_id())?
        else {
            return Err(invalid());
        };
        if frame.header.logical_id() != self.cas.state.region(self.region.logical_id())?.header {
            return Ok(None);
        }
        Ok(Some(frame))
    }

    /// An unreplaced coefficient declaration is its indeterminate in the actual parameter algebra.
    pub(crate) fn parameter(
        &self,
        id: LogicalId,
        budget: &mut Budget,
    ) -> Result<Option<Value>, Fault> {
        let Some(frame) = self.parameter_frame()? else {
            return Ok(None);
        };
        let expression = self.cas.state.expression(id)?;
        let (Operator::Free(key), None) = (&expression.operator, &expression.attachment) else {
            return Ok(None);
        };
        budget.work(crate::graph::size(frame.coordinates.len(), AT)?.max(1), AT)?;
        let real = expression.sort
            == crate::model::Sort::Value(crate::model::AtomicSort::Scalar(
                crate::model::Field::Real,
            ));
        let real_frame = if real && expression.header == frame.header.logical_id() {
            budget.work(crate::graph::size(frame.coordinates.len(), AT)?.max(1), AT)?;
            frame.coordinates.iter().all(|coordinate| {
                coordinate.field == crate::model::Field::Real
                    && coordinate.role == crate::solving::CoordinateRole::Coefficient
            })
        } else {
            false
        };
        if !frame.coordinates.iter().any(|coordinate| {
            coordinate.key == *key
                && (coordinate.field == crate::model::Field::Complex || real_frame)
                && expression.sort
                    == crate::model::Sort::Value(crate::model::AtomicSort::Scalar(
                        coordinate.field.clone(),
                    ))
                && coordinate.role == crate::solving::CoordinateRole::Coefficient
        }) {
            return Ok(None);
        }
        let Some(numerator) = crate::solving::coefficients::guard_polynomial::read(
            &self.cas.state,
            self.engine.domain,
            self.region.logical_id(),
            id,
            budget,
        )?
        else {
            return Ok(None);
        };
        let denominator = numerator.one(budget)?;
        let coefficient =
            self.engine
                .element(crate::solving::coefficients::model::Value::Fraction(
                    crate::solving::coefficients::fraction::Fraction {
                        numerator,
                        denominator,
                    },
                ));
        Ok(Some(Value::polynomial(
            Form::constant(coefficient, &self.engine, budget).map_err(|error| fault(&error))?,
            &self.engine,
            budget,
        )?))
    }

    /// Coefficients may vanish only after their denominator is proved nonzero on this exact cell.
    pub(crate) fn parameter_zero(
        &self,
        coefficient: &crate::solving::coefficients::model::Element,
        budget: &mut Budget,
    ) -> Result<bool, Fault> {
        let Some(frame) = self.parameter_frame()? else {
            return Ok(false);
        };
        budget.work(crate::graph::size(frame.coordinates.len(), AT)?.max(1), AT)?;
        if !frame.coordinates.iter().all(|coordinate| {
            matches!(
                coordinate.field,
                crate::model::Field::Real | crate::model::Field::Complex
            ) && frame
                .coordinates
                .first()
                .is_some_and(|first| first.field == coordinate.field)
                && coordinate.role == crate::solving::CoordinateRole::Coefficient
        }) {
            return Ok(false);
        }
        let crate::solving::coefficients::model::Value::Fraction(value) = &coefficient.value else {
            return Ok(false);
        };
        if !crate::solving::coefficients::nonzero::proves(
            &self.cas.state,
            self.engine.domain,
            &value.denominator,
            self.region,
            self.premises
                .iter()
                .map(|premise| &premise.judgment.statement),
            budget,
        )? {
            return Ok(false);
        }
        if self
            .engine
            .zero_on(coefficient, self.region, budget)
            .map_err(|error| fault(&error))?
        {
            return Ok(true);
        }
        let mut relations = crate::solving::coefficients::regional::zeros(
            &self.cas.state,
            self.engine.domain,
            &value.numerator,
            self.region,
            budget,
        )?;
        if frame
            .coordinates
            .first()
            .is_some_and(|coordinate| coordinate.field == crate::model::Field::Real)
        {
            crate::solving::real::zero::append(
                self.cas,
                self.engine.domain,
                self.region,
                self.premises,
                &mut relations,
                budget,
            )?;
        }
        let mut remainder = value.numerator.copy(budget)?;
        if frame
            .coordinates
            .first()
            .is_some_and(|coordinate| coordinate.field == crate::model::Field::Real)
        {
            return crate::solving::real::zero::reduce(remainder, &relations, budget);
        }
        for relation in &relations {
            let (_, reduced) =
                crate::solving::coefficients::gcd::division(&remainder, relation, budget)?;
            remainder = reduced;
        }
        remainder.zero(budget)
    }

    /// Ground scalars enter the actual algebra through its canonical parent chain.
    fn ground(
        &self,
        stored: &CoefficientValueObject<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let coefficient = match &stored.value {
            CoefficientValueData::Rational { value } => self
                .engine
                .rational(value, budget)
                .map_err(|error| fault(&error))?,
            CoefficientValueData::Gaussian { real, imaginary } if imaginary.is_zero() => self
                .engine
                .rational(real, budget)
                .map_err(|error| fault(&error))?,
            CoefficientValueData::Gaussian { real, imaginary } => {
                let scalar =
                    crate::solving::numeric::Gaussian::from_parts(real, imaginary, budget)?;
                let source = crate::solving::coefficients::arithmetic::Engine {
                    state: &self.cas.state,
                    domain: &stored.domain,
                };
                let value = source.element(crate::solving::coefficients::model::Value::Scalar(
                    crate::solving::coefficients::scalar::Scalar::Gaussian(scalar),
                ));
                crate::solving::coefficients::maps::canonical(
                    &self.cas.state,
                    &value,
                    self.engine.domain,
                    budget,
                )
                .map_err(|error| fault(&error))?
            }
            _ => return Err(invalid()),
        };
        Value::polynomial(
            Form::constant(coefficient, &self.engine, budget).map_err(|error| fault(&error))?,
            &self.engine,
            budget,
        )
    }

    /// A checked free parameter with no replacement is total and carries no nested map provenance.
    fn unchanged_parameter(&self, image: &Handle, budget: &mut Budget) -> Result<bool, Fault> {
        budget.work(1, AT)?;
        let expression = self.cas.state.expression(image.logical_id())?;
        let (Operator::Free(key), None) = (&expression.operator, &expression.attachment) else {
            return Ok(false);
        };
        for (name, _) in self.free {
            budget.work(1, AT)?;
            if *name == key.as_str() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Evaluate a canonical rational function through its explicit ordered scalar images.
    fn mapped_polynomial(
        &self,
        handle: &Handle,
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let stored = polynomial(&self.cas.state, handle)?;
        if stored.variables.len() != map.images.len() {
            return Err(invalid());
        }
        let mut result = None;
        for term in &stored.terms {
            let coefficient =
                crate::solving::coefficients::storage::value(&self.cas.state, &term.coefficient)?;
            let mut product = self.ground(coefficient, budget)?;
            for (exponent, image) in term.exponents.iter().zip(&map.images) {
                // The full map was checked first; an unreplaced free image is already known total.
                if *exponent == 0 && self.unchanged_parameter(image, budget)? {
                    continue;
                }
                let value = self.expression(image.logical_id(), true, budget)?;
                // Strict image checks precede the zero power; provenance is never discarded.
                if *exponent == 0 && value.maps.is_empty() {
                    continue;
                }
                let factor = value.pow(*exponent, &self.engine, budget)?;
                product = product.mul(&factor, &self.engine, budget)?;
            }
            result = Some(match result {
                None => product,
                Some(previous) => Value::add(&previous, &product, &self.engine, budget)?,
            });
        }
        result.map_or_else(|| Value::integer(0, &self.engine, budget), Ok)
    }

    /// Reuse the admitted fraction only in its identical algebra after all canonical images and its denominator are checked.
    fn parameter_fraction(
        &self,
        handle: &Handle,
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Option<Value>, Fault> {
        if map.source != *self.engine.domain {
            return Ok(None);
        }
        let Some(frame) = self.parameter_frame()? else {
            return Ok(None);
        };
        budget.work(crate::graph::size(frame.coordinates.len(), AT)?.max(1), AT)?;
        if !frame.coordinates.iter().all(|coordinate| {
            matches!(
                coordinate.field,
                crate::model::Field::Real | crate::model::Field::Complex
            ) && frame
                .coordinates
                .first()
                .is_some_and(|first| first.field == coordinate.field)
                && coordinate.role == crate::solving::CoordinateRole::Coefficient
        }) {
            return Ok(None);
        }
        for image in &map.images {
            if !self.unchanged_parameter(image, budget)? {
                return Ok(None);
            }
        }
        let coefficient = self
            .engine
            .read(handle, budget)
            .map_err(|error| fault(&error))?;
        let numerator =
            Form::constant(coefficient, &self.engine, budget).map_err(|error| fault(&error))?;
        Ok(Some(Value::polynomial(numerator, &self.engine, budget)?))
    }

    /// Interpret the selected scalar map, preserving every rational-function denominator.
    pub(crate) fn mapped(
        &self,
        handle: &Handle,
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let stored = crate::solving::coefficients::storage::value(&self.cas.state, handle)?;
        if stored.domain != map.source {
            return Err(invalid());
        }
        if map.region != *self.region {
            return Err(invalid());
        }
        crate::solving::coefficients::proof::identity_map(self.cas, map, self.premises, budget)?;
        match &stored.value {
            CoefficientValueData::Rational { .. } | CoefficientValueData::Gaussian { .. } => {
                self.ground(stored, budget)
            }
            CoefficientValueData::Fraction {
                numerator,
                denominator,
            } => {
                let scalar = matches!(&map.target, MapTarget::ScalarAmbient { .. });
                if scalar {
                    // The full map and original denominator are checked before any canonical reuse.
                    crate::solving::coefficients::proof::denominator(
                        self.cas,
                        denominator,
                        &stored.domain,
                        self.region,
                        self.premises,
                        budget,
                    )?;
                    if let Some(value) = self.parameter_fraction(handle, map, budget)? {
                        return Ok(value);
                    }
                }
                let dividend = self.mapped_polynomial(numerator, map, budget)?;
                let divisor = self.mapped_polynomial(denominator, map, budget)?;
                if !scalar {
                    self.nonzero(&divisor, budget)?;
                }
                dividend.div(&divisor, &self.engine, budget)
            }
            CoefficientValueData::Tower { coefficients } => self.tower(coefficients, map, budget),
        }
    }

    /// A tower evaluation follows the exact parent map of its selected generator.
    fn tower(
        &self,
        coefficients: &[Handle],
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let [image] = map.images.as_slice() else {
            return Err(invalid());
        };
        let expression = self.cas.state.expression(image.logical_id())?;
        let (Operator::RootValue { slot }, Some(ExpressionAttachment::SelectedRoot(selected))) =
            (&expression.operator, &expression.attachment)
        else {
            return Err(invalid());
        };
        let selection = selection(self.cas, &self.cas.handle(*selected))?;
        let relation = relation(
            self.cas,
            selection
                .relations
                .get(usize::try_from(*slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)?,
        )?;
        let SolveDraft::CoefficientMap(parent) =
            crate::solving::graph::object(&self.cas.state, relation.map.logical_id())?
        else {
            return Err(invalid());
        };
        let generator = self.expression(image.logical_id(), true, budget)?;
        let parameter = matches!(
            crate::solving::coefficients::storage::domain(&self.cas.state, &parent.source)?,
            crate::solving::CoefficientDomainData::Fraction { .. }
        );
        let unmarked = generator.maps.is_empty();
        let mut entries = coefficients.iter().rev();
        let mut result = if parameter && unmarked {
            match entries.next() {
                Some(coefficient) => self.mapped(coefficient, parent, budget)?,
                None => Value::integer(0, &self.engine, budget)?,
            }
        } else {
            Value::integer(0, &self.engine, budget)?
        };
        for coefficient in entries {
            result = result.mul(&generator, &self.engine, budget)?.add(
                &self.mapped(coefficient, parent, budget)?,
                &self.engine,
                budget,
            )?;
        }
        Ok(result)
    }

    /// Substitute a scalar into every term of the exact specialized root equation.
    pub(crate) fn root_equation(
        &self,
        root: &Value,
        relation: &RootRelationData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        if relation.region != *self.region {
            return Err(invalid());
        }
        let stored = polynomial(&self.cas.state, &relation.polynomial)?;
        let SolveDraft::CoefficientMap(map) =
            crate::solving::graph::object(&self.cas.state, relation.map.logical_id())?
        else {
            return Err(invalid());
        };
        if stored.variables.len() != 1 {
            return Err(invalid());
        }
        if matches!(
            crate::solving::coefficients::storage::domain(&self.cas.state, &map.source)?,
            crate::solving::CoefficientDomainData::Fraction { .. }
        ) {
            return self.parameter_equation(root, stored, map, budget);
        }
        let mut result = Value::integer(0, &self.engine, budget)?;
        for term in &stored.terms {
            let coefficient = self.mapped(&term.coefficient, map, budget)?;
            let power = root.pow(
                *term.exponents.first().ok_or_else(invalid)?,
                &self.engine,
                budget,
            )?;
            result = result.add(
                &coefficient.mul(&power, &self.engine, budget)?,
                &self.engine,
                budget,
            )?;
        }
        Ok(result)
    }

    /// Preserve a checked parameter equation's terms without synthesizing unused additive or multiplicative identities.
    fn parameter_equation(
        &self,
        root: &Value,
        stored: &crate::solving::PolynomialData<Handle>,
        map: &CoefficientMapData<Handle>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let mut result = None;
        for term in &stored.terms {
            let coefficient = self.mapped(&term.coefficient, map, budget)?;
            let exponent = *term.exponents.first().ok_or_else(invalid)?;
            let product = if exponent == 0 && root.maps.is_empty() {
                coefficient
            } else {
                coefficient.mul(
                    &root.pow(exponent, &self.engine, budget)?,
                    &self.engine,
                    budget,
                )?
            };
            result = Some(match result {
                None => product,
                Some(previous) => Value::add(&previous, &product, &self.engine, budget)?,
            });
        }
        result.map_or_else(|| Value::integer(0, &self.engine, budget), Ok)
    }

    /// Root atoms in an identity contribute only equations of their exact existing selection slots.
    pub(crate) fn root_relations(
        &self,
        form: &Form,
        budget: &mut Budget,
    ) -> Result<Vec<Form>, Fault> {
        let mut seen = vector(0, budget)?;
        let mut result = vector(0, budget)?;
        for term in &form.terms {
            for (variable, _) in &term.powers {
                let Variable::Atom(id) = variable else {
                    continue;
                };
                budget.work(crate::graph::size(seen.len(), AT)?.max(1), AT)?;
                if seen.contains(id) {
                    continue;
                }
                reserve(&mut seen, 1, budget)?;
                seen.push(*id);
                let value = self.cas.state.expression(*id)?;
                let (
                    Operator::RootValue { slot },
                    Some(ExpressionAttachment::SelectedRoot(selected)),
                ) = (&value.operator, &value.attachment)
                else {
                    continue;
                };
                let selected = selection(self.cas, &self.cas.handle(*selected))?;
                budget.solving(
                    BudgetKind::SelectionSlots,
                    crate::graph::size(selected.relations.len(), AT)?,
                    AT,
                )?;
                let handle = selected
                    .relations
                    .get(usize::try_from(*slot).map_err(|_| invalid())?)
                    .ok_or_else(invalid)?;
                let root = self.expression(*id, true, budget)?;
                let equation = self.root_equation(&root, relation(self.cas, handle)?, budget)?;
                reserve(&mut result, 1, budget)?;
                result.push(equation.numerator);
            }
        }
        Ok(result)
    }
}

/// Locate a slot occurrence by following only the supplied expression closure.
pub(crate) fn occurrence(
    cas: &crate::graph::Cas,
    roots: &[Handle],
    selected: &Handle,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut pending = vector(roots.len(), budget)?;
    pending.extend(roots.iter().map(Handle::logical_id));
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let expression = cas.state.expression(id)?;
        if matches!((&expression.operator, &expression.attachment), (Operator::RootValue { slot: 0 }, Some(ExpressionAttachment::SelectedRoot(handle))) if *handle == selected.logical_id())
        {
            return Ok(id);
        }
        reserve(&mut pending, expression.operands.len(), budget)?;
        pending.extend(&expression.operands);
        if let Some(ExpressionAttachment::ExactCoefficient { map, .. }) = &expression.attachment
            && let SolveDraft::CoefficientMap(mapping) =
                crate::solving::graph::object(&cas.state, *map)?
        {
            reserve(&mut pending, mapping.images.len(), budget)?;
            pending.extend(mapping.images.iter().map(Handle::logical_id));
        }
    }
    Err(invalid())
}

#[cfg(test)]
mod tests {
    use super::{Evaluation, Value};
    use crate::solving::{
        CoefficientMapData, PolynomialData, PolynomialTerm, SolveDraft,
        coefficients::arithmetic::Engine,
        radicals::construction::Construction,
        towers::{dense::Dense, materialize},
    };
    use crate::testing::common::TestResult;
    use crate::{Cas, Handle, Rational, budget::Budget, limits::Limits};

    /// Independently checked selected root, literal units, and noncanonical images of both map levels.
    struct Fixture {
        cas: Cas,
        region: Handle,
        base: Handle,
        parent: Handle,
        unit: Handle,
        tower: Handle,
        tower_unit: Handle,
        root: Handle,
        marked: [Handle; 2],
        compound: [Handle; 2],
        existence: crate::proof_model::CheckedApplication,
    }

    /// A nonreal quadratic avoids a real-root presentation and gives one actual `RootValue` image.
    fn fixture() -> TestResult<Fixture> {
        let (mut source, _) = crate::testing::solve_blind::symbolic(&["p"])?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let mut construction = Construction {
            header: source.header.clone(),
            context: source.context.clone(),
            region: source.region.clone(),
            steps: Vec::new(),
        };
        let images = crate::solving::solve_search::coefficient_images(
            &mut source.cas,
            &construction,
            &source.algebra,
            &mut budget,
        )?;
        let parent =
            construction.mapping(&mut source.cas, &source.algebra, &images, &mut budget)?;
        let engine = Engine {
            state: &source.cas.state,
            domain: &source.algebra,
        };
        let (_, _, base) = engine.fraction_shape()?;
        let base = base.clone();
        let one = engine.constant(true, &mut budget)?;
        let mut coefficients = Vec::new();
        for value in [2, 0, 1] {
            coefficients
                .push(engine.rational(&Rational::from_ratio(value, 1, &mut budget)?, &mut budget)?);
        }
        let polynomial = Dense::new(&engine, coefficients, &mut budget)?;
        let unit = materialize::value(&mut source.cas, &one, &mut budget)?;
        let marked_parent = construction.literal(&mut source.cas, &one, &parent, &mut budget)?;
        let extension =
            construction.adjoin(&mut source.cas, &polynomial, &parent, None, &mut budget)?;
        let witness = source.cas.check_proof(
            &source.context,
            &construction.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let existence =
            crate::proof_view::application(&source.cas, witness.proof.logical_id(), &mut budget)?;
        let engine = Engine {
            state: &source.cas.state,
            domain: &extension.domain,
        };
        let one = engine.constant(true, &mut budget)?;
        let tower_unit = materialize::value(&mut source.cas, &one, &mut budget)?;
        let marked_tower = construction.literal(
            &mut source.cas,
            &extension.generator,
            &extension.map,
            &mut budget,
        )?;
        let zero = source.integer(0, false)?;
        let compound_parent = source.expression(
            crate::model::Operator::Add,
            &[images[0].clone(), zero.clone()],
            false,
        )?;
        let compound_tower = source.expression(
            crate::model::Operator::Add,
            &[extension.root.clone(), zero],
            false,
        )?;
        Ok(Fixture {
            cas: source.cas,
            region: source.region,
            base,
            parent,
            unit,
            tower: extension.map,
            tower_unit,
            root: extension.root,
            marked: [marked_parent, marked_tower],
            compound: [compound_parent, compound_tower],
            existence,
        })
    }

    impl Fixture {
        fn evaluation(&self) -> Evaluation<'_> {
            Evaluation {
                cas: &self.cas,
                engine: Engine {
                    state: &self.cas.state,
                    domain: &self.base,
                },
                region: &self.region,
                premises: std::slice::from_ref(&self.existence),
                free: &[],
                bound: &[],
            }
        }
        fn mapping(&self, handle: &Handle) -> TestResult<&CoefficientMapData<Handle>> {
            let SolveDraft::CoefficientMap(map) =
                crate::solving::graph::object(&self.cas.state, handle.logical_id())?
            else {
                return Err("canonical map".into());
            };
            Ok(map)
        }
        fn constant(&self) -> TestResult<PolynomialData<Handle>> {
            Ok(PolynomialData {
                domain: self.mapping(&self.parent)?.source.clone(),
                variables: vec![crate::model::Field::Complex],
                terms: vec![PolynomialTerm {
                    exponents: vec![0],
                    coefficient: self.unit.clone(),
                }],
            })
        }
    }

    /// Neither zero-power polynomial evaluation nor root-equation evaluation accepts a noncanonical parameter image.
    /// ´test:unit:canonical-parameter-image-controls´
    #[test]
    fn canonical_parameter_image_controls() -> TestResult {
        let fixture = fixture()?;
        let evaluation = fixture.evaluation();
        let map = fixture.mapping(&fixture.parent)?;
        let stored = fixture.constant()?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let root = Value::integer(2, &evaluation.engine, &mut budget)?;
        assert_eq!(
            evaluation.mapped(&fixture.unit, map, &mut budget)?.maps,
            [] as [crate::graph::LogicalId; 0]
        );
        assert_eq!(
            evaluation
                .parameter_equation(&root, &stored, map, &mut budget)?
                .maps,
            [] as [crate::graph::LogicalId; 0]
        );
        let marked = evaluation.expression(fixture.marked[0].logical_id(), true, &mut budget)?;
        assert_eq!(marked.maps, [fixture.parent.logical_id()]);
        for image in [&fixture.marked[0], &fixture.compound[0]] {
            let mut changed = map.clone();
            assert_eq!(changed.images.len(), 1);
            assert_eq!(
                fixture.cas.expression(&changed.images[0])?.0,
                &crate::model::Operator::Free("p".into())
            );
            assert_ne!(*image, changed.images[0]);
            changed.images[0] = image.clone();
            assert!(
                evaluation
                    .mapped(&fixture.unit, &changed, &mut budget)
                    .is_err()
            );
            assert!(
                evaluation
                    .parameter_equation(&root, &stored, &changed, &mut budget)
                    .is_err()
            );
        }
        let free = [("p", marked)];
        let substituted = Evaluation {
            free: &free,
            ..fixture.evaluation()
        };
        assert_eq!(
            substituted.mapped(&fixture.unit, map, &mut budget)?.maps,
            [fixture.parent.logical_id()]
        );
        Ok(())
    }

    /// The equation's constant term keeps zero-power provenance, and a genuine selected root still requires existence.
    /// ´test:unit:canonical-root-equation-controls´
    #[test]
    fn canonical_root_equation_controls() -> TestResult {
        let fixture = fixture()?;
        let evaluation = fixture.evaluation();
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let root = evaluation.expression(fixture.root.logical_id(), true, &mut budget)?;
        assert_eq!(root.maps, [] as [crate::graph::LogicalId; 0]);
        let marked = evaluation.expression(fixture.marked[1].logical_id(), true, &mut budget)?;
        assert_eq!(marked.maps, [fixture.tower.logical_id()]);
        let stored = fixture.constant()?;
        let map = fixture.mapping(&fixture.parent)?;
        assert_eq!(
            evaluation
                .parameter_equation(&root, &stored, map, &mut budget)?
                .maps,
            [] as [crate::graph::LogicalId; 0]
        );
        assert_eq!(
            evaluation
                .parameter_equation(&marked, &stored, map, &mut budget)?
                .maps,
            marked.maps
        );
        let absent = Evaluation {
            premises: &[],
            ..fixture.evaluation()
        };
        assert!(
            absent
                .expression(fixture.root.logical_id(), true, &mut budget)
                .is_err()
        );
        Ok(())
    }

    /// Constant Horner values still reject compound and provenance-bearing generator images and missing existence.
    /// ´test:unit:canonical-tower-image-controls´
    #[test]
    fn canonical_tower_image_controls() -> TestResult {
        let fixture = fixture()?;
        let evaluation = fixture.evaluation();
        let map = fixture.mapping(&fixture.tower)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        assert_eq!(map.images, std::slice::from_ref(&fixture.root));
        assert_eq!(
            evaluation
                .mapped(&fixture.tower_unit, map, &mut budget)?
                .maps,
            [] as [crate::graph::LogicalId; 0]
        );
        let marked = evaluation.expression(fixture.marked[1].logical_id(), true, &mut budget)?;
        assert_eq!(marked.maps, [fixture.tower.logical_id()]);
        for image in [&fixture.marked[1], &fixture.compound[1]] {
            let mut changed = map.clone();
            assert_ne!(*image, changed.images[0]);
            changed.images[0] = image.clone();
            assert!(
                evaluation
                    .mapped(&fixture.tower_unit, &changed, &mut budget)
                    .is_err()
            );
        }
        let absent = Evaluation {
            premises: &[],
            ..fixture.evaluation()
        };
        assert!(
            absent
                .mapped(&fixture.tower_unit, map, &mut budget)
                .is_err()
        );
        Ok(())
    }
    /// Ordered coefficient declarations use their actual algebra; the unknown and replacements keep their roles.
    /// ´test:unit:parameter-declaration-replay´
    #[test]
    fn parameter_declaration_replay() -> TestResult {
        let (mut source, _) = crate::testing::solve_blind::symbolic(&["q", "p"])?;
        let p = source.free("p")?;
        let q = source.free("q")?;
        let unknown = source.free("x")?;
        let square = source.expression(
            crate::model::Operator::IntegerPower(2),
            std::slice::from_ref(&p),
            false,
        )?;
        let construction = Construction {
            header: source.header.clone(),
            context: source.context.clone(),
            region: source.region.clone(),
            steps: Vec::new(),
        };
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let images = crate::solving::solve_search::coefficient_images(
            &mut source.cas,
            &construction,
            &source.algebra,
            &mut budget,
        )?;
        let map = construction.mapping(&mut source.cas, &source.algebra, &images, &mut budget)?;
        let evaluation = Evaluation {
            cas: &source.cas,
            engine: Engine {
                state: &source.cas.state,
                domain: &source.algebra,
            },
            region: &source.region,
            premises: &[],
            free: &[],
            bound: &[],
        };
        for (handle, powers) in [(&q, [1, 0]), (&p, [0, 1])] {
            let value = evaluation.expression(handle.logical_id(), true, &mut budget)?;
            let [term] = value.numerator.terms.as_slice() else {
                return Err("coefficient term".into());
            };
            assert_eq!(
                term.powers,
                [] as [(crate::solving::coefficients::forms::Variable, u32); 0]
            );
            let crate::solving::coefficients::model::Value::Fraction(coefficient) =
                &term.coefficient.value
            else {
                return Err("parameter fraction".into());
            };
            assert_eq!(coefficient.numerator.terms.len(), 1);
            assert_eq!(coefficient.numerator.terms[0].powers, powers);
            assert_eq!(coefficient.denominator.terms.len(), 1);
            assert_eq!(coefficient.denominator.terms[0].powers, [0, 0]);
        }
        let value = evaluation.expression(unknown.logical_id(), true, &mut budget)?;
        assert_eq!(value.numerator.terms.len(), 1);
        assert_eq!(value.numerator.terms[0].powers.len(), 1);
        let mut replacement = Value::integer(7, &evaluation.engine, &mut budget)?;
        replacement.maps.push(map.logical_id());
        let free = [("p", replacement)];
        let substituted = Evaluation {
            free: &free,
            ..evaluation.unbound()
        };
        let actual = substituted.expression(p.logical_id(), true, &mut budget)?;
        assert_eq!(actual.maps, [map.logical_id()]);
        assert!(actual.equals(
            &Value::integer(7, &evaluation.engine, &mut budget)?,
            &evaluation.engine,
            &mut budget
        )?);
        let mut bounded = Budget::new(Limits {
            polynomial_degree: 1,
            ..Limits::blind_cubic()
        })?;
        assert!(
            evaluation
                .expression(square.logical_id(), true, &mut bounded)
                .is_err()
        );
        Ok(())
    }

    /// Cell-zero coefficients disappear only on that cell, while division keeps its original nonzero check.
    /// ´test:unit:parameter-cell-zero-replay´
    #[test]
    fn parameter_cell_zero_replay() -> TestResult {
        for zero in [true, false] {
            let (mut source, _) = crate::testing::solve_blind::regional_unit(zero)?;
            let parameter = source.free("a")?;
            let quotient = source.expression(
                crate::model::Operator::Div,
                &[parameter.clone(), parameter.clone()],
                false,
            )?;
            let square = source.expression(
                crate::model::Operator::Mul,
                &[parameter.clone(), parameter.clone()],
                false,
            )?;
            let cancelled = source.expression(
                crate::model::Operator::Div,
                &[square, parameter.clone()],
                false,
            )?;
            let evaluation = Evaluation {
                cas: &source.cas,
                engine: Engine {
                    state: &source.cas.state,
                    domain: &source.algebra,
                },
                region: &source.region,
                premises: &[],
                free: &[],
                bound: &[],
            };
            let mut budget = Budget::new(Limits::blind_cubic())?;
            let value = evaluation.expression(parameter.logical_id(), true, &mut budget)?;
            assert_eq!(evaluation.zero(&value, &mut budget).is_ok(), zero);
            assert!(
                evaluation
                    .zero(
                        &Value::integer(1, &evaluation.engine, &mut budget)?,
                        &mut budget
                    )
                    .is_err()
            );
            assert_eq!(
                evaluation
                    .expression(quotient.logical_id(), true, &mut budget)
                    .is_ok(),
                !zero
            );
            assert_ne!(cancelled, quotient);
            assert_eq!(
                evaluation
                    .expression(cancelled.logical_id(), true, &mut budget)
                    .is_ok(),
                !zero
            );
            let crate::solving::coefficients::model::Value::Fraction(coefficient) =
                &value.numerator.terms[0].coefficient.value
            else {
                return Err("parameter fraction".into());
            };
            let numerator = coefficient
                .numerator
                .mul(&coefficient.numerator, &mut budget)?;
            let denominator = coefficient
                .numerator
                .add(&coefficient.denominator, &mut budget)?;
            let changed =
                evaluation
                    .engine
                    .element(crate::solving::coefficients::model::Value::Fraction(
                        crate::solving::coefficients::fraction::Fraction::new(
                            &numerator,
                            &denominator,
                            &mut budget,
                        )?,
                    ));
            assert!(coefficient.denominator.scalar(&mut budget)?.is_some());
            assert!(!evaluation.engine.formal_equal(
                &changed,
                &value.numerator.terms[0].coefficient,
                &mut budget
            )?);
            assert!(!evaluation.parameter_zero(&changed, &mut budget)?);
        }
        Ok(())
    }

    /// A sum of distinct contained zero-guard multiples requires both cells, not either guard alone.
    /// ´test:unit:parameter-joint-zero-replay´
    #[test]
    fn parameter_joint_zero_replay() -> TestResult {
        use crate::domain_model::{AmbientDomain, RegionDraft, RegionExpr};
        use crate::graph::{Draft, Reference};
        let (mut source, _) = crate::testing::solve_blind::symbolic(&["p", "q"])?;
        let p = source.free("p")?;
        let q = source.free("q")?;
        let square = source.expression(
            crate::model::Operator::IntegerPower(2),
            std::slice::from_ref(&p),
            false,
        )?;
        let cube = source.expression(
            crate::model::Operator::IntegerPower(3),
            std::slice::from_ref(&q),
            false,
        )?;
        let sum = source.expression(crate::model::Operator::Add, &[square, cube], false)?;
        let origin = source.integer(0, false)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        for (parameter, expected) in [(p, false), (q, true)] {
            let predicate = source.equation(&parameter, &origin)?;
            let draft = Draft::Region(RegionDraft {
                header: Reference::Existing(source.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: source.coefficients.clone(),
                },
                expression: RegionExpr::GuardTrue {
                    base: Reference::Existing(source.region.clone()),
                    predicate: Reference::Existing(predicate),
                },
            });
            source.region = source
                .cas
                .admit(&[draft], &mut budget)?
                .into_iter()
                .next()
                .ok_or("zero cell")?;
            let evaluation = Evaluation {
                cas: &source.cas,
                engine: Engine {
                    state: &source.cas.state,
                    domain: &source.algebra,
                },
                region: &source.region,
                premises: &[],
                free: &[],
                bound: &[],
            };
            let value = evaluation.expression(sum.logical_id(), true, &mut budget)?;
            assert_eq!(value.numerator.terms.len(), 1);
            assert!(!evaluation.engine.zero_on(
                &value.numerator.terms[0].coefficient,
                &source.region,
                &mut budget
            )?);
            assert_eq!(evaluation.zero(&value, &mut budget).is_ok(), expected);
            let shifted = value.add(
                &Value::integer(1, &evaluation.engine, &mut budget)?,
                &evaluation.engine,
                &mut budget,
            )?;
            assert!(evaluation.zero(&shifted, &mut budget).is_err());
        }
        Ok(())
    }
}
