// SPDX-License-Identifier: AGPL-3.0-only
//! Every original required position participates in the full-coverage predicate.

use super::super::{
    native::Native,
    node::{Contract, State},
    owner::Owner,
};
use crate::loan::graph::walk::{Error, Handle};
use crate::{ExecutionAccount, Heap, LoanError};
use nrz_scalar::Scalar;

impl<P: Scalar + 'static, const N: usize> Owner<'_, P, N> {
    /// Refusal takes precedence over absence after every required producer has settled.
    pub(super) fn coverage(
        &self,
        handle: Handle,
        heap: &Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Option<State>, Error> {
        let node = &self.backing.nodes[handle.slot];
        match &node.contract {
            Contract::Input(_) => Err(Error::InvalidPhase),
            Contract::Mapped { source, .. } | Contract::Delivery { source } => {
                crate::loan::charge(&mut account.ledger, 1)?;
                disposition(self.backing.nodes[*source].state, *source)
            }
            Contract::Omega {
                sources, demand, ..
            } => {
                if !matches!(node.native, Some(Native::OmegaDeclaration(_))) {
                    return Ok(None);
                }
                let mut missing = false;
                let mut refused = None;
                for source in sources {
                    crate::loan::charge(&mut account.ledger, 1)?; // N original positional producer.
                    match disposition(self.backing.nodes[*source].state, *source)? {
                        Some(State::DependencyRefused(cause)) => {
                            if refused.is_none() {
                                refused = Some(cause);
                            }
                        }
                        Some(State::MissingInput) => missing = true,
                        Some(_) => return Err(Error::InvalidPhase),
                        None => {}
                    }
                }
                if let Some(cause) = refused {
                    return Ok(Some(State::DependencyRefused(cause)));
                }
                if missing {
                    return Ok(Some(State::MissingInput));
                }
                let tiles = members(&self.backing.nodes, sources)?;
                let readers = self.backing.inventory.readers(heap, &tiles, account)?;
                let demand = heap
                    .mask(*demand, demand.layout())
                    .map_err(LoanError::from)?;
                let mut requested = 0_usize;
                let mut intersection = 0_usize;
                for cell in 0..demand.axis().capacity() {
                    crate::loan::charge(&mut account.ledger, 1)?; // M original demand membership.
                    if !demand.contains(cell).map_err(LoanError::from)? {
                        continue;
                    }
                    requested += 1;
                    let mut filled = true;
                    for position in 0..N {
                        let reader = readers.reader(position).ok_or(LoanError::WrongLoan)?;
                        filled &= reader.contains(cell, account)?;
                    }
                    intersection += usize::from(filled);
                }
                Ok(if requested == intersection {
                    None
                } else if intersection == 0 {
                    Some(State::MissingInput)
                } else {
                    Some(State::UnsupportedPartial)
                })
            }
        }
    }
}

/// The fixed array borrows each original mapped owner without allocating or retaining another pin.
pub(super) fn members<'a, 'i, P: Scalar, const N: usize>(
    nodes: &'a [super::super::node::Node<'i, P, N>],
    sources: &[usize; N],
) -> Result<[&'a crate::LcShapeTile<'i, P>; N], LoanError> {
    let first = sources
        .first()
        .and_then(|index| nodes.get(*index))
        .and_then(|node| node.native.as_ref())
        .ok_or(LoanError::UnsupportedContract)?;
    let Native::MappedPublished(first) = first else {
        return Err(LoanError::WrongLoan);
    };
    let mut tiles = [first; N];
    for (target, source) in tiles.iter_mut().zip(sources) {
        let Some(Native::MappedPublished(tile)) =
            nodes.get(*source).and_then(|node| node.native.as_ref())
        else {
            return Err(LoanError::WrongLoan);
        };
        *target = tile;
    }
    Ok(tiles)
}

/// A previously returned source cause stays tied to its original producer position.
const fn disposition(state: State, source: usize) -> Result<Option<State>, Error> {
    Ok(match state {
        State::Published => None,
        State::Absent | State::Cancelled | State::MissingInput => Some(State::MissingInput),
        State::Refused
        | State::DependencyRefused(_)
        | State::UnsupportedPartial
        | State::AttemptsExhausted => Some(State::DependencyRefused(source)),
        State::Pending
        | State::Borrowed
        | State::Returned
        | State::Repair
        | State::ResourceWait
        | State::OwnershipWait => return Err(Error::InvalidPhase),
    })
}
