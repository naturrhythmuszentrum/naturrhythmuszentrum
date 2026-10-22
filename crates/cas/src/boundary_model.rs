// SPDX-License-Identifier: AGPL-3.0-only
//! Exact finite boundary evidence and explicitly ordered derivative statements.
//!
//! # Boundary tasks and checked jets
//!
//! `Task::Boundary` names its real coordinate and point. `Jets { order }` requests traces from zero through that order; `Jump { order }` requests the right-minus-left trace difference; `Derivative { order }` names a positive iterated classical derivative order. Complete finite partition evidence reconstructs every reachable case selector and requested-domain endpoint. Geometry alone supplies no analytic regularity: each jet additionally requires total local equality to a rational polynomial and an independently checked incident open neighborhood.
//!
//! `jet.polynomial` checks factorial-weighted shifted coefficients, with separate closed partition, trace-projection and jump-introduction modes. The derived `jet.jump` recipe records both projections and exact subtraction. `diff.case-interior` transports an existing scalar derivative through local equality on an open cell; its assembly mode checks all cells of the claimed derivative region. `diff.case-boundary` first compares both function traces with actual function-specific ownership, then compares slopes. Missing ownership or a side stays pending. Thus equal branch slopes do not rescue J's jump.

use crate::domain_model::Side;
use crate::graph::Handle;
use crate::number::Rational;
use crate::region::RegionCertificate;

/// A transparent named function; the retained operand is its checked canonical case definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaseName {
    /// Absolute value, including value zero at zero.
    Abs,
    /// Sign with value zero at zero.
    Sign,
    /// Positive part, including value zero at zero.
    Ramp,
    /// Unit step with an explicit rational value at zero.
    Step,
}

/// Complete name parameters, independent of a display spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedCase {
    /// Exact defining family.
    pub name: CaseName,
    /// Declared real coordinate.
    pub coordinate: String,
    /// Present exactly for the step's point value.
    pub parameter: Option<Rational>,
}

/// Complete partition of a function's case selectors on an exact real region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundaryPartition {
    /// Function whose complete selector graph is inspected.
    pub function: Handle,
    /// Requested region, including its endpoint ownership.
    pub domain: Handle,
    /// Exact real coordinate.
    pub coordinate: String,
    /// All canonical rational cut locations, including exterior cuts.
    pub boundaries: Vec<Handle>,
    /// Ordered open and singleton cells intersected with the requested domain.
    pub cells: Vec<Handle>,
}

/// A classical derivative on an explicitly claimed domain and at an explicit order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseDerivative {
    /// Original input function.
    pub function: Handle,
    /// Actual derivative function with established point values.
    pub derivative: Handle,
    /// Declared real coordinate.
    pub coordinate: String,
    /// Positive iterated derivative order.
    pub order: u32,
    /// Exact region where the derivative exists.
    pub domain: Handle,
}

/// A classical derivative value at one actual point of a function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointDerivative {
    /// Original input function.
    pub function: Handle,
    /// Shared boundary object.
    pub boundary: Handle,
    /// Exact real coordinate.
    pub coordinate: String,
    /// Positive iterated derivative order.
    pub order: u32,
    /// Exact derivative value.
    pub value: Handle,
}

/// The finite obstruction checked by the boundary derivative schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NonexistenceReason {
    /// This side's finite trace differs from the actual owned point value.
    Discontinuous(Side),
    /// Both traces match the point, but the two first derivative traces differ.
    UnequalSlopes,
}

/// A witnessed classical nonexistence statement, never a search status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointNonexistence {
    /// Original input function.
    pub function: Handle,
    /// Exact point where the requested derivative fails.
    pub boundary: Handle,
    /// Declared real coordinate.
    pub coordinate: String,
    /// Positive iterated derivative order.
    pub order: u32,
    /// Independently checked finite obstruction.
    pub reason: NonexistenceReason,
}

/// One projection from a previously checked function-specific side jet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceValue {
    /// Exact function.
    pub function: Handle,
    /// Exact location.
    pub boundary: Handle,
    /// Oriented side.
    pub side: Side,
    /// Projected derivative order.
    pub order: u32,
    /// Finite rational trace.
    pub value: Handle,
}

/// Untrusted data for the closed boundary-calculus modes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundaryCertificate {
    /// Complete selector inventory and exact reconstructed partition.
    Partition {
        /// Structurally ordered region dependencies.
        regions: Vec<Handle>,
        /// All affine cuts and membership rows.
        partition: RegionCertificate,
    },
    /// Polynomial germ, compared independently with its exact coefficient vector.
    Polynomial {
        /// Function agreeing with the input on the incident neighborhood.
        polynomial: Handle,
        /// Canonical ascending coefficients; zero has an empty vector.
        coefficients: Vec<Rational>,
    },
    /// Projection from one checked jet.
    Projection,
    /// Two projections and an exact rational subtraction application.
    Jump,
    /// Local equality and an independently checked branch derivative.
    Interior,
    /// Actual ownership and two checked incident jets, with any previous derivative.
    Point,
    /// Complete cover and a checked derivative fact for every nonempty cell.
    Assemble,
}
