// SPDX-License-Identifier: AGPL-3.0-only
//! Placement query scheduling keeps failed incidence attempts on the same request ledger.

use super::{build, invalid, membership::PlacementOutcome};
use crate::budget::Budget;
use crate::error::{Fault, FaultKind, InvalidCode};
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, Scope, Sort};
use crate::proof_model::VerifiedWitness;
use crate::solving::radicals::construction::admit;
use crate::solving::{FamilyArgument, FamilyArgumentRole, FamilyExpressionData, SolveDraft};

impl Cas {
    /// Decide the sole registered placement at a separately supplied Complex query.
    ///
    /// Incidence and exclusion are derived from replayed native case component
    /// identities. An undecided query returns no value and no exclusion theorem.
    ///
    /// # Errors
    /// Refuses mismatched pieces, contexts, query sorts or scopes, and exhausted budgets.
    pub fn placement_membership(
        &mut self,
        context: &Handle,
        piece: &Handle,
        location: &Handle,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<PlacementOutcome, Fault> {
        self.transaction(generation, |cas, generation| {
            let SolveDraft::ScalarPiece(value) =
                crate::solving::graph::object(&cas.state, cas.checked(piece)?)?
            else {
                return Err(invalid());
            };
            let region = value.region.clone();
            if cas.state.context(cas.checked(context)?)?.admissible != region.logical_id() {
                return Err(invalid());
            }
            super::membership::validate(cas, piece, location, &region)?;
            let data = cas.state.region(region.logical_id())?;
            let family = cas.handle(data.lies_over.as_ref().ok_or_else(invalid)?.family);
            let header = cas.handle(data.header);
            let scope = data.scope.clone();
            Query {
                cas,
                context,
                piece,
                location,
                region,
                family,
                header,
                scope,
                generation,
                checker,
            }
            .run()
        })
    }
}

/// One incidence/exclusion request owns both budgets across both attempts.
struct Query<'a> {
    /// Unpublished candidate graph.
    cas: &'a mut Cas,
    /// Exact context supplied by the caller.
    context: &'a Handle,
    /// Original registered piece.
    piece: &'a Handle,
    /// Same-Complex query substitution for the local bound location.
    location: &'a Handle,
    /// Exact theorem domain.
    region: Handle,
    /// Family determining the coefficient and unknown roles.
    family: Handle,
    /// Immutable original declarations.
    header: Handle,
    /// Exact outer binder frames.
    scope: Scope,
    /// Monotone generation ledger.
    generation: &'a mut Budget,
    /// Monotone independent checking ledger.
    checker: &'a mut Budget,
}

