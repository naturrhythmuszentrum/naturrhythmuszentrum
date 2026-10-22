// SPDX-License-Identifier: AGPL-3.0-only
//! Representations prove complete reconstruction on the exact coordinate family.

use super::{PhysicalReading, ReadingError, meaning, support};
use nrz_cas::{
    Cas, Handle, Rational,
    domain::Layer,
    limits::Budget,
    proof::{DomainContract, Judgment, Statement, VerifiedWitness},
    readings::AffineMap,
    syntax::{AtomicSort, Field, Sort},
};

/// Closed layout contract, distinct from a symbol's mathematical sort.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepresentationKind {
    /// Exact fixed coordinates with no numerical storage or numerical companions.
    None,
    /// Ordered real and imaginary coordinates in a two-entry column.
    ComponentPair,
    /// Complete Complex block with logical coordinates `(a,b,-b,a)`.
    ComplexBlock,
    /// An explicit reconstruction map with a checked whole-family equation.
    Defined,
}

/// Proposed representation, with actual reconstruction declarations rather than names.
#[derive(Clone, Debug)]
pub enum RepresentationDefinition {
    /// Request exact constancy on the complete family, without numerical storage.
    Exact,
    /// Reconstruct the complete pair under this declared two-entry matrix symbol.
    ComponentPair(Handle),
    /// Reconstruct the complete block under this declared two-by-two matrix symbol.
    ComplexBlock(Handle),
    /// Another explicit layout must supply its full reconstruction map and declaration.
    Defined {
        /// Actual declared reconstruction symbol, with the selected source's exact sort.
        symbol: Handle,
        /// Complete expansion from logical values to selected source entries.
        expansion: AffineMap,
    },
}

/// Independently rebuilt expected reconstruction, without any admitted proof.
#[derive(Clone, Debug)]
struct Expected {
    /// Closed physical representation interpretation.
    kind: RepresentationKind,
    /// Actual complete reconstruction expression.
    node: Handle,
    /// Complete exact reconstruction or no-storage constancy map.
    expansion: AffineMap,
    /// Full expected scoped equality, built from original definitions.
    judgment: Judgment,
}

/// Checked representation reconstruction; numerical family admission remains external.
#[derive(Clone, Debug)]
pub struct Representation {
    /// Exact physical reading whose values are represented.
    reading: PhysicalReading,
    /// Independently constructed complete reconstruction and expected equality.
    expected: Expected,
    /// Discharged complete-family reconstruction or constancy witness.
    witness: VerifiedWitness,
}

impl Representation {
    /// Prove the complete representation, including every repeated or negated entry.
    /// # Errors
    /// Refuses center-only structure, wrong order/sign, missing equations and exact limits.
    pub fn prove(
        cas: &mut Cas,
        reading: &PhysicalReading,
        definition: RepresentationDefinition,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let expected = build(cas, reading, definition, budget)?;
        let Statement::Equality { left, right, .. } = &expected.judgment.statement else {
            return Err(ReadingError::Definition);
        };
        let witness =
            cas.prove_reading_equality(reading.family().context(), left, right, budget)?;
        Ok(Self {
            reading: reading.clone(),
            expected,
            witness,
        })
    }

    /// Rebuild an unproved expected representation before replay in a fresh graph.
    /// # Errors
    /// Refuses substituted layout, copy, definition, family/domain or proof bytes.
    pub fn replay(
        cas: &mut Cas,
        reading: &PhysicalReading,
        definition: RepresentationDefinition,
        bytes: &[u8],
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let expected = build(cas, reading, definition, budget)?;
        let witness = cas.replay(bytes, &expected.judgment, budget)?;
        Ok(Self {
            reading: reading.clone(),
            expected,
            witness,
        })
    }
    /// Closed layout interpretation; `None` requires the retained constancy witness.
    #[must_use]
    pub const fn kind(&self) -> RepresentationKind {
        self.expected.kind
    }
    /// Actual physical reading, including every original selected entry.
    #[must_use]
    pub const fn reading(&self) -> &PhysicalReading {
        &self.reading
    }
    /// Actual CAS expression for the complete reconstruction or fixed exact value.
    #[must_use]
    pub const fn reconstruction(&self) -> &Handle {
        &self.expected.node
    }
    /// Complete reconstruction coefficients and offsets.
    #[must_use]
    pub const fn expansion(&self) -> &AffineMap {
        &self.expected.expansion
    }
    /// Whole-family proof, which establishes no statement about rounded centers.
    #[must_use]
    pub const fn witness(&self) -> &VerifiedWitness {
        &self.witness
    }
}

