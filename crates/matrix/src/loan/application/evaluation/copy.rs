// SPDX-License-Identifier: AGPL-3.0-only
//! Tile-bound complete companion owners avoid repeating authority checks at each load.

use crate::loan::{HostChannel, LoanError, TileReader};
use nrz_scalar::Scalar;

/// Borrowed immutable owners are valid for the complete prepaid selected traversal.
pub(super) struct BoundCopy<'a, P: Scalar> {
    /// Original component addressing, shared by the three numerical planes.
    layout: &'a crate::Layout,
    /// Fixed complete center, total and error owners.
    channels: [&'a faer::Mat<P>; 3],
}
impl<'a, P: Scalar + 'static> BoundCopy<'a, P> {
    /// Validate all companion attachments once, after the source pin and capture were checked.
    pub(super) fn bind(reader: &TileReader<'a, P>) -> Result<Self, LoanError> {
        let bundle = reader.bundle;
        let owner = |channel: HostChannel| {
            let qualified = bundle.qualified[channel.index()].ok_or(LoanError::CoverageMismatch)?;
            if qualified.identity != bundle.identity || qualified.channel != channel {
                return Err(LoanError::CoverageMismatch);
            }
            let matrix = bundle.channels[channel.index()]
                .as_ref()
                .ok_or(LoanError::UnsupportedContract)?
                .matrix
                .typed::<P>()?;
            if matrix.ncols() != 1 {
                return Err(LoanError::LayoutMismatch);
            }
            Ok(matrix)
        };
        let channels = [
            owner(HostChannel::Center)?,
            owner(HostChannel::Total)?,
            owner(HostChannel::Error)?,
        ];
        if channels
            .iter()
            .any(|matrix| matrix.nrows() != channels[0].nrows())
        {
            return Err(LoanError::LayoutMismatch);
        }
        Ok(Self {
            layout: &bundle.layout.components,
            channels,
        })
    }

    /// Two complete bound planes share one checked address and prepaid numerical observation.
    pub(super) fn pair(
        &self,
        cell: usize,
        component: usize,
        _permit: &super::Traversal,
    ) -> Result<[P; 2], LoanError> {
        let address = self.address(cell, component)?;
        #[cfg(test)]
        for _ in 0..2 {
            crate::loan::fault::loaded();
        }
        Ok([
            self.channels[0][(address, 0)],
            self.channels[1][(address, 0)],
        ])
    }

    /// Projected unary laws share bound owners without materializing a copied tile.
    pub(super) fn evaluate(
        &self,
        member: super::Member<'_>,
        cell: usize,
        component: usize,
        entry: usize,
        permit: &super::Traversal,
    ) -> Result<super::selected::Values<P>, super::CellOutcome> {
        if member.is_transform() {
            return self
                .component(cell, component, permit)
                .map(super::selected::Values::Closed)
                .map_err(|_| member.storage_failure(component));
        }
        let super::Member::ProjectedElementwise(_, function) = member else {
            return Err(member.storage_failure(component));
        };
        let load = || -> Result<_, LoanError> {
            let address = self.address(cell, component)?;
            #[cfg(test)]
            for _ in 0..2 {
                crate::loan::fault::loaded();
            }
            Ok((
                self.channels[0][(address, 0)],
                self.channels[1][(address, 0)],
            ))
        };
        let (center, total) = load().map_err(|_| member.storage_failure(component))?;
        let result = nrz_accuracy::Nonnegative::new(total)
            .map_err(crate::KernelError::from)
            .and_then(|total| {
                crate::kernels::nonlinear::evaluate_selected(
                    center,
                    total,
                    function,
                    entry,
                    &permit.context,
                )
            });
        result
            .map(|value| super::selected::Values::Closed([value.center, value.total, value.error]))
            .map_err(|reason| super::CellOutcome::CellRefused { component, reason })
    }

    /// One checked address serves all equal-length immutable component planes.
    fn address(&self, cell: usize, component: usize) -> Result<usize, LoanError> {
        let entry = self.layout.index(cell, component)?;
        if entry >= self.channels[0].nrows() {
            return Err(LoanError::LayoutMismatch);
        }
        Ok(entry)
    }

    /// The sealed filled words already authenticate this cell's complete bundle coverage.
    pub(super) fn component(
        &self,
        cell: usize,
        component: usize,
        _permit: &super::Traversal,
    ) -> Result<[P; 3], LoanError> {
        let entry = self.address(cell, component)?;
        #[cfg(test)]
        for _ in 0..3 {
            crate::loan::fault::loaded();
        }
        Ok([
            self.channels[0][(entry, 0)],
            self.channels[1][(entry, 0)],
            self.channels[2][(entry, 0)],
        ])
    }
}

/// Cartesian binary arithmetic uses both immutable companion bundles bound once per tile.
pub(super) fn binary<P: Scalar + 'static>(
    member: super::Member<'_>,
    left: &BoundCopy<'_, P>,
    right: &BoundCopy<'_, f64>,
    cells: [usize; 2],
    component: usize,
    entry: usize,
    permit: &super::Traversal,
) -> Result<super::selected::Values<P>, super::CellOutcome> {
    let super::Member::Binary(_, function) = member else {
        return Err(member.storage_failure(component));
    };
    let left = left
        .pair(cells[0], component, permit)
        .map_err(|_| member.storage_failure(component))?;
    let right = right
        .pair(cells[1], component, permit)
        .map_err(|_| member.storage_failure(component))?;
    crate::kernels::binary::evaluate_selected(
        [left[0].to_f64(), right[0]],
        [left[1].to_f64(), right[1]],
        function,
        entry,
        &permit.context,
    )
    .map(|value| super::selected::Values::Binary([value.center, value.total, value.error]))
    .map_err(|reason| super::CellOutcome::CellRefused { component, reason })
}

/// Nonprojected members keep their existing reader path without binding unused owners.
pub(super) fn bind<'a, P: Scalar + 'static>(
    reader: Option<&TileReader<'a, P>>,
    enabled: bool,
) -> Result<Option<BoundCopy<'a, P>>, LoanError> {
    if enabled {
        BoundCopy::bind(reader.ok_or(LoanError::WrongLoan)?).map(Some)
    } else {
        Ok(None)
    }
}
