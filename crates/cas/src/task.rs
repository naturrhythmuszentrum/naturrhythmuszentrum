// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit task intent, including represented operations with deferred producers.

use crate::domain_model::Layer;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::Handle;
use crate::model::Operation;
use crate::proof_model::DomainContract;

/// The intended mathematical operation; availability never follows merely from representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Task {
    /// Explicitly certify and construct a regular distribution.
    EmbedRegular {
        /// Exact distribution space.
        space: Handle,
    },
    /// Request a point value before constructing any application syntax.
    ApplyAt {
        /// Declared coordinate.
        coordinate: String,
        /// Scalar point expression.
        point: Handle,
    },
    /// Request a product before admitting potentially ill-sorted multiplication syntax.
    Product {
        /// Intended result category.
        requested_sort: crate::model::Sort,
    },
    /// Bounded exact simplification on the current context domain.
    Simplify,
    /// Equality with an explicit domain and definedness contract.
    Equal {
        /// Exact requested region.
        domain: Handle,
        /// Required definedness strength.
        domain_contract: DomainContract,
    },
    /// Exact polynomial expansion.
    Expand,
    /// Explicit differentiation request, with pointwise or distributional meaning.
    Differentiate {
        /// Declared coordinate key.
        coordinate: String,
        /// Requested nonnegative order.
        order: u32,
        /// Mathematical layer.
        layer: Layer,
        /// Exact requested domain.
        domain: Handle,
    },
    /// Ordered scalar Jacobian columns.
    Jacobian(Vec<String>),
    /// Rational univariate square-free multiplicity.
    SquareFree(String),
    /// Finite polynomial integral, with oriented rational bounds.
    Integrate(Box<crate::split_model::SplitRange>),
    /// Inclusive finite polynomial sum, with exact integer endpoints.
    Sum(Box<crate::split_model::SplitRange>),
    /// Standalone finite polynomial limit on an explicit punctured approach.
    Limit(Box<crate::germ_model::GermAt>),
    /// Polynomial side series with exact remainder and explicit analytic mode.
    Series(Box<crate::germ_model::SeriesAt>),
    /// Represented solution set.
    Solve,
    /// Represented spectral operation.
    Spectral,
    /// Elaborate an ordered original case form.
    ElaborateCases,
    /// Select a branch on an explicit region.
    Select {
        /// Requested selection region.
        region: Handle,
    },
    /// Eliminate a finite cover using supplied region premises.
    Eliminate {
        /// Exact member regions.
        regions: Vec<Handle>,
        /// Exact parent theorem whose mechanical restrictions the premises must prove.
        conclusion: Box<crate::proof_model::Statement>,
    },
    /// Merge branch values using an explicit proposed common value.
    Merge {
        /// Regions being compared.
        regions: Vec<Handle>,
        /// Candidate common expression.
        proposed_value: Handle,
    },
    /// Prove that supplied premises cover a domain.
    Coverage {
        /// Domain whose cover is requested.
        domain: Handle,
    },
    /// Request function-indexed facts at one boundary.
    Boundary {
        /// Exact real coordinate for all requested facts.
        coordinate: String,
        /// Boundary object with no built-in ownership claim.
        point: Handle,
        /// Independently requested fact classes.
        requested_facts: Vec<BoundaryFact>,
    },
    /// Pair a distribution with a test or a germ near proved support.
    Pair {
        /// Exact distribution space.
        space: Handle,
        /// Test versus near-support action.
        action_mode: ActionMode,
    },
    /// Multiply a distribution by a proved smooth function.
    SmoothMultiply {
        /// Exact distribution space.
        space: Handle,
    },
    /// Request a represented pullback between explicit spaces.
    Pullback {
        /// Source space.
        source_space: Handle,
        /// Target space.
        target_space: Handle,
        /// Explicit map convention.
        map_kind: String,
    },
}

/// Independent boundary facts; no variant promises that its producer is available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryFact {
    /// Actual point ownership.
    Ownership,
    /// One-sided finite jets.
    Jets {
        /// Largest included derivative order, starting at zero.
        order: u32,
    },
    /// Classical point derivative.
    Derivative {
        /// Positive iterated classical derivative order.
        order: u32,
    },
    /// Right-minus-left jump.
    Jump {
        /// Trace derivative order; zero is the function jump.
        order: u32,
    },
}

/// A near-support germ is distinct from a compactly supported test.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionMode {
    /// Action on a declared test.
    Test,
    /// Action with an explicit support neighborhood premise.
    NearSupport {
        /// Certified neighborhood region.
        neighborhood: Handle,
    },
}

impl Task {
    /// Select an existing scalar producer only for its exact admitted semantics.
    pub(crate) fn scalar(&self) -> Option<Operation> {
        Some(match self {
            Self::Simplify => Operation::Simplify,
            Self::Equal {
                domain_contract: DomainContract::DefinedAndEqual,
                ..
            } => Operation::Equal,
            Self::Expand => Operation::Expand,
            Self::Differentiate {
                coordinate,
                order: 1,
                layer: Layer::Pointwise,
                ..
            } => Operation::Differentiate(coordinate.clone()),
            Self::Jacobian(values) => Operation::Jacobian(values.clone()),
            Self::SquareFree(value) => Operation::SquareFree(value.clone()),
            Self::Solve => Operation::Solve,
            Self::Spectral => Operation::Spectral,
            _ => return None,
        })
    }