impl Query<'_> {
    /// No branch assigns a sampled value to any base or fibre declaration.
    fn run(&mut self) -> Result<PlacementOutcome, Fault> {
        let zero = self.constant(false, Field::Real)?;
        let arguments = [
            FamilyArgument {
                role: FamilyArgumentRole::Base,
                value: Reference::Existing(zero.clone()),
            },
            FamilyArgument {
                role: FamilyArgumentRole::Fibre,
                value: Reference::Existing(self.location.clone()),
            },
        ];
        let (placed, _) = self.operation(Operator::Add, Field::Complex, &arguments)?;
        let argument = FamilyArgument {
            role: FamilyArgumentRole::Family,
            value: Reference::Existing(placed),
        };
        let (imaginary, view) = self.operation(
            Operator::ImagPart,
            Field::Real,
            std::slice::from_ref(&argument),
        )?;
        if let Some(incidence) = self.attempt(&view, &zero)? {
            let (_, real) = self.operation(
                Operator::RealPart,
                Field::Real,
                std::slice::from_ref(&argument),
            )?;
            let local = build::expression(
                self.cas,
                &self.header,
                &self.scope,
                Operator::RealPart,
                Field::Real,
                std::slice::from_ref(self.location),
                self.generation,
            )?;
            let value = self.cas.family_identity(
                self.context,
                &real,
                &local,
                self.generation,
                self.checker,
            )?;
            return Ok(PlacementOutcome::Inside {
                piece: self.piece.clone(),
                location: self.location.clone(),
                argument: local,
                incidence: Box::new([value, incidence]),
            });
        }
        let argument = FamilyArgument {
            role: FamilyArgumentRole::Family,
            value: Reference::Existing(imaginary),
        };
        let (_, quotient) =
            self.operation(Operator::Div, Field::Complex, &[argument.clone(), argument])?;
        let one = self.constant(true, Field::Complex)?;
        if let Some(nonzero) = self.attempt(&quotient, &one)? {
            return Ok(PlacementOutcome::Refused {
                piece: self.piece.clone(),
                location: self.location.clone(),
                reason: crate::request_model::RefusalReason::NoApplicablePiece,
                exclusion: Box::new(nonzero),
            });
        }
        Ok(PlacementOutcome::Unresolved {
            reason: crate::solving::SolveReason::PieceMembershipPending,
        })
    }

    /// Missing mathematical evidence is distinct from malformed syntax or a spent ledger.
    fn attempt(&mut self, left: &Handle, right: &Handle) -> Result<Option<VerifiedWitness>, Fault> {
        match self
            .cas
            .family_identity(self.context, left, right, self.generation, self.checker)
        {
            Ok(value) => Ok(Some(value)),
            Err(fault)
                if matches!(
                    fault.kind,
                    FaultKind::Invalid(
                        InvalidCode::Rule
                            | InvalidCode::UnsupportedFragment
                            | InvalidCode::PendingPremise
                    )
                ) =>
            {
                Ok(None)
            }
            Err(fault) => Err(fault),
        }
    }

    /// Build a native local operation and retain its underlying body for nested operations.
    fn operation(
        &mut self,
        operator: Operator,
        field: Field,
        arguments: &[FamilyArgument],
    ) -> Result<(Handle, Handle), Fault> {
        let sort = Sort::Value(AtomicSort::Scalar(field));
        let ambient =
            crate::solving::sorts::ambient(&self.cas.state, self.header.logical_id(), &sort)?
                .ok_or_else(invalid)?;
        let value = FamilyExpressionData {
            family: Reference::Existing(self.family.clone()),
            ambient: Reference::Existing(self.cas.handle(ambient)),
            region: Reference::Existing(self.region.clone()),
            scope: self.scope.clone(),
            operator,
            result: sort,
            arguments: arguments.to_vec(),
        };
        let body = admit(
            self.cas,
            &Draft::Solve(SolveDraft::FamilyExpression(value)),
            self.generation,
        )?;
        let cases = self.view(&body)?;
        Ok((body, cases))
    }

    /// Zero and one belong to the requested scalar space without changing any declaration.
    fn constant(&mut self, one: bool, field: Field) -> Result<Handle, Fault> {
        let value = if one {
            crate::number::Rational::one()
        } else {
            crate::number::Rational::zero()
        };
        build::expression(
            self.cas,
            &self.header,
            &self.scope,
            Operator::Rational(value),
            field,
            &[],
            self.generation,
        )
    }

    /// Every required association names the caller's original registered piece.
    fn view(&mut self, body: &Handle) -> Result<Handle, Fault> {
        use crate::domain_model::{CaseArmDraft, CasesDraft};
        let local = super::body(self.cas, body.logical_id())?.ok_or_else(invalid)?;
        let result = local.result.clone();
        let operands =
            crate::solving::sorts::placement_operands(&self.cas.state, local, self.generation)?;
        let placements = operands
            .into_iter()
            .map(|(operand, _)| crate::solving::PlacementUse {
                operand: Reference::Existing(self.cas.handle(operand)),
                piece: Reference::Existing(self.piece.clone()),
            })
            .collect();
        let arm = admit(
            self.cas,
            &Draft::CaseArm(CaseArmDraft {
                placements,
                header: Reference::Existing(self.header.clone()),
                scope: self.scope.clone(),
                region: Reference::Existing(self.region.clone()),
                value: Reference::Existing(body.clone()),
            }),
            self.generation,
        )?;
        admit(
            self.cas,
            &Draft::Cases(CasesDraft {
                sort: result,
                header: Reference::Existing(self.header.clone()),
                scope: self.scope.clone(),
                domain: Reference::Existing(self.region.clone()),
                arms: vec![Reference::Existing(arm)],
            }),
            self.generation,
        )
    }
}
