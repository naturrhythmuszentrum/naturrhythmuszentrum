// SPDX-License-Identifier: AGPL-3.0-only
//! Constant-size original incidence binds factor axes without per-cell descriptors.

use crate::{CellAxis, CellLayoutVersion, CellProduct, Extent, LoanError};

/// Stack-only construction and authentication view of the retained native recipe.
#[derive(Clone, Copy)]
pub(in crate::loan::application) enum Geometry {
    /// Identity incidence with optional whole-tile scalar repetition.
    Aligned(CellAxis),
    /// Left is outer and Right is inner on the exact product demand layout.
    Cartesian(CellProduct, CellLayoutVersion),
}
impl Geometry {
    /// Original output axis is independent of either factor's physical storage layout.
    pub(in crate::loan::application) const fn axis(self) -> CellAxis {
        match self {
            Self::Aligned(axis) => axis,
            Self::Cartesian(product, _) => product.axis(),
        }
    }
    /// Retain copied native identities inside the existing largest recipe allocation.
    pub(in crate::loan::application) const fn recipe(
        self,
        function: crate::BinaryElementwise,
    ) -> crate::kernels::contract::Recipe {
        match self {
            Self::Aligned(_) => crate::kernels::contract::Recipe::BinaryElementwise(function),
            Self::Cartesian(product, layout) => {
                crate::kernels::contract::Recipe::CartesianBinary(product, layout, function)
            }
        }
    }
    /// Each source's logical axis is checked independently of its physical component map.
    pub(in crate::loan::application) fn source_axis(
        self,
        index: usize,
        actual: CellAxis,
        input: Extent,
        output: Extent,
    ) -> CellAxis {
        match self {
            Self::Aligned(axis) => {
                if input == output {
                    axis
                } else {
                    actual
                }
            }
            Self::Cartesian(product, _) => product.factors()[index],
        }
    }
    /// Cartesian rows derive from checked factors; components retain identity incidence.
    pub(in crate::loan::application) fn extents(
        self,
        inputs: [Extent; 2],
        output: Extent,
    ) -> Result<(), LoanError> {
        match self {
            Self::Aligned(_) => super::extents(inputs, output),
            Self::Cartesian(product, layout) => {
                if layout.scope != product.axis().scope {
                    return Err(LoanError::ForeignOwner);
                }
                let rows = inputs.each_ref().map(|extent| extent.rows());
                let factors = product.factors().map(CellAxis::capacity);
                if rows != factors
                    || output.rows() != product.axis().capacity()
                    || inputs
                        .iter()
                        .any(|input| input.columns() != output.columns())
                {
                    return Err(LoanError::UnsupportedBinaryExtent);
                }
                Ok(())
            }
        }
    }
}
