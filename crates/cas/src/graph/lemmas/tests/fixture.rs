// SPDX-License-Identifier: AGPL-3.0-only
//! One original scope exercises every retained judgment family through its checker.

use crate::budget::{Budget, Limits};
use crate::decision::{
    EnclosureOrigin, RationalEnclosure, ScalarKernelCertificate, ScalarKernelLaw,
    ScalarKernelRecord,
};
use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::enclosure::{EnclosureGoal, EnclosureRequest, RefinementPolicy};
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::model::{Constant, Environment, Operator};
use crate::pi_bounds::PiBoundCertificate;
use crate::proof_model::{
    ApplicationDraft, Certificate, DomainContract, Statement, VerifiedWitness,
};
use crate::rule::Primitive;
use crate::testing::common::TestResult;
use crate::testing::native::{admit, term};
use crate::{Fault, Rational};

/// All measured operations use unchanged default ceilings.
pub(super) fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// Independent source handles and portable evidence precede measured retention.
pub(super) struct Fixture {
    /// Original receiving store.
    pub(super) cas: Cas,
    /// Exact root context.
    pub(super) context: Handle,
    /// Original closed rational target.
    pub(super) one: Handle,
    /// Original anchor for the Machin certificate.
    pub(super) pi: Handle,
    /// Independently checked scalar trace request.
    pub(super) scalar: RationalEnclosure,
    /// Portable proof prepared before measuring retention.
    pub(super) witness: VerifiedWitness,
}

impl Fixture {
    /// Admit source syntax and independently check the portable reflexivity proof.
    pub(super) fn new() -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let header = admit(
            &mut cas,
            Draft::Header(HeaderDraft::Core(Environment::default())),
        )?;
        let one = term(&mut cas, &header, Operator::Rational(Rational::one()), &[])?;
        let pi = term(&mut cas, &header, Operator::Constant(Constant::Pi), &[])?;
        let region = admit(
            &mut cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
        )?;
        let context = admit(
            &mut cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        let scalar = scalar(&one, &context, &region);
        let witness = cas.check_proof(
            &context,
            &[ApplicationDraft {
                rule: Primitive::Reflexivity,
                context: context.clone(),
                conclusion: Statement::Equality {
                    left: one.clone(),
                    right: one.clone(),
                    domain: region,
                    domain_contract: DomainContract::DefinedAndEqual,
                },
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::None,
            }],
            &mut ledger()?,
        )?;
        Ok(Self {
            cas,
            context,
            one,
            pi,
            scalar,
            witness,
        })
    }

    /// Admit each graph-owned kind through its independent public or internal checking route.
    pub(super) fn retain_graph(&mut self, budget: &mut Budget) -> TestResult {
        Ok(retain_graph(
            &mut self.cas,
            &self.context,
            &self.one,
            &self.pi,
            &self.scalar,
            budget,
        )?)
    }
}

/// Run the same independent checkers inside an enclosing transaction control.
pub(super) fn retain_graph(
    cas: &mut Cas,
    context: &Handle,
    one: &Handle,
    pi: &Handle,
    scalar: &RationalEnclosure,
    budget: &mut Budget,
) -> Result<(), Fault> {
    cas.prepare_constant(context, one, budget)?;
    cas.check_rational_enclosure(scalar, budget)?;
    machin(cas, pi, budget)?;
    let report = cas.enclose(
        &EnclosureRequest {
            target: one.clone(),
            context: context.clone(),
            coordinates: Vec::new(),
            goal: EnclosureGoal::Width,
        },
        RefinementPolicy {
            version: 1,
            precision: 4,
            grid_ceiling: 64,
            rounds: 2,
            reduction_ceiling: 16,
            term_ceiling: 64,
            checking_work: 50_000,
        },
        budget,
        &mut |_| false,
    )?;
    assert!(report.checked.is_some());
    Ok(())
}

/// The scalar inventory has one exact rational record and its original scope.
fn scalar(one: &Handle, context: &Handle, region: &Handle) -> RationalEnclosure {
    RationalEnclosure {
        target: one.clone(),
        context: context.clone(),
        input_family: region.clone(),
        lower: Rational::one(),
        upper: Rational::one(),
        origin: EnclosureOrigin::ScalarKernel(Box::new(ScalarKernelCertificate {
            version: 1,
            expression: one.clone(),
            context: context.clone(),
            input_family: region.clone(),
            records: vec![ScalarKernelRecord {
                law: ScalarKernelLaw::Rational(Rational::one()),
                lower: Rational::one(),
                upper: Rational::one(),
            }],
        })),
    }
}

/// The checked finite Machin theorem uses the existing two/one-term certificate.
fn machin(cas: &mut Cas, pi: &Handle, budget: &mut Budget) -> Result<(), Fault> {
    use crate::decision_kernel::cache::{atomic, attempt, retained, synchronized, validate_pi};
    let certificate = PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [
            Rational::from_ratio(74, 375, budget)?,
            Rational::from_ratio(1, 239, budget)?,
        ],
        lower: Rational::from_ratio(128, 41, budget)?,
        upper: Rational::from_ratio(640, 203, budget)?,
    };
    attempt(cas, budget, |cas, budget| {
        synchronized(cas, budget, |budget| {
            atomic(cas, budget, |budget| {
                retained(cas, budget, |budget| {
                    validate_pi(cas, cas.checked(pi, budget)?, &certificate, budget).map(|_| ())
                })
            })
        })
    })
}
