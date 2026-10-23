// SPDX-License-Identifier: AGPL-3.0-only
//! Public exact-reading builders shared by the library and public test binaries.

use nrz_bindings::readings::algebra as nrz_cas;
use nrz_bindings::readings::{CoordinateFamily, EntryMeaning, PhysicalReading, ReadingDefinition};
use nrz_bindings::{
    Budget, Descriptor, Dimension, Kind, Limits, Measure, ReferenceDefinition, Role,
};
use nrz_cas::{
    Cas, Handle, Rational, Reference,
    domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
    readings::{AffineMap, SelectionSpec},
    syntax::{
        AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Extent, Field, Operator, Sort,
    },
};

/// Fallible public fixture and test result.
pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
/// Unchanged production allowance for an independent fixture operation.
pub(crate) fn ledger() -> Result<Budget, nrz_cas::Fault> {
    Budget::new(Limits::default())
}
/// Literal Real rectangle in logical column order.
pub(crate) const fn matrix(rows: u32, columns: u32) -> Sort {
    Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(rows),
        columns: Extent::Literal(columns),
    })
}
/// Mathematical Real scalar sort.
pub(crate) const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}
/// Exact explicit coefficients, independently of the map's composition algorithm.
pub(crate) fn map(
    inputs: u32,
    outputs: u32,
    coefficients: &[i64],
    offsets: &[i64],
) -> TestResult<AffineMap> {
    let values = |entries: &[i64]| -> TestResult<Vec<Rational>> {
        entries
            .iter()
            .map(|value| Ok(Rational::from_ratio(*value, 1, &mut ledger()?)?))
            .collect()
    };
    Ok(AffineMap::new(
        inputs,
        outputs,
        values(coefficients)?,
        values(offsets)?,
        &mut ledger()?,
    )?)
}
/// One physical kind with its own exact unit reference.
pub(crate) fn meaning(name: &str, dimensions: [i64; 7]) -> TestResult<EntryMeaning> {
    let descriptor = Descriptor::new(
        Kind::Named(name.into(), Box::new(Dimension::integers(dimensions)?)),
        Measure::coherent(),
        Role::Difference,
    )?;
    let reference = ReferenceDefinition::unit(name, &descriptor)?;
    Ok(EntryMeaning::new(
        descriptor,
        reference,
        None,
        &mut ledger()?,
    )?)
}
/// A graph with explicit original declarations and no setup proof.
pub(crate) struct Fixture {
    /// Owning exact graph.
    pub(crate) cas: Cas,
    /// Exact root context and domain.
    pub(crate) context: Handle,
    /// Original declaration identities; display names deliberately coincide.
    pub(crate) symbols: Vec<(String, Handle)>,
}
impl Fixture {
    /// Construct an independent receiver without inspecting a sender's proof.
    pub(crate) fn new(declarations: &[(&str, Sort)]) -> TestResult<Self> {
        Self::with_limits(declarations, Limits::default())
    }
    /// Explicit permanent ceilings for caller-budget measurements; ordinary fixtures keep defaults.
    pub(crate) fn with_limits(declarations: &[(&str, Sort)], limits: Limits) -> TestResult<Self> {
        let mut cas = Cas::new(limits)?;
        let environment = Environment {
            declarations: declarations
                .iter()
                .map(|(key, sort)| Declaration {
                    key: (*key).into(),
                    display_name: "same display".into(),
                    sort: sort.clone(),
                })
                .collect(),
            ..Environment::default()
        };
        let nodes = cas.admit(
            &[
                Draft::Header(HeaderDraft::Core(environment)),
                Draft::Region(RegionDraft {
                    header: Reference::Local(0),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Assignments,
                    expression: RegionExpr::Ambient,
                }),
                Draft::Context(ContextDraft::Root {
                    header: Reference::Local(0),
                    admissible: Reference::Local(1),
                    assumptions: Vec::new(),
                }),
            ],
            &mut ledger()?,
        )?;
        let context = nodes.get(2).ok_or("context")?.clone();
        let mut symbols = Vec::new();
        for (key, sort) in declarations {
            let draft = Draft::Expression(ExpressionDraft {
                operator: Operator::Free((*key).into()),
                sort: sort.clone(),
                header: Reference::Existing(nodes[0].clone()),
                scope: Vec::new(),
                binding: None,
                space: None,
                operands: Vec::new(),
            });
            let node = cas.admit(&[draft], &mut ledger()?)?.pop().ok_or("symbol")?;
            symbols.push(((*key).into(), node));
        }
        Ok(Self {
            cas,
            context,
            symbols,
        })
    }
    /// Resolve an original declaration by fixture key, never a reading by display name.
    pub(crate) fn symbol(&self, key: &str) -> TestResult<Handle> {
        self.symbols
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, node)| node.clone())
            .ok_or_else(|| "missing symbol".into())
    }
    /// Admit one physical reading through the public constructor.
    pub(crate) fn define(
        &mut self,
        family: &CoordinateFamily,
        key: &str,
        selection: SelectionSpec,
        affine: AffineMap,
        entries: Vec<EntryMeaning>,
    ) -> TestResult<PhysicalReading> {
        let definition = ReadingDefinition {
            symbol: self.symbol(key)?,
            selection,
            map: affine,
            entries,
        };
        Ok(PhysicalReading::define(
            &mut self.cas,
            family,
            definition,
            &mut ledger()?,
        )?)
    }
}
