// SPDX-License-Identifier: AGPL-3.0-only
//! Shared dyadic resistor example; construction itself grants no plan validation.

use nrz_algebra::{Limits, Rational, Symbol};
use nrz_units::{
    BinaryOp, Context, Declaration, Descriptor, Dimension, Evidence, ExponentPolicy, Kind, Measure,
    Registry, Role, UncertaintyMode, uncertainty,
};

use crate::{
    Axis, CoordinateMap, Coverage, Error, Heap, Inputs, Layout, Maps, Plan, Scalar, SchemaPool,
    Shape, ShapeId, Unchecked,
};

/// CAS quantities shared with the static comparison; dimensions have one authority.
#[derive(Clone, Debug)]
pub struct ResistorQuantities {
    /// Voltage differences, not affine potential points.
    pub voltage: Descriptor,
    /// Current through the resistor.
    pub current: Descriptor,
    /// Positive resistance.
    pub resistance: Descriptor,
}

impl ResistorQuantities {
    /// Create coherent physical meanings for the small example.
    ///
    /// # Errors
    /// Preserves descriptor construction and exact dimension errors.
    pub fn new() -> Result<Self, Error> {
        let voltage = Dimension::integers([2, 1, -3, -1, 0, 0, 0]);
        let current = Dimension::integers([0, 0, 0, 1, 0, 0, 0]);
        let descriptor = |name, dimension| -> Result<Descriptor, Error> {
            Ok(Descriptor::new(
                Kind::Named(Symbol::new(name)?, dimension),
                Measure::coherent(),
                Role::Difference,
            )?)
        };
        Ok(Self {
            voltage: descriptor("voltage_difference", voltage)?,
            current: descriptor("resistor_current", current)?,
            resistance: descriptor("resistance", voltage.quotient(current)?)?,
        })
    }

    /// Build the explicitly registered operation snapshot used by both approaches.
    ///
    /// # Errors
    /// Preserves CAS registration failures.
    pub fn registry(&self) -> Result<Registry, Error> {
        let mut registry = Registry::new(ExponentPolicy::Rational);
        registry.register_binary(
            BinaryOp::Multiply,
            self.resistance.kind().clone(),
            self.current.kind().clone(),
            self.voltage.kind().clone(),
        )?;
        registry.register_binary(
            BinaryOp::Divide,
            self.voltage.kind().clone(),
            self.resistance.kind().clone(),
            self.current.kind().clone(),
        )?;
        Ok(registry)
    }
}

/// Unchecked plan, physical inputs and coordinate contracts for an executable example.
#[derive(Debug)]
pub struct Fixture<P> {
    /// Owned physical storage; generic only in numerical precision.
    pub heap: Heap<P>,
    /// Original expressions have yet to pass checking.
    pub plan: Plan<Unchecked>,
    /// Separate object identities and shared-source coverage.
    pub inputs: Inputs<P>,
    /// Maps are proposed independently of shape equality.
    pub maps: Maps,
}

/// Intern a column vector with meaningful row coordinates and a ratio scalar axis.
fn vector(pool: &mut SchemaPool, entries: &[(&str, &Descriptor)]) -> Result<ShapeId, Error> {
    let ratio = pool.quantity(Descriptor::ratio())?;
    let mut rows = Vec::new();
    for (meaning, descriptor) in entries {
        rows.push(Axis {
            meaning: (*meaning).into(),
            quantity: pool.quantity((*descriptor).clone())?,
        });
    }
    let quantities = rows.iter().map(|axis| axis.quantity).collect();
    pool.shape(Shape {
        rows,
        columns: vec![Axis {
            meaning: "scalar".into(),
            quantity: ratio,
        }],
        entries: quantities,
    })
}

/// Intern a paired covariance schema using the target vector's ordered coordinates.
fn covariance_shape(pool: &mut SchemaPool, value: ShapeId) -> Result<ShapeId, Error> {
    let axes = pool.shape_definition(value)?.rows.clone();
    let mut entries = Vec::new();
    for right in &axes {
        for left in &axes {
            let descriptor = uncertainty(
                UncertaintyMode::Covariance,
                pool.descriptor(left.quantity)?,
                Some(pool.descriptor(right.quantity)?),
                None,
            )?;
            entries.push(pool.quantity(descriptor)?);
        }
    }
    pool.shape(Shape {
        rows: axes.clone(),
        columns: axes,
        entries,
    })
}

/// Build the admitted dyadic fixture without granting checked or bound state.
///
/// # Errors
/// Preserves construction, CAS declaration, schema and storage admission failures.
pub fn resistor_fixture<P: Scalar>() -> Result<Fixture<P>, Error> {
    let quantities = ResistorQuantities::new()?;
    let registry = quantities.registry()?;
    let mut context = Context::new(registry.clone(), Limits::default());
    for (slot, (name, descriptor)) in [
        ("s", &quantities.voltage),
        ("R", &quantities.resistance),
        ("v", &quantities.voltage),
        ("i", &quantities.current),
        ("zero_v", &quantities.voltage),
        ("zero_r", &quantities.resistance),
    ]
    .iter()
    .enumerate()
    {
        let declaration = if name.starts_with("zero_") {
            Declaration::Constant {
                descriptor: (*descriptor).clone(),
                value: Rational::ZERO,
                evidence: Evidence::default(),
            }
        } else {
            Declaration::Input {
                descriptor: (*descriptor).clone(),
                nonzero: *name == "R",
                evidence: Evidence::default(),
            }
        };
        context
            .declare(Symbol::new(*name)?, declaration)
            .map_err(|source| Error::Check {
                node: 0,
                slot: Some(slot),
                reference: None,
                binding: None,
                source: Box::new(source),
            })?;
    }
    let mut pool = SchemaPool::new(registry)?;
    let source = vector(&mut pool, &[("source_voltage", &quantities.voltage)])?;
    let resistance = vector(&mut pool, &[("resistor", &quantities.resistance)])?;
    let solution = vector(
        &mut pool,
        &[
            ("node_voltage", &quantities.voltage),
            ("resistor_current", &quantities.current),
        ],
    )?;
    let source_covariance = covariance_shape(&mut pool, source)?;
    let covariance = covariance_shape(&mut pool, solution)?;
    let layout = Layout {
        source,
        resistance,
        source_covariance,
        solution,
        covariance,
    };
    let voltage_map = CoordinateMap::new(
        quantities.voltage.clone(),
        (&quantities.voltage, Rational::integer(2)),
        None,
    )?;
    let current_map = CoordinateMap::new(
        quantities.current.clone(),
        (&quantities.current, Rational::ONE),
        None,
    )?;
    let maps = Maps {
        solution: [voltage_map.clone(), current_map],
        equations: [voltage_map.clone(), voltage_map],
    };
    let plan = Plan::resistor(
        context,
        [quantities.voltage, quantities.current],
        layout,
        maps.clone(),
        &pool,
    )?;
    let mut heap = Heap::new(pool)?;
    let source = heap.value(source, vec![P::rational(Rational::integer(4))])?;
    let resistance = heap.value(resistance, vec![P::rational(Rational::integer(2))])?;
    let target = heap.target(source, vec![0])?;
    let covariance = heap.covariance(
        source_covariance,
        vec![P::rational(Rational::new(1, 4)?)],
        target.clone(),
        target,
    )?;
    Ok(Fixture {
        heap,
        plan,
        inputs: Inputs {
            source,
            resistance,
            covariance,
            coverage: Coverage::SharedSourceFiniteSecondMoment,
        },
        maps,
    })
}
