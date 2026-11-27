// SPDX-License-Identifier: AGPL-3.0-only
//! Private correspondence and allowance checks before the funded application has a bundle slot.

use super::ApplicationError;
use crate::loan::retained_dyadic_power::Binding;
use crate::{
    BundlePin, CellAxis, CheckedOperation, ExecutionAccount, ExecutionLimits, Heap,
    HostTileDeclaration, LoanError, MaskVersion, RegionContract, RetainedDyadicPower,
};
use nrz_bindings::{BindingSymbols, Budget, readings::algebra::Handle};
use std::rc::Rc;

/// Original dyadic power authority instantiated over matching declared ratio components.
#[derive(Clone, Debug)]
pub struct PreparedDyadicPowerOperation {
    /// Checked original region inventory and closed numerical recipe.
    pub(super) operation: CheckedOperation,
    /// Exact original value symbols.
    pub(super) source: BindingSymbols,
    /// Independent result and its two companions.
    pub(super) destination: BindingSymbols,
    /// Original cell association.
    pub(super) axis: CellAxis,
    /// Complete immutable owner and funded-use binding.
    pub(super) binding: Binding,
}

impl PreparedDyadicPowerOperation {
    /// Bind the checked retained use to the actual original statement and regions.
    /// # Errors
    /// Refuses substituted authority, unsupported geometry, foreign accounts and Work limits.
    #[expect(
        clippy::too_many_arguments,
        reason = "the explicit original correspondence is checked together"
    )]
    pub fn check(
        law: &RetainedDyadicPower,
        target: &Handle,
        context: &Handle,
        argument: &Handle,
        domain: [f64; 2],
        environment: u32,
        input: RegionContract,
        output: RegionContract,
        source: BindingSymbols,
        destination: BindingSymbols,
        axis: CellAxis,
        account: &mut ExecutionAccount,
    ) -> Result<Self, ApplicationError> {
        law.authenticate_account(account)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "dyadic power correspondence", |budget| {
                let binding =
                    check_original(law, target, context, argument, domain, environment, budget)?;
                check_regions_and_symbols(&input, &output, &source, &destination, axis, budget)?;
                let inventory = binding.schedule_inventory()?;
                if inventory != law.schedule_inventory()? {
                    return Err(LoanError::WrongLoan.into());
                }
                let extent = input.shape().extent();
                let (work, _, _) = allowances(inventory, extent.rows(), extent.columns())?;
                let numerical = crate::kernels::KernelContract {
                    inputs: Rc::new([extent]),
                    output: extent,
                    recipe: crate::kernels::contract::Recipe::PreparedDyadicPower,
                    work,
                };
                Ok(Self {
                    operation: CheckedOperation {
                        inputs: Rc::new([input]),
                        outputs: Rc::new([output]),
                        revision: 1,
                        recipe: crate::binding::NativeRecipe::Kernel(Rc::new(numerical)),
                    },
                    source,
                    destination,
                    axis,
                    binding,
                })
            })
    }

    /// Compare exact source and result declarations with the captured original symbols.
    pub(super) fn validate_destination(
        &self,
        source: &crate::loan::registry::Declaration,
        symbols: &BindingSymbols,
        axis: CellAxis,
        coverage: crate::Coverage,
    ) -> Result<(), LoanError> {
        if !super::prepared::same_symbols(&self.source, &source.symbols)
            || !super::prepared::same_symbols(&self.destination, symbols)
            || self.axis != source.axis
            || self.axis != axis
        {
            return Err(LoanError::WrongLoan);
        }
        if source.coverage != crate::Coverage::RepresentedInputs
            || coverage != crate::Coverage::RepresentedInputs
        {
            return Err(LoanError::CoverageMismatch);
        }
        Ok(())
    }
}

