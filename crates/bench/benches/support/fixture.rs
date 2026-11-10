// SPDX-License-Identifier: AGPL-3.0-only
//! Bench-owned construction of the matrix example's checked exponential problem.
//! Adapted from `crates/matrix/examples/prepared_exponential/support.rs`; preparation
//! creates exact authority before either sampling host starts its timer.
use super::BenchResult;
use nrz_bindings::readings::algebra::domain::{
    AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr,
};
use nrz_bindings::readings::algebra::enclosure::prepared_expm1::Request;
use nrz_bindings::readings::algebra::enclosure::{
    CoordinateBound, EnclosureGoal, EnclosureRequest, Interval,
};
use nrz_bindings::readings::algebra::limits::{Budget, Limits};
use nrz_bindings::readings::algebra::syntax::{
    AtomicSort, Declaration, Environment, Field, Operator, Sort,
};
use nrz_bindings::readings::algebra::syntax::{Draft, ExpressionDraft};
use nrz_bindings::readings::algebra::{Cas, Fault, Handle, Rational, Reference};
/// Finite fixture allowances, independent of observation storage.
pub(super) fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits {
        work: 2_000_000_000,
        integer_exponent: 4096,
        arithmetic_steps: 50_000_000,
        name_bytes: 131_072,
        literal_bytes: 618,
        source_bytes: 4_194_304,
        output_bytes: 4_194_304,
        witness_bytes: 4_194_304,
        ..Limits::default()
    })
}
/// Construct one exact fixture rational outside the native clocks.
fn number(n: i64, d: u64) -> Result<Rational, Fault> {
    Rational::from_ratio(n, d, &mut ledger()?)
}
/// Original claim graph with one real coordinate and its root context.
#[derive(Debug)]
pub(super) struct Fixture {
    /// Graph that owns every admitted original claim.
    pub(super) cas: Cas,
    /// Declaration authority for fixture expressions.
    header: Handle,
    /// Root assumptions for the original enclosure.
    context: Handle,
}
impl Fixture {
    /// Build the one-coordinate environment used by every measured route.
    pub(super) fn new() -> BenchResult<Self> {
        Self::with_names(&["x"])
    }
    /// Supply bounded store limits for named fixture declarations.
    fn with_names(names: &[&str]) -> BenchResult<Self> {
        Self::with_names_and_limits(
            names,
            Limits {
                work: 2_000_000_000,
                integer_exponent: 4096,
                arithmetic_steps: 50_000_000,
                ..Limits::default()
            },
        )
    }
    /// Admit the original header, ambient region and root context.
    fn with_names_and_limits(names: &[&str], limits: Limits) -> BenchResult<Self> {
        let mut environment = Environment::default();
        for name in names {
            environment.declarations.push(Declaration {
                key: (*name).into(),
                display_name: (*name).into(),
                sort: real(),
            });
        }
        let mut cas = Cas::new(limits)?;
        let drafts = [
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
        ];
        let [header, _, context]: [Handle; 3] = cas
            .admit(&drafts, &mut ledger()?)?
            .try_into()
            .map_err(|_| "fixture")?;
        Ok(Self {
            cas,
            header,
            context,
        })
    }
    /// Admit one exact expression under the original header.
    fn term(&mut self, operator: Operator, operands: &[Handle]) -> BenchResult<Handle> {
        let draft = Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: real(),
            header: Reference::Existing(self.header.clone()),
            scope: Vec::new(),
            binding: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        });
        self.cas
            .admit(&[draft], &mut ledger()?)?
            .into_iter()
            .next()
            .ok_or_else(|| "term".into())
    }

    /// Attach the original context to a target without numerical work.
    fn request(&self, target: Handle) -> EnclosureRequest {
        EnclosureRequest {
            target,
            context: self.context.clone(),
            coordinates: Vec::new(),
            goal: EnclosureGoal::Width,
        }
    }
}
/// Sort of the fixture coordinate and every expression.
const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}

/// Encode the checked law as exp(argument) minus one.
fn expression(f: &mut Fixture, argument: Handle, bits: u32) -> BenchResult<Request> {
    let e = f.term(Operator::Exponential, &[argument])?;
    let one = f.term(Operator::Rational(Rational::one()), &[])?;
    let target = f.term(Operator::Sub, &[e, one])?;
    Ok(Request {
        original: f.request(target),
        bits,
        environment: 1,
    })
}

/// Bind the entire admitted interval to the original coordinate.
pub(super) fn broad(f: &mut Fixture, bits: u32) -> BenchResult<Request> {
    let x = f.term(Operator::Free("x".into()), &[])?;
    let mut r = expression(f, x.clone(), bits)?;
    r.original.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: Interval::new(number(-2, 1)?, Rational::zero(), &mut ledger()?)?,
    });
    Ok(r)
}
