// SPDX-License-Identifier: AGPL-3.0-only
//! Closed substitution of the retained scalar parameter into original declared components.

use super::{ApplicationError, PointwiseRequest};
use crate::loan::retained_expm1::{Attachment, Binding};
use crate::{
    BundlePin, CellAxis, CheckedOperation, Coverage, ExecutionAccount, ExecutionLimits,
    HostTileDeclaration, LoanError, MaskVersion, RegionContract, RetainedExpm1,
};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, readings::algebra::Handle};
use std::rc::Rc;

/// Original scalar request instantiated only over matching declared ratio components.
#[derive(Clone, Debug)]
pub struct PreparedExpm1Operation {
    /// Original region witnesses and the zero-allowance closed recipe.
    pub(super) operation: CheckedOperation,
    /// Exact source symbols; equal descriptors do not establish correspondence.
    pub(super) source: BindingSymbols,
    /// Independently declared result and its two companions.
    pub(super) destination: BindingSymbols,
    /// Original cell association, independent of equal capacity.
    pub(super) axis: CellAxis,
    /// Exact funded use, including the owner's admitted domain and revisions.
    pub(super) binding: Binding,
    /// Native invocation target established against the actual authority.
    pub(super) bits: u32,
}
impl PreparedExpm1Operation {
    /// Instantiate the owner's original unary request over two checked assembly regions.
    ///
    /// The supplied target, context and argument must be the actual authority's handles.
    /// Region witnesses can be obtained from a checked copy's input and output contracts.
    /// This constructor admits no arbitrary expression correspondence or numerical allocation.
    /// # Errors
    /// Refuses substituted authority, unsupported schemas/maps, foreign accounts and Work limits.
    #[expect(
        clippy::too_many_arguments,
        reason = "the explicit original correspondence is checked together"
    )]
    pub fn check(
        law: &RetainedExpm1,
        target: &Handle,
        context: &Handle,
        argument: &Handle,
        domain: [f64; 2],
        bits: u32,
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
            .with_work_phase(super::FAMILY, "prepared correspondence", |budget| {
                crate::loan::charge(budget, 1)?;
                let request = law.authority()?.request();
                let [coordinate] = request.original.coordinates.as_slice() else {
                    return Err(LoanError::UnsupportedContract.into());
                };
                if target != &request.original.target
                    || context != &request.original.context
                    || argument != &coordinate.coordinate
                    || bits != request.bits
                    || environment != request.environment
                    || environment != 1
                    || domain.map(f64::to_bits) != law.domain()?.map(f64::to_bits)
                {
                    return Err(LoanError::WrongLoan.into());
                }
                regions(&input, &output, axis, budget)?;
                crate::loan::charge(budget, 4)?;
                symbols(&source, &destination, input.shape().extent())?;
                ExecutionLimits::new(1, 0, 0).check(1, 0)?;
                let extent = input.shape().extent();
                let numerical = crate::kernels::KernelContract {
                    inputs: Rc::new([extent]),
                    output: extent,
                    recipe: crate::kernels::contract::Recipe::PreparedExpm1,
                    work: 0,
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
                    binding: Binding::of(law)?,
                    bits,
                })
            })
    }
    /// Fixed comparisons preserve the original substitution without revisiting shape entries.
    pub(super) fn validate(
        &self,
        source: &crate::loan::registry::Declaration,
        destination: &crate::loan::registry::Declaration,
    ) -> Result<(), LoanError> {
        self.validate_destination(
            source,
            &destination.symbols,
            destination.axis,
            destination.coverage,
        )
    }
    /// The same declaration pairing is checked before any output owner exists.
    pub(super) fn validate_destination(
        &self,
        source: &crate::loan::registry::Declaration,
        symbols: &BindingSymbols,
        axis: CellAxis,
        coverage: Coverage,
    ) -> Result<(), LoanError> {
        let source_matches = same_symbols(&self.source, &source.symbols);
        let destination_matches = same_symbols(&self.destination, symbols);
        if !source_matches || !destination_matches || self.axis != source.axis || self.axis != axis
        {
            return Err(LoanError::WrongLoan);
        }
        if source.coverage != Coverage::RepresentedInputs || coverage != Coverage::RepresentedInputs
        {
            return Err(LoanError::CoverageMismatch);
        }
        Ok(())
    }
}
/// Complete request returned unchanged on any admission refusal.
#[derive(Debug)]
pub struct PointwiseExpm1Request {
    /// Checked original parameter-to-component substitution.
    pub operation: PreparedExpm1Operation,
    /// Exact already funded law use captured by the operation.
    pub law: RetainedExpm1,
    /// Current center-only represented host input.
    pub input: BundlePin<f64>,
    /// Fresh center, total and error declaration on the same original axis.
    pub destination: HostTileDeclaration<f64>,
    /// Demand on the source's exact native layout.
    pub demand: MaskVersion,
    /// Complete five-reading payload ceilings; native invocation allowance is zero.
    pub limits: ExecutionLimits,
}
/// Admission preserves the complete prepared request and both original retaining edges.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "a refusal returns the original request without an allocation"
)]
pub enum Expm1ApplicationRequest {
    /// Detached owner uses the common evaluation/publication/cancellation protocol.
    ApplicationReady(super::OwnedApplication<f64>),
    /// Original complete request, including its exact funded use and source pin.
    ApplicationRefused(ApplicationError, PointwiseExpm1Request),
}
impl crate::Heap {
    /// Apply the paired retained law through the common masked application mechanism.
    pub fn request_pointwise_expm1(
        &mut self,
        request: PointwiseExpm1Request,
        account: &mut ExecutionAccount,
    ) -> Expm1ApplicationRequest {
        super::admission::request_expm1(self, request, account)
    }
}
/// Borrowing the request retains every recoverable edge until private preparation completes.
pub(super) struct Request<'a, P: nrz_scalar::Scalar> {
    /// Original checked region inventory.
    pub(super) operation: &'a CheckedOperation,
    /// Source retaining edge.
    pub(super) input: &'a BundlePin<P>,
    /// Fresh destination declaration.
    pub(super) destination: &'a HostTileDeclaration<P>,
    /// Captured ordinary demand.
    pub(super) demand: MaskVersion,
    /// Complete payload ceilings.
    pub(super) limits: ExecutionLimits,
    /// Expected entry point, checked before the member profile is used.
    pub(super) expected: crate::KernelKind,
    /// Only the projected entry point admits a projected unary recipe.
    pub(super) projected_entry: bool,
    /// Optional ordered binary correspondence and borrowed right edge.
    pub(super) binary: Option<(&'a super::PointwiseBinaryOperation, &'a BundlePin<f64>)>,
    /// Optional retained correspondence for the prepared member.
    pub(super) prepared: Option<&'a PreparedExpm1Operation>,
}
impl<P: nrz_scalar::Scalar> Request<'_, P> {
    /// Both role inventories participate in initialization and symbolic cleanup.
    pub(super) fn input_channels(&self) -> Result<usize, LoanError> {
        let left = super::contract::source(self)?
            .channels
            .iter()
            .flatten()
            .count();
        let right = self
            .binary
            .map(|(_, pin)| super::binary::bundle(pin))
            .transpose()?
            .map_or(0, |bundle| bundle.channels.iter().flatten().count());
        Ok(left + right)
    }
    /// Borrow a complete closed profile only after authenticating the requested entry point.
    pub(super) fn member(&self) -> Result<super::member::Member<'_>, LoanError> {
        super::member::Member::check(self.operation, self.expected, self.prepared)
    }
}
impl<'a, P: nrz_scalar::Scalar> From<&'a PointwiseRequest<P>> for Request<'a, P> {
    fn from(request: &'a PointwiseRequest<P>) -> Self {
        Self {
            operation: &request.operation,
            input: &request.input,
            destination: &request.destination,
            demand: request.demand,
            limits: request.limits,
            projected_entry: false,
            binary: None,
            prepared: None,
            expected: crate::KernelKind::Elementwise(crate::Elementwise::Square),
        }
    }
}
impl<'a> From<&'a PointwiseExpm1Request> for Request<'a, f64> {
    fn from(request: &'a PointwiseExpm1Request) -> Self {
        Self {
            operation: &request.operation.operation,
            input: &request.input,
            destination: &request.destination,
            demand: request.demand,
            limits: request.limits,
            projected_entry: false,
            binary: None,
            prepared: Some(&request.operation),
            expected: crate::KernelKind::PreparedExpm1,
        }
    }
}
/// Exact value and companion handles, not shape or statement equality.
fn same_symbols(left: &BindingSymbols, right: &BindingSymbols) -> bool {
    let value = left.value() == right.value();
    let total = left.bound() == right.bound();
    let error = left.error() == right.error();
    value && total && error
}
/// Two assembly witnesses share one original declaration snapshot and identity coordinates.
fn regions(
    input: &RegionContract,
    output: &RegionContract,
    axis: CellAxis,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
    crate::loan::charge(budget, 2)?;
    let extent = input.shape().extent();
    if extent != output.shape().extent()
        || extent.rows() != axis.capacity()
        || extent.columns() == 0
        || input.shape().declarations().id() != output.shape().declarations().id()
        || input.shape().domain() != output.shape().domain()
        || input.shape().codomain() != output.shape().codomain()
    {
        return Err(LoanError::LayoutMismatch.into());
    }
    for region in [input, output] {
        if region.descriptor() != &Descriptor::ratio()
            || region.shape().recipe() != crate::ShapeRecipe::Assembly
        {
            return Err(LoanError::UnsupportedContract.into());
        }
        for map in [region.domain_coordinates(), region.codomain_coordinates()] {
            for conversion in map.normalization().iter().chain(map.restoration()) {
                crate::loan::charge(budget, 1)?;
                if !identity(conversion) {
                    return Err(LoanError::UnsupportedContract.into());
                }
            }
        }
        for entry in region.shape().entries() {
            crate::loan::charge(budget, 1)?;
            let declared = entry.original().is_none() && entry.checked_expression().is_none();
            if !declared
                || entry.scalar_descriptor() != Some(&Descriptor::ratio())
                || !entry.conversion().is_some_and(identity)
            {
                return Err(LoanError::UnsupportedContract.into());
            }
        }
    }
    Ok(())
}
/// Exact unit conversion is inspected without running any exact arithmetic.
fn identity(conversion: &nrz_bindings::Conversion) -> bool {
    let scale = conversion
        .scale
        .as_rational()
        .is_some_and(|scale| scale == &nrz_bindings::readings::algebra::Rational::one());
    let shift = conversion
        .shift
        .as_rational()
        .is_some_and(nrz_bindings::readings::algebra::Rational::is_zero);
    scale && shift
}
/// Validate independent import symbols and their complete companion relations once.
fn symbols(
    source: &BindingSymbols,
    destination: &BindingSymbols,
    extent: crate::Extent,
) -> Result<(), LoanError> {
    source
        .check_import()
        .map_err(|_| LoanError::StaleDependency)?;
    destination
        .check_import()
        .map_err(|_| LoanError::StaleDependency)?;
    let shape = [
        u32::try_from(extent.rows()).map_err(|_| LoanError::CountOverflow)?,
        u32::try_from(extent.columns()).map_err(|_| LoanError::CountOverflow)?,
    ];
    if source.value().descriptor() != &Descriptor::ratio()
        || destination.value().descriptor() != &Descriptor::ratio()
        || source.value().shape() != shape
        || destination.value().shape() != shape
        || source.bound().is_some()
        || source.error().is_some()
        || destination.bound().is_none()
        || destination.error().is_none()
    {
        return Err(LoanError::UnsupportedContract);
    }
    for symbol in [
        Some(destination.value()),
        destination.bound(),
        destination.error(),
    ]
    .into_iter()
    .flatten()
    {
        if symbol == source.value() {
            return Err(LoanError::TargetMismatch);
        }
    }
    Ok(())
}
/// Authenticate the exact operation-to-use pairing before any managed application allocation.
pub(super) fn authenticate(
    request: &PointwiseExpm1Request,
    heap: &crate::Heap,
    account: u64,
    budget: &mut Budget,
) -> Result<(), LoanError> {
    request.law.authenticate_identity_account(account)?;
    crate::loan::charge(budget, Attachment::WORK)?;
    request.law.validate_current(heap)?;
    if Binding::of(&request.law)? != request.operation.binding
        || request.law.authority()?.request().bits != request.operation.bits
    {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}

impl<'a> From<&'a super::PointwiseBinaryRequest> for Request<'a, f64> {
    fn from(request: &'a super::PointwiseBinaryRequest) -> Self {
        Self {
            operation: &request.operation.operation,
            input: &request.left,
            destination: &request.destination,
            demand: request.demand,
            limits: request.limits,
            projected_entry: false,
            expected: crate::KernelKind::BinaryElementwise(crate::BinaryElementwise::Product),
            prepared: None,
            binary: Some((&request.operation, &request.right)),
        }
    }
}
