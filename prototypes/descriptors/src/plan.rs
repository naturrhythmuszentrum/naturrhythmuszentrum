// SPDX-License-Identifier: AGPL-3.0-only
//! Evidence-owning typestate for one deliberately narrow resistor recipe.

use std::collections::BTreeMap;

use nrz_algebra::{Expr, Limits, Rational, Symbol, parse};
use nrz_units::{CheckedExpression, Context, Descriptor};

use crate::{
    Companion, CoordinateMap, Error, Handle, Heap, Scalar, SchemaPool, Shape, ShapeId, Value,
    error::identity,
};

/// A plan for which no CAS or recipe checks have yet succeeded.
#[derive(Debug)]
pub struct Unchecked;

/// Owned checking evidence, constructible only by checking original expressions.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::Checked;
/// let checked = Checked { witnesses: Vec::new() };
/// ```
///
/// ```
/// use nrz_descriptor_prototype::{Checked, Plan, resistor_fixture};
/// let plan: Plan<Checked> = resistor_fixture::<f64>()?.plan.check()?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
#[derive(Debug)]
pub struct Checked {
    /// Original expressions, declaration snapshots and derived descriptors.
    witnesses: Vec<CheckedExpression>,
}

/// Explicit correspondence between expression names and recipe slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Slot {
    /// Source voltage input.
    Source,
    /// Resistance input.
    Resistance,
    /// First unknown, a voltage difference.
    Voltage,
    /// Second unknown, the current through the resistor.
    Current,
    /// Typed zero voltage constant.
    ZeroVoltage,
    /// Typed zero resistance coefficient.
    ZeroResistance,
}

/// Interned shapes of the concrete toy recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    /// Scalar source value.
    pub source: ShapeId,
    /// Scalar resistance value.
    pub resistance: ShapeId,
    /// Covariance of the single source with itself.
    pub source_covariance: ShapeId,
    /// Two-entry heterogeneous solution vector.
    pub solution: ShapeId,
    /// Full two-by-two solution covariance.
    pub covariance: ShapeId,
}

/// The exact coordinate system is a separate binding contract from the shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Maps {
    /// Physical references and origins for the two unknowns.
    pub solution: [CoordinateMap; 2],
    /// Physical references and origins for the two equations.
    pub equations: [CoordinateMap; 2],
}

/// Explicit uncertainty coverage, with one supported mathematical model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coverage {
    /// One shared random source with a supplied finite second moment.
    SharedSourceFiniteSecondMoment,
    /// A deterministic radius needs a different propagation contract.
    DeterministicRadius,
}

/// Input object identities are independent of schemas and symbolic names.
#[derive(Clone, Copy, Debug)]
pub struct Inputs<P> {
    /// Source voltage value.
    pub source: Handle<Value, P>,
    /// Exact, positive resistance value in the fixture model.
    pub resistance: Handle<Value, P>,
    /// Independently stored covariance of this source with itself.
    pub covariance: Handle<Companion, P>,
    /// Mathematical interpretation of the supplied companion.
    pub coverage: Coverage,
}

/// Original equations plus an immutable environment snapshot and evidence owner.
///
/// An unchecked plan has no binding constructor.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::resistor_fixture;
/// let fixture = resistor_fixture::<f64>()?;
/// let binding = fixture.plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
///
/// ```
/// use nrz_descriptor_prototype::resistor_fixture;
/// let fixture = resistor_fixture::<f64>()?;
/// let plan = fixture.plan.check()?;
/// let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
#[derive(Debug)]
pub struct Plan<S> {
    /// Unique declaration and operation snapshot identity.
    snapshot: u64,
    /// Owned CAS environment, never exposed mutably.
    context: Context,
    /// Original equation sides, before any algebraic collection.
    equations: [(Expr, Expr); 2],
    /// Explicit names to client slots, including typed zero declarations.
    slots: BTreeMap<Symbol, Slot>,
    /// Physical voltage and current output descriptors.
    outputs: [Descriptor; 2],
    /// Expected logical schemas from the fixture's frozen pool.
    layout: Layout,
    /// Ordered solution shape snapshot checked alongside scalar expressions.
    solution_shape: Shape,
    /// Scalar domain coordinates retained from the admitted shape.
    solution_columns: Vec<crate::Axis>,
    /// Ordered covariance shape snapshot checked alongside scalar expressions.
    covariance_shape: Shape,
    /// Exact maps required by this checked recipe.
    maps: Maps,
    /// Either no evidence or the owned successful checks.
    state: S,
}

/// Build the original recipe using typed zero constants, including the zero coefficient.
fn recipe() -> Result<[(Expr, Expr); 2], Error> {
    let limits = Limits::default();
    Ok([
        (parse("v - R * i", limits)?, parse("zero_v", limits)?),
        (parse("v + zero_r * i", limits)?, parse("s", limits)?),
    ])
}

