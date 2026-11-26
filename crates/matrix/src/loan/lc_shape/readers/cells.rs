// SPDX-License-Identifier: AGPL-3.0-only
//! Complete original-role observations preflight every companion before the first load.

mod product;
pub(in crate::loan) use product::LcKernelStage;
pub use product::{
    LcFrequencyPrelude, LcFrequencyTarget, LcInputProduct, LcProductCell, LcProductError,
    LcQuantityDemand, LcRequestedCell, LcRequestedState, LcRequestedValues,
};

use super::{LcInputInventory, LcInputReaders, LcShapeAxes};
use crate::{ExecutionAccount, HostChannel, LoanError, NumericalErrorScope, TileReader};
use nrz_scalar::Scalar;

/// Original companion values and their unchanged numerical-error scope.
#[derive(Clone, Copy, Debug)]
pub struct LcInputReading<P: Scalar> {
    /// Unchanged center, total radius and numerical-only error.
    values: [P; 3],
    /// Original publication scope, without an observation-time upgrade.
    scope: NumericalErrorScope,
}
impl<P: Scalar> LcInputReading<P> {
    /// Original center, total radius and numerical-only error, in that order.
    #[must_use]
    pub const fn values(&self) -> [P; 3] {
        self.values
    }
    /// Observation never strengthens the source publication's numerical claim.
    #[must_use]
    pub const fn numerical_error_scope(&self) -> NumericalErrorScope {
        self.scope
    }
}

/// One complete original tuple, with every role observed or no result returned.
#[derive(Debug)]
pub struct LcInputCell<'a, P: Scalar, const N: usize> {
    /// Exact complete schema inventory that authorized this observation.
    inventory: &'a LcInputInventory<P, N>,
    /// Original common native roles and ordered product identities.
    axes: LcShapeAxes,
    /// Original logical tuple index, independent of physical addresses.
    cell: usize,
    /// Every original position is initialized before this result is exposed.
    readings: [LcInputReading<P>; N],
}
impl<P: Scalar, const N: usize> LcInputCell<'_, P, N> {
    /// Exact original complete inventory, never reconstructed from equal descriptors.
    #[must_use]
    pub const fn inventory(&self) -> &LcInputInventory<P, N> {
        self.inventory
    }
    /// Common original instance/sample/branch identities.
    #[must_use]
    pub const fn axes(&self) -> LcShapeAxes {
        self.axes
    }
    /// Original tuple index within the common complete axis.
    #[must_use]
    pub const fn cell(&self) -> usize {
        self.cell
    }
    /// Every original supplied position, including repeated constraints.
    #[must_use]
    pub const fn readings(&self) -> &[LcInputReading<P>; N] {
        &self.readings
    }
}

