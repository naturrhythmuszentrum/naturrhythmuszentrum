// SPDX-License-Identifier: AGPL-3.0-only
//! Original ordered correspondence and the sole additional binary owner.

mod correspondence;
mod validation;
pub(super) use validation::check_request;

use super::{ApplicationError, Evidence};
use crate::loan::registry::{Declaration, Identity, QualifiedMaskVersion, Token};
use crate::{
    BinaryElementwise, BundlePin, CellAxis, CheckedOperation, ExecutionAccount, ExecutionLimits,
    Heap, HostTileDeclaration, LoanError, MaskVersion, RegionContract,
};
use nrz_bindings::BindingSymbols;
use std::rc::Rc;

/// Original checked ordered product or quotient; symbols and occupations cannot be substituted.
#[derive(Clone, Debug)]
pub struct PointwiseBinaryOperation {
    /// Original contract allocation and native law identities.
    pub(super) operation: CheckedOperation,
    /// Original ordered source symbol inventories.
    sources: [BindingSymbols; 2],
    /// Full original source occupations in operand order.
    identities: [Identity; 2],
    /// Independently declared result convention.
    destination: BindingSymbols,
    /// Original logical cell association.
    axis: CellAxis,
}
impl PointwiseBinaryOperation {
    /// Check original regions against the two borrowed host pins and a fresh destination.
    ///
    /// Each source's exact symbols and occupation are captured from its borrowed pin.
    /// The caller's account funds all correspondence visits; no numerical edge is acquired.
    /// # Errors
    /// Refuses aliases, incoherent original coordinates, unsupported meanings and exhausted limits.
    #[expect(
        clippy::too_many_arguments,
        reason = "the original ordered correspondence is checked together"
    )]
    pub fn check(
        inputs: [RegionContract; 2],
        output: RegionContract,
        sources: [&BundlePin<f64>; 2],
        destination: BindingSymbols,
        axis: CellAxis,
        function: BinaryElementwise,
        limits: ExecutionLimits,
        account: &mut ExecutionAccount,
    ) -> Result<Self, ApplicationError> {
        account
            .ledger
            .with_work_phase(super::FAMILY, "binary correspondence", |budget| {
                crate::loan::charge(budget, 1)?;
                let bundles = [bundle(sources[0])?, bundle(sources[1])?];
                for (source, pin) in bundles.into_iter().zip(sources) {
                    if pin
                        .record
                        .payload
                        .as_ref()
                        .ok_or(LoanError::WrongLoan)?
                        .account
                        != account.identity
                    {
                        return Err(LoanError::ForeignAccount.into());
                    }
                    if source.origin.evidence.is_some() {
                        return Err(LoanError::UnsupportedContract.into());
                    }
                }
                correspondence::check(
                    &inputs,
                    &output,
                    bundles,
                    &destination,
                    axis,
                    function,
                    budget,
                )?;
                let extent = output.shape().extent();
                let work = crate::kernels::contract::work(extent.entries(), 21)?;
                limits.check(2, work)?;
                correspondence::preflight(budget)?;
                let law = crate::kernels::KernelContract {
                    inputs: Rc::new([extent; 2]),
                    output: extent,
                    recipe: crate::kernels::contract::Recipe::BinaryElementwise(function),
                    work,
                };
                Ok(Self {
                    operation: CheckedOperation {
                        inputs: Rc::new(inputs),
                        outputs: Rc::new([output]),
                        revision: 1,
                        recipe: crate::binding::NativeRecipe::Kernel(Rc::new(law)),
                    },
                    sources: bundles.map(|source| source.declaration.symbols.clone()),
                    identities: bundles.map(|source| source.identity),
                    destination,
                    axis,
                })
            })
    }
}

/// Both owning pins return unchanged if admission refuses.
#[derive(Debug)]
pub struct PointwiseBinaryRequest {
    /// Checked original operation and exact ordered captures.
    pub operation: PointwiseBinaryOperation,
    /// Left source pin, matching the first original operand.
    pub left: BundlePin<f64>,
    /// Right source pin, matching the second original operand.
    pub right: BundlePin<f64>,
    /// Fresh three-reading destination on the original cell axis.
    pub destination: HostTileDeclaration<f64>,
    /// Ordinary demand on the exact left layout occupation.
    pub demand: MaskVersion,
    /// Two-slot original application invocation and payload ceilings.
    pub limits: ExecutionLimits,
}