impl Plan<Unchecked> {
    /// Construct only the supported recipe; no unchecked arbitrary kernel is selectable.
    pub(crate) fn resistor(
        context: Context,
        outputs: [Descriptor; 2],
        layout: Layout,
        maps: Maps,
        schemas: &SchemaPool,
    ) -> Result<Self, Error> {
        let slots = [
            ("s", Slot::Source),
            ("R", Slot::Resistance),
            ("v", Slot::Voltage),
            ("i", Slot::Current),
            ("zero_v", Slot::ZeroVoltage),
            ("zero_r", Slot::ZeroResistance),
        ]
        .into_iter()
        .map(|(name, slot)| Ok((Symbol::new(name)?, slot)))
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
        let solution_shape = schemas.shape_definition(layout.solution)?.clone();
        let covariance_shape = schemas.shape_definition(layout.covariance)?.clone();
        if solution_shape.entries.len() != outputs.len() {
            return Err(Error::Shape);
        }
        for (quantity, output) in solution_shape.entries.iter().zip(&outputs) {
            if schemas.descriptor(*quantity)? != output {
                return Err(Error::Shape);
            }
        }
        Ok(Self {
            snapshot: identity()?,
            context,
            equations: recipe()?,
            slots,
            outputs,
            layout,
            solution_columns: solution_shape.columns.clone(),
            solution_shape,
            covariance_shape,
            maps,
            state: Unchecked,
        })
    }

    /// Replace an original equation to investigate checking failures.
    ///
    /// # Errors
    /// Refuses an equation index outside the two-row recipe.
    pub fn with_equation(mut self, node: usize, left: Expr, right: Expr) -> Result<Self, Error> {
        *self.equations.get_mut(node).ok_or(Error::Shape)? = (left, right);
        Ok(self)
    }

    /// Replace the proposed solution shape to investigate ordered-axis checks.
    #[must_use]
    pub fn with_solution_shape(mut self, shape: Shape) -> Self {
        self.solution_shape = shape;
        self
    }

    /// Check original operations before admitting this exact execution recipe.
    ///
    /// # Errors
    /// Preserves CAS diagnostics; refuses unsupported recipes, roles and maps.
    pub fn check(self) -> Result<Plan<Checked>, Error> {
        let mut witnesses = Vec::new();
        for (node, (left, right)) in self.equations.iter().enumerate() {
            let (left, right) =
                self.context
                    .check_equation(left, right)
                    .map_err(|source| Error::Check {
                        node,
                        slot: None,
                        reference: None,
                        binding: None,
                        source: Box::new(source),
                    })?;
            for witness in [&left, &right] {
                witness.descriptor().conversion_to(&self.outputs[0])?;
                if witness
                    .original()
                    .symbols()
                    .iter()
                    .any(|name| !self.slots.contains_key(name))
                {
                    return Err(Error::Unsupported);
                }
            }
            witnesses.extend([left, right]);
        }
        let shape = &self.solution_shape;
        if shape.rows.len() != 2
            || shape.columns.len() != 1
            || shape.entries.len() != 2
            || shape.rows[0].meaning != "node_voltage"
            || shape.rows[1].meaning != "resistor_current"
            || shape
                .rows
                .iter()
                .map(|axis| axis.quantity)
                .collect::<Vec<_>>()
                != shape.entries
            || shape.columns != self.solution_columns
            || self.covariance_shape.rows != shape.rows
            || self.covariance_shape.columns != shape.rows
        {
            return Err(Error::Shape);
        }
        if self.equations != recipe()? {
            return Err(Error::Unsupported);
        }
        for (map, expected) in self.maps.solution.iter().zip(&self.outputs) {
            if map.descriptor() != expected || map.origin() != Rational::ZERO {
                return Err(Error::Map);
            }
        }
        for map in &self.maps.equations {
            if map.descriptor() != &self.outputs[0] || map.origin() != Rational::ZERO {
                return Err(Error::Map);
            }
        }
        Ok(Plan {
            snapshot: self.snapshot,
            context: self.context,
            equations: self.equations,
            slots: self.slots,
            outputs: self.outputs,
            layout: self.layout,
            solution_shape: self.solution_shape,
            solution_columns: self.solution_columns,
            covariance_shape: self.covariance_shape,
            maps: self.maps,
            state: Checked { witnesses },
        })
    }
}

impl<S> Plan<S> {
    /// Immutable explicit symbol-to-slot mapping.
    #[must_use]
    pub const fn slots(&self) -> &BTreeMap<Symbol, Slot> {
        &self.slots
    }

    /// Shape contracts expected by this recipe.
    #[must_use]
    pub const fn layout(&self) -> Layout {
        self.layout
    }
}