/// A complete current reader set with native companion planes resolved once per tile.
///
/// Masks remain independent. A successful cell observation requires all original roles;
/// it does not establish physical feasibility or apply any arithmetic operation.
pub struct LcInputCells<'a, P: Scalar, const N: usize> {
    /// Complete current authority, retaining the original inventory and native borrows.
    readers: LcInputReaders<'a, P, N>,
    /// Every slot is resolved before return; options support fallible array construction.
    bound: [Option<Bound<'a, P>>; N],
}
impl<P: Scalar, const N: usize> std::fmt::Debug for LcInputCells<'_, P, N> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Native planes contain holes: formatting exposes only authenticated metadata.
        formatter
            .debug_struct("LcInputCells")
            .field("readers", &self.readers)
            .finish_non_exhaustive()
    }
}
/// Native companion planes resolved once for one original member.
struct Bound<'a, P: Scalar> {
    /// Immutable center/total/error owners, with a common validated extent.
    planes: [&'a faer::Mat<P>; 3],
    /// Original publication scope, without an observation-time upgrade.
    scope: NumericalErrorScope,
}
/// One fully qualified original member and its checked physical address.
#[derive(Clone, Copy)]
struct Selected<'a, P: Scalar> {
    /// Immutable center/total/error owners, with a common validated extent.
    planes: [&'a faer::Mat<P>; 3],
    /// Original publication scope, without an observation-time upgrade.
    scope: NumericalErrorScope,
    /// Validated row shared by all three resolved one-column planes.
    address: usize,
}
impl<'a, P: Scalar + 'static, const N: usize> LcInputReaders<'a, P, N> {
    /// Plane binding is included once in readiness's per-row tariff.
    pub(in crate::loan) fn into_cells_prepaid(self) -> Result<LcInputCells<'a, P, N>, LoanError> {
        let bound = self.bind_planes()?;
        Ok(LcInputCells {
            readers: self,
            bound,
        })
    }
    /// Both charged and prepaid callers use the same five-visit plane gate per row.
    fn bind_planes(&self) -> Result<[Option<Bound<'a, P>>; N], LoanError> {
        let mut bound = std::array::from_fn(|_| None);
        for (slot, reader) in bound.iter_mut().zip(&self.readers) {
            *slot = Some(bind(
                reader.as_ref().ok_or(LoanError::UnsupportedContract)?,
            )?);
        }
        Ok(bound)
    }

    /// Resolve all original scalar companion planes once without loading a value.
    /// # Errors
    /// Refuses another account, incomplete representations and exhausted Work.
    /// Refusal consumes this reader set; the caller retains its tiles and can re-admit them.
    pub fn into_cells(
        self,
        account: &mut ExecutionAccount,
    ) -> Result<LcInputCells<'a, P, N>, LoanError> {
        let original = self
            .inventory
            .input(0)
            .ok_or(LoanError::UnsupportedContract)?;
        account.authenticate(original.account)?;
        let bound = account.ledger.with_work_phase(
            crate::loan::FAMILY,
            "LC input cell binding",
            |budget| {
                crate::loan::charge(budget, work::<N>(5)?)?;
                self.bind_planes()
            },
        )?;
        Ok(LcInputCells {
            readers: self,
            bound,
        })
    }
}
impl<'a, P: Scalar + 'static, const N: usize> LcInputCells<'a, P, N> {
    /// Observe every original role only after complete coverage and address preflight.
    /// # Errors
    /// Refuses foreign accounts, holes, out-of-capacity tuples and Work before any load.
    pub fn read_cell(
        &self,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<LcInputCell<'a, P, N>, LoanError> {
        let original = self
            .readers
            .inventory
            .input(0)
            .ok_or(LoanError::UnsupportedContract)?;
        account.authenticate(original.account)?;
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "LC input cell observation", |budget| {
                crate::loan::charge(budget, work::<N>(7)?)?;
                self.observe(cell)
            })
    }
    /// Complete preflight and loads shared by prepaid observation and product consumers.
    fn observe(&self, cell: usize) -> Result<LcInputCell<'a, P, N>, LoanError> {
        let selected = self.select(cell)?;
        let readings = selected.map(|selected| {
            let values = selected.planes.map(|plane| {
                #[cfg(test)]
                crate::loan::fault::loaded();
                plane[(selected.address, 0)]
            });
            LcInputReading {
                values,
                scope: selected.scope,
            }
        });
        Ok(LcInputCell {
            inventory: self.readers.inventory,
            axes: self.readers.axes,
            cell,
            readings,
        })
    }

    /// All addresses and all three qualified channels are checked before any selected load.
    fn select(&self, cell: usize) -> Result<[Selected<'a, P>; N], LoanError> {
        if cell >= self.readers.axes.combined.axis().capacity() {
            return Err(LoanError::LayoutMismatch);
        }
        if N == 0 {
            return Err(LoanError::UnsupportedContract);
        }
        let first = self.select_member(0, cell)?;
        let mut selected = [first; N];
        for (index, slot) in selected.iter_mut().enumerate().skip(1) {
            *slot = self.select_member(index, cell)?;
        }
        Ok(selected)
    }
    /// Readiness shares complete selection without loading a numerical component.
    pub(in crate::loan) fn preflight(&self, cell: usize) -> Result<(), LoanError> {
        self.select(cell).map(|_| ())
    }
    /// Resolve one position without loading any numerical companion.
    fn select_member(&self, index: usize, cell: usize) -> Result<Selected<'a, P>, LoanError> {
        let reader = self
            .readers
            .reader(index)
            .ok_or(LoanError::UnsupportedContract)?;
        for channel in HostChannel::ALL {
            reader.selected_owner(channel, cell, 0)?;
        }
        let address = reader.bundle.layout.components.index(cell, 0)?;
        let bound = self.bound[index]
            .as_ref()
            .ok_or(LoanError::UnsupportedContract)?;
        if address >= bound.planes[0].nrows() {
            return Err(LoanError::LayoutMismatch);
        }
        Ok(Selected {
            planes: bound.planes,
            scope: bound.scope,
            address,
        })
    }
}
/// Checked header plus literal per-member visit inventory.
fn work<const N: usize>(per_member: usize) -> Result<usize, LoanError> {
    N.checked_mul(per_member)
        .and_then(|work| work.checked_add(1))
        .ok_or(LoanError::CountOverflow)
}
/// Resolve the admitted scalar representation and preserve its original error scope.
fn bind<'a, P: Scalar + 'static>(reader: &TileReader<'a, P>) -> Result<Bound<'a, P>, LoanError> {
    let bundle = reader.bundle;
    if bundle.endpoints.is_some()
        || bundle.retained_law.is_some()
        || bundle.declaration.extent.columns() != 1
    {
        return Err(LoanError::UnsupportedContract);
    }
    let planes = bundle.channels.each_ref().map(|channel| {
        channel
            .as_ref()
            .ok_or(LoanError::UnsupportedContract)?
            .matrix
            .typed::<P>()
    });
    let [center, total, error] = planes;
    let planes = [center?, total?, error?];
    if planes
        .iter()
        .any(|plane| plane.ncols() != 1 || plane.nrows() != planes[0].nrows())
    {
        return Err(LoanError::LayoutMismatch);
    }
    Ok(Bound {
        planes,
        scope: reader.numerical_error_scope(),
    })
}

#[cfg(test)]
mod containment_tests {
    use super::*;
    use nrz_bindings::{Budget, LcArithmeticMode, LcContainmentError, LcStratum, Limits};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A total outer cover remains usable with local numerical error scope.
    #[test]
    fn lc_containment_local_total_cover() -> TestResult {
        let mut budget = Budget::new(Limits::default())?;
        let stratum32 = LcStratum::<f32> {
            lower: None,
            upper: None,
        };
        let stratum64 = LcStratum::<f64> {
            lower: None,
            upper: None,
        };
        for scope in [
            NumericalErrorScope::PropagatedLocal,
            NumericalErrorScope::LocalOperation,
        ] {
            let single = LcInputReading {
                values: [0.0_f32, 0.0, 0.0],
                scope,
            };
            let double = LcInputReading {
                values: [0.0_f64, 0.0, 0.0],
                scope,
            };
            assert!(matches!(
                single.check_containment32(
                    [0.0, 0.0],
                    &stratum32,
                    LcArithmeticMode::REQUIRED,
                    &mut budget
                ),
                Ok(true)
            ));
            assert!(matches!(
                double.check_containment64(
                    [0.0, 0.0],
                    &stratum64,
                    LcArithmeticMode::REQUIRED,
                    &mut budget
                ),
                Ok(true)
            ));
        }
        let single = LcInputReading {
            values: [0.0_f32, 0.0, f32::from_bits(1)],
            scope: NumericalErrorScope::HostInput,
        };
        let double = LcInputReading {
            values: [0.0_f64, 0.0, f64::from_bits(1)],
            scope: NumericalErrorScope::HostInput,
        };
        assert!(matches!(
            single.check_containment32(
                [0.0, 0.0],
                &stratum32,
                LcArithmeticMode::REQUIRED,
                &mut budget
            ),
            Err(LcContainmentError::InvalidInput)
        ));
        assert!(matches!(
            double.check_containment64(
                [0.0, 0.0],
                &stratum64,
                LcArithmeticMode::REQUIRED,
                &mut budget
            ),
            Err(LcContainmentError::InvalidInput)
        ));
        Ok(())
    }
}