/// Binary admission returns the existing detached owner or the entire original request.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "refusal preserves both original pins without another allocation"
)]
pub enum BinaryApplicationRequest {
    /// Common detached evaluation and publication capability.
    ApplicationReady(super::OwnedApplication<f64>),
    /// Original request and both still-owning pins.
    ApplicationRefused(ApplicationError, PointwiseBinaryRequest),
}
impl Heap {
    /// Apply cartesian product through the common masked application mechanism.
    pub fn request_pointwise_product(
        &mut self,
        request: PointwiseBinaryRequest,
        account: &mut ExecutionAccount,
    ) -> BinaryApplicationRequest {
        super::admission::request_binary(self, request, BinaryElementwise::Product, account)
    }
    /// Apply direct ordered quotient through the common masked application mechanism.
    pub fn request_pointwise_quotient(
        &mut self,
        request: PointwiseBinaryRequest,
        account: &mut ExecutionAccount,
    ) -> BinaryApplicationRequest {
        super::admission::request_binary(self, request, BinaryElementwise::Quotient, account)
    }
}

/// Closed alternative uses the existing endpoint-pair payload width for every unary owner.
#[derive(Debug)]
pub(in crate::loan) enum Auxiliary {
    /// Ordinary unary application has no additional owner.
    Absent,
    /// Prepared member's original ordered endpoint tokens.
    Endpoints([(usize, u64, u64); 2]),
    /// Binary correspondence and the right source edge.
    Binary(Box<Side>),
}
impl Auxiliary {
    /// Borrow the binary owner without creating another edge or authority.
    pub(in crate::loan) const fn binary(&self) -> Option<&Side> {
        match self {
            Self::Binary(side) => Some(side),
            Self::Absent | Self::Endpoints(_) => None,
        }
    }
    /// Prepared endpoint authentication excludes the mutually exclusive binary owner.
    pub(super) const fn endpoints(&self) -> Option<[(usize, u64, u64); 2]> {
        match self {
            Self::Endpoints(tokens) => Some(*tokens),
            Self::Absent | Self::Binary(_) => None,
        }
    }
    /// Extract the transferred right edge only at the final no-fail publication boundary.
    pub(super) fn take_pin(&mut self) -> Option<BundlePin<f64>> {
        match self {
            Self::Binary(side) => side.input.take(),
            Self::Absent | Self::Endpoints(_) => None,
        }
    }
}

/// One inventoried box survives publication with its right-pin slot empty.
#[derive(Debug)]
pub(in crate::loan) struct Side {
    /// Original checked correspondence, distinct from the captured source witnesses.
    pub(super) original: PointwiseBinaryOperation,
    /// Captured immutable right declaration.
    pub(super) source: Declaration,
    /// Full captured right occupation.
    pub(super) identity: Identity,
    /// Independently authenticated right filled roles.
    pub(super) attachments: [Option<QualifiedMaskVersion>; 3],
    /// Transferred right edge, empty in every immutable published seal.
    pub(super) input: Option<BundlePin<f64>>,
    /// Sole allocation debit for this box.
    pub(in crate::loan) token: Token,
}

/// Borrowing a source cannot manufacture another pin.
pub(super) fn bundle(pin: &BundlePin<f64>) -> Result<&crate::loan::registry::Bundle, LoanError> {
    pin.record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or(LoanError::WrongLoan)
}

/// Binary structural refusal distinguishes original operand scopes without changing unary errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryRegion {
    /// First original input.
    Left,
    /// Second original input.
    Right,
    /// Independently declared output.
    Output,
}

/// Exact source and destination handles, including optional companions.
fn same_symbols(left: &BindingSymbols, right: &BindingSymbols) -> bool {
    let value = left.value() == right.value();
    let total = left.bound() == right.bound();
    let error = left.error() == right.error();
    value && total && error
}