/// Complete dyadic application request preserved unchanged on refusal.
#[derive(Debug)]
pub struct PointwiseDyadicPowerRequest {
    /// Checked original component correspondence.
    pub operation: PreparedDyadicPowerOperation,
    /// Exact already funded use captured by that operation.
    pub law: RetainedDyadicPower,
    /// Current represented center-only source.
    pub input: BundlePin<f64>,
    /// Fresh center, total and error destination.
    pub destination: HostTileDeclaration<f64>,
    /// Original ordinary demand.
    pub demand: MaskVersion,
    /// Complete invocation and five-reading ceilings.
    pub limits: ExecutionLimits,
}

/// Admission either transfers the complete request or returns it with its typed refusal.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "the original request is returned without a replacement allocation"
)]
pub enum DyadicPowerApplicationRequest {
    /// Detached ready owner of the common application protocol.
    ApplicationReady(super::OwnedApplication<f64>),
    /// Original request and its exact funded use and pin.
    ApplicationRefused(ApplicationError, PointwiseDyadicPowerRequest),
}

impl Heap {
    /// Admit the paired retained dyadic law through the common masked mechanism.
    pub fn request_pointwise_dyadic_power(
        &mut self,
        request: PointwiseDyadicPowerRequest,
        account: &mut ExecutionAccount,
    ) -> DyadicPowerApplicationRequest {
        super::admission::request_dyadic_power(self, request, account)
    }
}

/// Borrow the exact original statement and immutable use seal without rereading its CAS.
fn check_original(
    law: &RetainedDyadicPower,
    target: &Handle,
    context: &Handle,
    argument: &Handle,
    domain: [f64; 2],
    environment: u32,
    budget: &mut Budget,
) -> Result<Binding, ApplicationError> {
    crate::loan::charge(budget, 1)?;
    let request = law.authority()?.judgment().request();
    let [coordinate] = request.coordinates.as_slice() else {
        return Err(LoanError::UnsupportedContract.into());
    };
    let binding = Binding::of(law)?;
    if target != &request.target
        || context != &request.context
        || argument != &coordinate.coordinate
        || domain.map(f64::to_bits) != law.domain()?.map(f64::to_bits)
        || environment != binding.arithmetic_revision()
    {
        return Err(LoanError::WrongLoan.into());
    }
    Ok(binding)
}

/// Reuse the prepared member's original region and symbol validators in their order.
fn check_regions_and_symbols(
    input: &RegionContract,
    output: &RegionContract,
    source: &BindingSymbols,
    destination: &BindingSymbols,
    axis: CellAxis,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
    super::prepared::regions(input, output, axis, budget)?;
    crate::loan::charge(budget, 4)?;
    super::prepared::symbols(source, destination, input.shape().extent())?;
    Ok(())
}

/// Authenticate account, live owner/use debits and the operation's captured binding.
pub(super) fn check_current(
    heap: &Heap,
    law: &RetainedDyadicPower,
    expected: Binding,
    identity: u64,
) -> Result<(), ApplicationError> {
    law.authenticate_identity_account(identity)?;
    law.validate_current(heap)?;
    if Binding::of(law)? != expected {
        return Err(LoanError::WrongLoan.into());
    }
    Ok(())
}

/// Reserve one validation, all roots and products, and five midpoint/displacement stages.
pub(super) fn invocation_units(inventory: [usize; 3]) -> Result<usize, LoanError> {
    inventory
        .into_iter()
        .try_fold(6_usize, usize::checked_add)
        .ok_or(LoanError::CountOverflow)
}

/// Independently bound the original array and one selected cell at the admitted schedule.
fn allowances(
    inventory: [usize; 3],
    cells: usize,
    components: usize,
) -> Result<(usize, usize, usize), LoanError> {
    let units = invocation_units(inventory)?;
    let length = cells
        .checked_mul(components)
        .ok_or(LoanError::CountOverflow)?;
    let original = units.checked_mul(length).ok_or(LoanError::CountOverflow)?;
    let one_cell = units
        .checked_mul(components)
        .ok_or(LoanError::CountOverflow)?;
    let payload = 5_usize
        .checked_mul(length)
        .ok_or(LoanError::CountOverflow)?;
    Ok((original, one_cell, one_cell.max(payload)))
}

#[cfg(test)]
mod tests;