/// Exact None and numerical layouts have different expected left endpoints.
fn build(
    cas: &mut Cas,
    reading: &PhysicalReading,
    definition: RepresentationDefinition,
    budget: &mut Budget,
) -> Result<Expected, ReadingError> {
    let (kind, expansion, node) = match definition {
        RepresentationDefinition::Exact => exact(cas, reading, budget)?,
        other => reconstruct(cas, reading, other, budget)?,
    };
    let left = if kind == RepresentationKind::None {
        reading.node()
    } else {
        reading.selected()
    };
    let context = reading.family().context();
    let judgment = Judgment {
        context: context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: left.clone(),
            right: node.clone(),
            domain: cas.admissible_region(context)?,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    };
    Ok(Expected {
        kind,
        node,
        expansion,
        judgment,
    })
}

/// No numerical slot can disappear unless every free-coordinate coefficient vanishes.
fn exact(
    cas: &mut Cas,
    reading: &PhysicalReading,
    budget: &mut Budget,
) -> Result<(RepresentationKind, AffineMap, Handle), ReadingError> {
    let expansion = reading.family_map(cas, budget)?;
    if expansion
        .coefficients()
        .iter()
        .any(|value| !value.is_zero())
    {
        return Err(ReadingError::Unsupported);
    }
    let expected = cas.define_reading(
        reading.symbol(),
        reading.family().root(),
        &expansion,
        budget,
    )?;
    Ok((RepresentationKind::None, expansion, expected))
}

/// Construct complete reconstruction syntax; no caller-supplied equation gains authority.
fn reconstruct(
    cas: &mut Cas,
    reading: &PhysicalReading,
    definition: RepresentationDefinition,
    budget: &mut Budget,
) -> Result<(RepresentationKind, AffineMap, Handle), ReadingError> {
    let (kind, symbol, expansion) = match definition {
        RepresentationDefinition::ComponentPair(symbol) => {
            complex(reading, (2, 1))?;
            (
                RepresentationKind::ComponentPair,
                symbol,
                AffineMap::identity(2, budget)?,
            )
        }
        RepresentationDefinition::ComplexBlock(symbol) => {
            complex(reading, (2, 2))?;
            (RepresentationKind::ComplexBlock, symbol, block_map(budget)?)
        }
        RepresentationDefinition::Defined { symbol, expansion } => {
            (RepresentationKind::Defined, symbol, expansion)
        }
        RepresentationDefinition::Exact => return Err(ReadingError::Definition),
    };
    let selected = selected_meanings(reading, budget)?;
    meaning::map_meanings(&expansion, reading.entries(), &selected, budget)?;
    let (sort, _, _, _) = support::inspect(cas, reading.selected(), budget)?;
    let (declared, _, _, header) = support::inspect(cas, &symbol, budget)?;
    if declared != sort || header != cas.context_header(reading.family().context())? {
        return Err(ReadingError::Definition);
    }
    let expected = cas.define_reading(&symbol, reading.node(), &expansion, budget)?;
    Ok((kind, expansion, expected))
}

/// Pair and block contracts require Complex sorts and the exact complete layout shape.
fn complex(reading: &PhysicalReading, shape: (u32, u32)) -> Result<(), ReadingError> {
    let complex = reading.sort() == &Sort::Value(AtomicSort::Scalar(Field::Complex));
    if complex && reading.selection().shape() == Some(shape) {
        Ok(())
    } else {
        Err(ReadingError::Unsupported)
    }
}

/// Canonical complete block coefficients are explicit exact integers.
fn block_map(budget: &mut Budget) -> Result<AffineMap, ReadingError> {
    let mut coefficients = support::reserve(8, budget)?;
    for value in [1, 0, 0, 1, 0, -1, 1, 0] {
        coefficients.push(Rational::from_ratio(value, 1, budget)?);
    }
    Ok(AffineMap::new(
        2,
        4,
        coefficients,
        vec![Rational::zero(); 4],
        budget,
    )?)
}

/// Selected physical coordinates include every expanded copy, even unread ones.
fn selected_meanings(
    reading: &PhysicalReading,
    budget: &mut Budget,
) -> Result<Vec<super::EntryMeaning>, ReadingError> {
    let mut entries = support::reserve(reading.selection().indices().len(), budget)?;
    for index in reading.selection().indices() {
        let position =
            u64::from(index.column) * u64::from(reading.family().shape()[0]) + u64::from(index.row);
        entries.push(
            reading
                .family()
                .entries()
                .get(usize::try_from(position).map_err(|_| ReadingError::Shape)?)
                .ok_or(ReadingError::Shape)?
                .clone(),
        );
    }
    Ok(entries)
}
