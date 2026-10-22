// SPDX-License-Identifier: AGPL-3.0-only
//! Public physical-reading fixtures compiled by the public test target.

use nrz_bindings::readings::{CoordinateFamily, EntryMeaning, PhysicalReading, ReadingDefinition};
use nrz_bindings::{
    Budget, Descriptor, Dimension, Kind, Limits, Measure, ReferenceDefinition, Role,
};
use nrz_cas::{
    Cas, Handle, Rational, Reference,
    domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
    readings::{AffineMap, EntryIndex, SelectionSpec},
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
        let mut cas = Cas::new(Limits::default())?;
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
/// A whole reading and an independently defined component of the same family.
pub(crate) struct Case {
    /// Complete independent graph and context.
    pub(crate) fixture: Fixture,
    /// The whole state or Complex interpretation.
    pub(crate) whole: PhysicalReading,
    /// A Real component reading of specified source entries.
    pub(crate) component: PhysicalReading,
}
/// Mixed volts and amperes use one ordered two-coordinate family.
pub(crate) fn state_case(component: u32) -> TestResult<Case> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("v", real()),
        ("i", real()),
        ("rows", Sort::Value(AtomicSort::Integer)),
        ("columns", Sort::Value(AtomicSort::Integer)),
    ])?;
    let entries = vec![
        meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?,
        meaning("current", [0, 0, 0, 1, 0, 0, 0])?,
    ];
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        entries.clone(),
        &mut ledger()?,
    )?;
    let whole = fixture.define(
        &family,
        "x",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        entries.clone(),
    )?;
    let entry = entries
        .get(usize::try_from(component)?)
        .ok_or("component")?
        .clone();
    let selected = SelectionSpec::Entry(EntryIndex {
        row: component,
        column: 0,
    });
    let component = fixture.define(
        &family,
        if component == 0 { "v" } else { "i" },
        selected,
        AffineMap::identity(1, &mut ledger()?)?,
        vec![entry],
    )?;
    Ok(Case {
        fixture,
        whole,
        component,
    })
}
/// The full block expansion retains the negative and repeated coordinates.
pub(crate) fn complex_case(copy: u32, sign: i64, structured: bool) -> TestResult<Case> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("block", matrix(2, 2)),
        ("c", Sort::Value(AtomicSort::Scalar(Field::Complex))),
        ("r", real()),
    ])?;
    let entry = meaning("impedance", [2, 1, -3, -2, 0, 0, 0])?;
    let family = if structured {
        let family = CoordinateFamily::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("z")?,
            vec![entry.clone(); 2],
            &mut ledger()?,
        )?;
        let symbol = fixture.symbol("block")?;
        family.expand(
            &mut fixture.cas,
            &symbol,
            &map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0, 0, 0, 0])?,
            vec![entry.clone(); 4],
            &mut ledger()?,
        )?
    } else {
        CoordinateFamily::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("block")?,
            vec![entry.clone(); 4],
            &mut ledger()?,
        )?
    };
    let whole = fixture.define(
        &family,
        "c",
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0, 0])?,
        vec![entry.clone(); 2],
    )?;
    let component = fixture.define(
        &family,
        "r",
        SelectionSpec::Entry(EntryIndex {
            row: copy,
            column: copy,
        }),
        map(1, 1, &[sign], &[0])?,
        vec![entry],
    )?;
    Ok(Case {
        fixture,
        whole,
        component,
    })
}

/// Check exact Work, cumulative precharge and staged-payload boundaries without raising a limit.
pub(crate) fn boundaries(mut attempt: impl FnMut(Limits, u64) -> TestResult<u64>) -> TestResult {
    let needed = attempt(Limits::default(), 0)?;
    assert!(needed > 0);
    assert!(
        attempt(
            Limits {
                work: needed,
                ..Limits::default()
            },
            0
        )
        .is_ok()
    );
    assert!(
        attempt(
            Limits {
                work: needed - 1,
                ..Limits::default()
            },
            0
        )
        .is_err()
    );
    assert!(
        attempt(
            Limits {
                work: needed,
                ..Limits::default()
            },
            1
        )
        .is_err()
    );
    assert!(
        attempt(
            Limits {
                staged_payload_bytes: 0,
                ..Limits::default()
            },
            0
        )
        .is_err()
    );
    let mut lower = 0;
    let mut upper = Limits::default().staged_payload_bytes;
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if attempt(
            Limits {
                staged_payload_bytes: middle,
                ..Limits::default()
            },
            0,
        )
        .is_ok()
        {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    attempt(
        Limits {
            staged_payload_bytes: upper,
            ..Limits::default()
        },
        0,
    )?;
    assert!(
        attempt(
            Limits {
                staged_payload_bytes: upper - 1,
                ..Limits::default()
            },
            0
        )
        .is_err()
    );
    Ok(())
}