    /// Every graph reference participates in validation and request identity.
    pub(crate) fn references(&self) -> Vec<&Handle> {
        match self {
            Self::Limit(at) => vec![&at.point],
            Self::Series(series) => vec![&series.at.point],
            Self::Equal { domain, .. }
            | Self::Differentiate { domain, .. }
            | Self::Coverage { domain } => vec![domain],
            Self::Select { region } => vec![region],
            Self::Eliminate {
                regions,
                conclusion,
            } => {
                let mut references: Vec<_> = regions.iter().collect();
                references.extend(crate::statement::objects(conclusion));
                references
            }
            Self::Merge {
                regions,
                proposed_value,
            } => regions
                .iter()
                .chain(std::iter::once(proposed_value))
                .collect(),
            Self::ApplyAt { point, .. } | Self::Boundary { point, .. } => vec![point],
            Self::Pair {
                space,
                action_mode: ActionMode::NearSupport { neighborhood },
            } => vec![space, neighborhood],
            Self::EmbedRegular { space }
            | Self::Pair { space, .. }
            | Self::SmoothMultiply { space } => vec![space],
            Self::Pullback {
                source_space,
                target_space,
                ..
            } => vec![source_space, target_space],
            _ => Vec::new(),
        }
    }
}

/// Encode all explicit task semantics, including represented but unavailable operations.
pub(crate) fn encode(value: &Task, out: &mut Encoder<'_>) -> Result<(), Fault> {
    encode_with(value, out, crate::statement::handle)
}

/// Encode intent with caller-selected local or portable reference identities.
pub(crate) fn encode_with(
    value: &Task,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.byte(tag(value))?;
    let references = value.references();
    out.count(references.len())?;
    for reference in references {
        write(reference, out)?;
    }
    match value {
        Task::Integrate(range) | Task::Sum(range) => crate::split_wire::range(range, out)?,
        Task::Limit(at) => crate::germ_wire::approach(at, out, &mut write)?,
        Task::Series(series) => {
            crate::germ_wire::approach(&series.at, out, &mut write)?;
            out.unsigned(u64::from(series.order))?;
            out.byte(u8::from(series.analytic))?;
        }
        Task::ApplyAt { coordinate, .. } => out.name(coordinate)?,
        Task::Product { requested_sort } => crate::encoding::sort(requested_sort, out)?,
        Task::Eliminate { conclusion, .. } => {
            crate::statement::encode(conclusion, out, &mut write)?;
        }
        Task::Equal {
            domain_contract, ..
        } => out.byte(match domain_contract {
            DomainContract::DefinedAndEqual => 0,
            DomainContract::SameDefinedDomain => 1,
        })?,
        Task::Differentiate {
            coordinate,
            order,
            layer,
            ..
        } => {
            out.name(coordinate)?;
            out.unsigned(u64::from(*order))?;
            out.byte(layer.tag())?;
        }
        Task::Jacobian(values) => {
            out.count(values.len())?;
            for value in values {
                out.name(value)?;
            }
        }
        Task::SquareFree(value)
        | Task::Pullback {
            map_kind: value, ..
        } => out.name(value)?,
        Task::Boundary {
            coordinate,
            requested_facts,
            ..
        } => {
            out.name(coordinate)?;
            out.count(requested_facts.len())?;
            for fact in requested_facts {
                out.byte(match fact {
                    BoundaryFact::Ownership => 0,
                    BoundaryFact::Jets { .. } => 1,
                    BoundaryFact::Derivative { .. } => 2,
                    BoundaryFact::Jump { .. } => 3,
                })?;
                if let BoundaryFact::Jets { order }
                | BoundaryFact::Derivative { order }
                | BoundaryFact::Jump { order } = fact
                {
                    out.unsigned(u64::from(*order))?;
                }
            }
        }
        Task::Pair { action_mode, .. } => out.byte(u8::from(matches!(
            action_mode,
            ActionMode::NearSupport { .. }
        )))?,
        _ => {}
    }
    Ok(())
}

/// Stable task tags are independent of Rust enum layout.
const fn tag(value: &Task) -> u8 {
    match value {
        Task::Simplify => 0,
        Task::Equal { .. } => 1,
        Task::Expand => 2,
        Task::Differentiate { .. } => 3,
        Task::Jacobian(_) => 4,
        Task::SquareFree(_) => 5,
        Task::Integrate(_) => 6,
        Task::Sum(_) => 7,
        Task::Limit(_) => 8,
        Task::Series(_) => 9,
        Task::Solve => 10,
        Task::Spectral => 11,
        Task::ElaborateCases => 12,
        Task::Select { .. } => 13,
        Task::Eliminate { .. } => 14,
        Task::Merge { .. } => 15,
        Task::Coverage { .. } => 16,
        Task::Boundary { .. } => 17,
        Task::Pair { .. } => 18,
        Task::SmoothMultiply { .. } => 19,
        Task::Pullback { .. } => 20,
        Task::EmbedRegular { .. } => 21,
        Task::ApplyAt { .. } => 22,
        Task::Product { .. } => 23,
    }
}
