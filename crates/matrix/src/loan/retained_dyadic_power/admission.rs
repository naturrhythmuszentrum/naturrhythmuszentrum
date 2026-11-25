// SPDX-License-Identifier: AGPL-3.0-only
//! Complete ledger-aware admission allocates disjoint empty shells before moving the producing CAS.

use super::{
    FrozenLaw, LawIdentity, PreparedDyadicPowerOwner, PreparedDyadicPowerUseRecord,
    RetainedDyadicPower, RetainedDyadicPowerError, Seal,
};
use crate::{
    Heap,
    heap::resources::{ResourcePermit, ResourcePlan, ResourcePolicy, add},
    loan::{
        ExecutionAccount, LoanError, charge,
        preparation::Preparation,
        rc_bytes,
        registry::{AllocationKind, CleanupReceipt, Debit, DebitOwner, Token},
    },
};
use nrz_bindings::{
    Budget,
    readings::algebra::{
        Cas, Handle, Location,
        enclosure::RefinementPolicy,
        limits::{BudgetKind, BudgetUsage},
    },
};
use nrz_scalar::{ArithmeticContext, DyadicPowerLimits};
use std::rc::Rc;

/// Complete managed staged check uses the original ledger and CAS graph scope.
const AT: Location = Location::new("retained dyadic power owner admission");

/// Complete preflight for either an owner plus initial use or a further use.
pub(super) struct Plan {
    /// Preselected one or two vacant debit destinations.
    pub(super) slots: [usize; 2],
    /// Actual required debit backing capacity.
    capacity: usize,
    /// Checked mutation successor for the installation.
    pub(super) revision: u64,
    /// Common scope/account/epoch permit for disjoint allocations.
    permit: ResourcePermit,
    /// Complete acquired-prefix cleanup prepayment.
    cleanup: CleanupReceipt,
}

impl Plan {
    /// Admit one complete retained peak and the unchanged common resource preflight.
    pub(super) fn new(
        heap: &Heap,
        account: u64,
        evidence_and_keys: Option<(usize, usize)>,
        receipt: u64,
        budget: &mut Budget,
    ) -> Result<Self, LoanError> {
        let count = 1 + usize::from(evidence_and_keys.is_some());
        charge(budget, heap.loans.debits.len())?;
        let slots = heap.loans.vacant_inventory::<2>(count)?;
        let capacity = add(slots[count - 1], 1)?.max(heap.loans.debits.capacity());
        let (growth, old) =
            crate::loan::lifecycle::admission::growth(&heap.loans.debits, capacity)?;
        let owner = evidence_and_keys.map_or(Ok(0), |(evidence, keys)| {
            add(add(rc_bytes::<PreparedDyadicPowerOwner>(), evidence)?, keys)
        })?;
        let retained = add(
            add(owner, rc_bytes::<PreparedDyadicPowerUseRecord>())?,
            growth,
        )?;
        let scratch = old.checked_mul(2).ok_or(LoanError::ByteOverflow)?;
        let staged = add(retained, scratch)?;
        let revision = heap.loans.next_revision()?;
        charge(budget, heap.resource_work()?)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            u64::try_from(staged).map_err(|_| LoanError::ByteOverflow)?,
            AT,
        )?;
        let mut permit = heap.preflight_resources(
            ResourcePlan {
                retained,
                scratch,
                ..ResourcePlan::default()
            },
            ResourcePolicy::from_budget(budget),
        )?;
        permit.account = Some(account);
        let cleanup = CleanupReceipt {
            identity: receipt,
            allocations: count + usize::from(growth != 0),
            records: count,
            edges: 1,
        };
        charge(
            budget,
            cleanup.allocations + 2 * cleanup.records + 2 * cleanup.edges,
        )?;
        Ok(Self {
            slots,
            capacity,
            revision,
            permit,
            cleanup,
        })
    }

    /// Validate the one permit before table growth or any owner/use shell allocation.
    pub(super) fn allocate(
        self,
        heap: &mut Heap,
        account: u64,
    ) -> Result<(Preparation, [usize; 2], u64), LoanError> {
        self.permit.storage(heap.storage.epoch())?;
        if self.permit.scope != heap.scope || self.permit.account != Some(account) {
            return Err(LoanError::ForeignOwner);
        }
        let mut guard = Preparation::new(self.cleanup);
        crate::loan::lifecycle::admission::grow_tracked(
            &mut heap.loans.debits,
            self.capacity,
            &mut guard,
        )?;
        Ok((guard, self.slots, self.revision))
    }
}

/// Private empty headers exist only during the charged graph-scoped allocation phase.
pub(super) struct Shells {
    /// Shared owner not yet carrying its graph.
    owner: Rc<PreparedDyadicPowerOwner>,
    /// Initial use not yet carrying its strong edge.
    use_record: Rc<PreparedDyadicPowerUseRecord>,
    /// Checked joint debit installation revision.
    revision: u64,
}