impl Plan<Checked> {
    /// Capture candidate input versions; this does not authorize a kernel view.
    ///
    /// # Errors
    /// Refuses references that cannot even be resolved in the proposed heap.
    pub fn bind<P: Scalar>(
        &self,
        heap: &Heap<P>,
        inputs: Inputs<P>,
        maps: Maps,
    ) -> Result<Binding<'_, P>, Error> {
        let versions = [
            heap.contract(inputs.source)?.1,
            heap.contract(inputs.resistance)?.1,
        ];
        Ok(Binding {
            plan: self,
            inputs,
            versions,
            maps,
        })
    }

    /// Recheck each actual input descriptor against the retained symbol declaration.
    fn binding_check<P: Scalar>(
        &self,
        heap: &Heap<P>,
        inputs: Inputs<P>,
        binding: u64,
    ) -> Result<(), Error> {
        for (slot, (name, value)) in [("s", inputs.source), ("R", inputs.resistance)]
            .into_iter()
            .enumerate()
        {
            let shape = heap.schemas().shape_definition(heap.contract(value)?.0)?;
            let quantity = *shape.entries.first().ok_or(Error::Shape)?;
            let descriptor = heap.schemas().descriptor(quantity)?;
            let expression = parse(name, Limits::default())?;
            self.context
                .check_against(&expression, descriptor)
                .map_err(|source| Error::Check {
                    node: 0,
                    slot: Some(slot),
                    reference: Some(value.reference()),
                    binding: Some(binding),
                    source: Box::new(source),
                })?;
        }
        Ok(())
    }

    /// Recheck output expression contracts in the retained immutable CAS snapshot.
    pub(crate) fn publication_check(&self, binding: u64) -> Result<(), Error> {
        for (node, witness) in self.state.witnesses.iter().enumerate() {
            self.context
                .check_against(witness.original(), &self.outputs[0])
                .map_err(|source| Error::Check {
                    node,
                    slot: Some(node / 2),
                    reference: None,
                    binding: Some(binding),
                    source: Box::new(source),
                })?;
        }
        // The current output contract also passes through the CAS in this snapshot.
        for (slot, name) in ["v", "i"].iter().enumerate() {
            let expression = parse(name, Limits::default())?;
            self.context
                .check_against(&expression, &self.outputs[slot])
                .map_err(|source| Error::Check {
                    node: 2,
                    slot: Some(slot),
                    reference: None,
                    binding: Some(binding),
                    source: Box::new(source),
                })?;
        }
        Ok(())
    }
}

/// A proposed binding with no numerical view method.
///
/// ```compile_fail
/// use nrz_descriptor_prototype::resistor_fixture;
/// let fixture = resistor_fixture::<f64>()?;
/// let plan = fixture.plan.check()?;
/// let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
/// let view = binding.normalized()?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
///
/// ```
/// use nrz_descriptor_prototype::resistor_fixture;
/// let mut fixture = resistor_fixture::<f64>()?;
/// let plan = fixture.plan.check()?;
/// let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
/// let bound = binding.validate(&mut fixture.heap)?;
/// let view = bound.normalized()?;
/// # Ok::<(), nrz_descriptor_prototype::Error>(())
/// ```
#[derive(Debug)]
pub struct Binding<'p, P> {
    /// Evidence-owning checked plan.
    plan: &'p Plan<Checked>,
    /// Proposed object identities and uncertainty model.
    inputs: Inputs<P>,
    /// Versions captured before validation.
    versions: [u64; 2],
    /// Proposed maps must match the plan's coordinate contract.
    maps: Maps,
}

impl<'p, P: Scalar> Binding<'p, P> {
    /// Validate schemas, versions, coverage and maps while acquiring exclusive storage.
    ///
    /// # Errors
    /// Refuses changed inputs, stale companions, wrong schemas or unsupported uncertainty.
    pub fn validate(self, heap: &mut Heap<P>) -> Result<crate::Bound<'_, 'p, P>, Error> {
        let source = heap.contract(self.inputs.source)?;
        let resistance = heap.contract(self.inputs.resistance)?;
        if [source.1, resistance.1] != self.versions {
            return Err(Error::Version);
        }
        if source.0 != self.plan.layout.source || resistance.0 != self.plan.layout.resistance {
            return Err(Error::Shape);
        }
        if self.maps != self.plan.maps {
            return Err(Error::Map);
        }
        if self.inputs.coverage != Coverage::SharedSourceFiniteSecondMoment {
            return Err(Error::Unsupported);
        }
        let target = heap.target(self.inputs.source, vec![0])?;
        let variance = heap.covariance_for(self.inputs.covariance, &target, &target)?[0];
        let source_value = heap.values(self.inputs.source)?[0];
        let resistance_value = heap.values(self.inputs.resistance)?[0];
        if resistance_value <= P::ZERO || variance < P::ZERO {
            return Err(Error::Numerical);
        }
        let binding = identity()?;
        self.plan.binding_check(heap, self.inputs, binding)?;
        Ok(crate::Bound::new(
            heap,
            self.plan,
            self.maps,
            [source_value, resistance_value, variance],
            binding,
        ))
    }
}
