// SPDX-License-Identifier: AGPL-3.0-only
//! Map domains retain scalar descriptors or formal quotients without inventing a scalar kind.

use crate::{Descriptor, PairingWitness, Role};

/// Coordinate meaning on one side of a checked map.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Coordinates {
    /// A scalar quantity in its declared measure.
    Scalar(Box<Descriptor>),
    /// Coherent physical coordinates of the retained formal row/column quotient.
    Quotient {
        /// Original ordered reference descriptors and their checked quotient relation.
        pairing: Box<PairingWitness>,
        /// Affine role of the coherent entry, after checking any named point origin.
        role: Role,
    },
}

/// Scalar coordinates or a coherent formal quotient, including its affine role.
///
/// Formal quotients retain both reference descriptors and have no fabricated scalar kind.
/// Their coordinates are coherent even though the pairing retains the reference measures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapCoordinates {
    /// Private variants prevent construction of unchecked map-coordinate combinations.
    coordinates: Coordinates,
}

impl MapCoordinates {
    /// Retain a checked scalar descriptor.
    pub(crate) fn scalar(descriptor: Descriptor) -> Self {
        Self {
            coordinates: Coordinates::Scalar(Box::new(descriptor)),
        }
    }

    /// Retain a checked reference quotient and the separately checked origin role.
    pub(crate) fn formal(pairing: PairingWitness, role: Role) -> Self {
        Self {
            coordinates: Coordinates::Quotient {
                pairing: Box::new(pairing),
                role,
            },
        }
    }

    /// Scalar descriptor, absent when the physical meaning is a formal quotient.
    #[must_use]
    pub fn descriptor(&self) -> Option<&Descriptor> {
        match &self.coordinates {
            Coordinates::Scalar(descriptor) => Some(descriptor),
            Coordinates::Quotient { .. } => None,
        }
    }

    /// Formal quotient with its original ordered reference descriptors, when present.
    #[must_use]
    pub fn quotient(&self) -> Option<&PairingWitness> {
        match &self.coordinates {
            Coordinates::Quotient { pairing, .. } => Some(pairing),
            Coordinates::Scalar(_) => None,
        }
    }

    /// Points remove or restore their named origin; differences always have zero shifts.
    #[must_use]
    pub fn role(&self) -> Role {
        match &self.coordinates {
            Coordinates::Scalar(descriptor) => descriptor.role(),
            Coordinates::Quotient { role, .. } => *role,
        }
    }
}