impl Heap {
    /// Prepare and permanently encapsulate the original checked power and its producing CAS.
    /// # Errors
    /// Returns the same CAS and the performed exact prefix on preparation or ownership refusal.
    #[expect(
        clippy::too_many_arguments,
        reason = "the original graph, policy and cumulative ledger remain explicit"
    )]
    #[expect(
        clippy::result_large_err,
        reason = "refusal returns the consumed producing CAS"
    )]
    pub fn prepare_retained_dyadic_power(
        &mut self,
        mut cas: Cas,
        target: &Handle,
        context: &Handle,
        domain: [f64; 2],
        policy: RefinementPolicy,
        limits: DyadicPowerLimits,
        arithmetic: &ArithmeticContext,
        interrupt: &mut impl FnMut(BudgetUsage) -> bool,
        account: &mut ExecutionAccount,
    ) -> Result<RetainedDyadicPower, (RetainedDyadicPowerError, Cas)> {
        let span = tracing::trace_span!(
            "nrz.prepare",
            family = "dyadic-power",
            result = tracing::field::Empty,
            bindings_work = tracing::field::Empty,
            owner_bytes = tracing::field::Empty,
            evidence_bytes = tracing::field::Empty,
            key_bytes = tracing::field::Empty,
            use_bytes = tracing::field::Empty,
            table_capacity = tracing::field::Empty
        );
        let _entered = span.enter();
        let mut work_used = 0;
        let prepared = crate::prepared_dyadic_power::owned_parts(
            &mut cas,
            target,
            context,
            domain,
            policy,
            limits,
            arithmetic,
            &mut account.ledger,
            interrupt,
            &mut work_used,
        );
        span.record("bindings_work", work_used);
        let parts = match prepared {
            Ok(parts) => parts,
            Err(error) => {
                span.record("result", "preparation-refused");
                return Err((error.into(), cas));
            }
        };
        let keys = match parts.key_bytes() {
            Ok(bytes) => bytes,
            Err(error) => {
                span.record("result", "key-inventory-refused");
                return Err((error.into(), cas));
            }
        };
        let seal = Seal {
            domain: domain.map(f64::to_bits),
            exponent: parts.exponent(),
            revisions: [parts.argument_revision(), arithmetic.revision(), 0],
        };
        let shells = cas.with_retained_graph(&mut account.ledger, |budget| {
            budget.with_work_phase(
                crate::loan::FAMILY,
                "dyadic power law admission",
                |budget| {
                    shells(
                        self,
                        account.identity,
                        parts.authority.retained_bytes(),
                        keys,
                        budget,
                        seal,
                    )
                },
            )
        });
        let mut shells = match shells {
            Ok(shells) => shells,
            Err(error) => {
                span.record("result", "ownership-refused");
                return Err((RetainedDyadicPowerError::Ownership(error), cas));
            }
        };
        let Some(owner) = Rc::get_mut(&mut shells.owner) else {
            return Err((LoanError::WrongLoan.into(), cas));
        };
        let Some(use_record) = Rc::get_mut(&mut shells.use_record) else {
            return Err((LoanError::WrongLoan.into(), cas));
        };
        let graph_bytes = cas.counts().payload_bytes;
        owner.payload = Some(FrozenLaw {
            parts,
            seal,
            graph_bytes,
            graph: cas,
        });
        use_record.owner = Some(Rc::clone(&shells.owner));
        install_owner(self, &shells.owner);
        super::retention::install_use(self, &shells.use_record, shells.owner.identity);
        self.loans.revision = shells.revision;
        span.record("owner_bytes", rc_bytes::<PreparedDyadicPowerOwner>());
        span.record("evidence_bytes", shells.owner.evidence);
        span.record("key_bytes", shells.owner.keys);
        span.record("use_bytes", rc_bytes::<PreparedDyadicPowerUseRecord>());
        span.record("table_capacity", self.loans.debits.capacity());
        span.record("result", "accepted");
        Ok(RetainedDyadicPower {
            record: shells.use_record,
        })
    }
}

/// Allocate both prepaid headers before moving authority or producing graph.
pub(super) fn shells(
    heap: &mut Heap,
    account: u64,
    evidence: u64,
    keys: usize,
    budget: &mut Budget,
    seal: Seal,
) -> Result<Shells, LoanError> {
    charge(budget, 1)?;
    let identity = LawIdentity {
        scope: heap.scope,
        account,
        owner: super::identity()?,
    };
    let use_identity = super::identity()?;
    let receipt = super::identity()?;
    let evidence = usize::try_from(evidence).map_err(|_| LoanError::ByteOverflow)?;
    let plan = Plan::new(heap, account, Some((evidence, keys)), receipt, budget)?;
    let (mut guard, slots, revision) = plan.allocate(heap, account)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mut owner = Rc::new(PreparedDyadicPowerOwner {
        payload: None,
        identity,
        evidence,
        keys,
        token: Token {
            slot: slots[0],
            key: identity.owner,
            receipt,
        },
        cleanup: CleanupReceipt::empty(receipt),
    });
    guard.acquired(false)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mut use_record = Rc::new(PreparedDyadicPowerUseRecord {
        owner: None,
        binding: super::Binding {
            identity,
            use_identity,
            seal,
        },
        token: Token {
            slot: slots[1],
            key: use_identity,
            receipt,
        },
        cleanup: CleanupReceipt::empty(receipt),
    });
    guard.acquired(false)?;
    let mut cleanup = guard.finish()?;
    Rc::get_mut(&mut owner).ok_or(LoanError::WrongLoan)?.cleanup = CleanupReceipt {
        identity: receipt,
        allocations: 1,
        records: 1,
        edges: 0,
    };
    cleanup.allocations -= 1;
    cleanup.records -= 1;
    Rc::get_mut(&mut use_record)
        .ok_or(LoanError::WrongLoan)?
        .cleanup = cleanup;
    Ok(Shells {
        owner,
        use_record,
        revision,
    })
}

/// Install only the completed owner with the CAS-owned E and separate key buffers.
fn install_owner(heap: &mut Heap, owner: &Rc<PreparedDyadicPowerOwner>) {
    if owner.payload.is_some() {
        heap.loans.install(
            owner.token.slot,
            Debit {
                kind: AllocationKind::DyadicOwner {
                    witness: Rc::downgrade(owner),
                    allowance: owner.evidence + owner.keys,
                },
                owner: DebitOwner::Dyadic(owner.identity),
                key: owner.token.key,
                elements: 0,
                bytes: rc_bytes::<PreparedDyadicPowerOwner>() + owner.evidence + owner.keys,
            },
        );
    }
}
