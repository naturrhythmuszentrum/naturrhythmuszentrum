// SPDX-License-Identifier: AGPL-3.0-only
//! One complete ledger-aware plan prepares disjoint empty shells before the producing CAS moves.

use super::{
    FrozenLaw, LawIdentity, PreparedExpm1Owner, PreparedExpm1UseRecord, RetainedExpm1,
    RetainedExpm1Error, Seal,
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
        Cas,
        enclosure::prepared_expm1::{Counts, Policy, Request},
        limits::BudgetKind,
    },
};
use nrz_scalar::ArithmeticContext;
use std::rc::Rc;

/// Fixed accounting location for the complete combined staged check.
const AT: nrz_bindings::readings::algebra::Location =
    nrz_bindings::readings::algebra::Location::new("retained expm1 owner admission");
/// Exact vacant destinations and the single common permit for disjoint allocations.
pub(super) struct Plan {
    /// One or two debit destinations, with no temporary backing allocation.
    pub(super) slots: [usize; 2],
    /// Required debit backing capacity.
    capacity: usize,
    /// Fresh mutation successor, preserving pending producer headroom.
    pub(super) revision: u64,
    /// Only the common preflight issues this scope/account/epoch authorization.
    permit: ResourcePermit,
    /// Prepaid acquired-prefix wrapper and table cleanup.
    cleanup: CleanupReceipt,
}
impl Plan {
    /// Both admission paths use the identical complete staged check and common preflight.
    pub(super) fn new(
        heap: &Heap,
        account: u64,
        evidence: Option<usize>,
        receipt: u64,
        budget: &mut Budget,
    ) -> Result<Self, LoanError> {
        let count = 1 + usize::from(evidence.is_some());
        charge(budget, heap.loans.debits.len())?;
        let slots = heap.loans.vacant_inventory::<2>(count)?;
        let capacity = add(slots[count - 1], 1)?.max(heap.loans.debits.capacity());
        let (growth, old) =
            crate::loan::lifecycle::admission::growth(&heap.loans.debits, capacity)?;
        let owner = evidence.map_or(Ok(0), |bytes| add(rc_bytes::<PreparedExpm1Owner>(), bytes))?;
        let retained = add(add(owner, rc_bytes::<PreparedExpm1UseRecord>())?, growth)?;
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
        // Each prepared record, installed edge and eventual wrapper/edge release is paid once.
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
    /// Validate the complete permit before the first managed allocation or debit installation.
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
/// Prepared empty shared headers, created while the graph's accounting borrow is active.
pub(super) struct Shells {
    /// No weak witness exists before immutable pairing.
    owner: Rc<PreparedExpm1Owner>,
    /// No use is exposed before the graph is encapsulated.
    use_record: Rc<PreparedExpm1UseRecord>,
    /// Prechecked revision for the joint installation.
    revision: u64,
}
impl Heap {
    /// Prepare, lower and permanently encapsulate the uniquely owned producing CAS.
    ///
    /// Refusal returns that same CAS, preserving performed exact Work and any successfully
    /// prepared lemma. No supplied program, evidence, key or reading is admission authority.
    /// # Errors
    /// Returns exact/native preparation and complete managed resource admission causes.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor owns the CAS and preserves all original preparation inputs"
    )]
    #[expect(
        clippy::result_large_err,
        reason = "refusal returns the consumed CAS without allocating another owner on resource failure"
    )]
    pub fn prepare_retained_expm1(
        &mut self,
        mut cas: Cas,
        request: &Request,
        domain: [f64; 2],
        policy: Policy,
        context: &ArithmeticContext,
        counts: &mut Counts,
        account: &mut ExecutionAccount,
    ) -> Result<RetainedExpm1, (RetainedExpm1Error, Cas)> {
        let prepared = crate::prepared_expm1::owned_parts(
            &mut cas,
            request,
            domain,
            policy,
            context,
            &mut account.ledger,
            counts,
            0,
        );
        let parts = match prepared {
            Ok(parts) => parts,
            Err(error) => return Err((error.into(), cas)),
        };
        let seal = Seal {
            domain: domain.map(f64::to_bits),
            revisions: [1, 1, 1, context.revision()],
        };
        let shells = cas.with_retained_graph(&mut account.ledger, |budget| {
            budget.with_work_phase(crate::loan::FAMILY, "prepared law admission", |budget| {
                shells(self, account.identity, parts.evidence, budget, seal)
            })
        });
        let mut shells = match shells {
            Ok(shells) => shells,
            Err(error) => return Err((RetainedExpm1Error::Ownership(error), cas)),
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
            #[cfg(test)]
            _destroyed: super::tests::Destroyed,
        });
        use_record.owner = Some(Rc::clone(&shells.owner));
        install_owner(self, &shells.owner);
        super::retention::install_use(self, &shells.use_record, shells.owner.identity);
        self.loans.revision = shells.revision;
        Ok(RetainedExpm1 {
            record: shells.use_record,
        })
    }
}
/// Prepay the complete inventory and allocate both headers before consuming their graph.
pub(super) fn shells(
    heap: &mut Heap,
    account: u64,
    evidence: u64,
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
    let plan = Plan::new(heap, account, Some(evidence), receipt, budget)?;
    let (mut guard, slots, revision) = plan.allocate(heap, account)?;
    #[cfg(test)]
    crate::loan::fault::before()?;
    let mut owner = Rc::new(PreparedExpm1Owner {
        payload: None,
        identity,
        evidence,
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
    let mut use_record = Rc::new(PreparedExpm1UseRecord {
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
    let owner_shell = Rc::get_mut(&mut owner).ok_or(LoanError::WrongLoan)?;
    owner_shell.cleanup = CleanupReceipt {
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
/// Installation only moves prechecked records; no fallible work follows graph encapsulation.
fn install_owner(heap: &mut Heap, owner: &Rc<PreparedExpm1Owner>) {
    if owner.payload.is_some() {
        // The conversion and sum were admitted by Plan before either header allocation.
        let evidence = owner.evidence;
        heap.loans.install(
            owner.token.slot,
            Debit {
                kind: AllocationKind::PreparedOwner {
                    witness: Rc::downgrade(owner),
                    evidence,
                },
                owner: DebitOwner::Prepared(owner.identity),
                key: owner.token.key,
                elements: 0,
                bytes: rc_bytes::<PreparedExpm1Owner>() + evidence,
            },
        );
    }
}
