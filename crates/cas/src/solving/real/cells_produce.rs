// SPDX-License-Identifier: AGPL-3.0-only
//! Exact Real cells, count-one selections and ordinary coverage consequences.

use super::leaf::Leaf;
use crate::graph::{Draft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, PredicateKind};
use crate::solving::coefficients::{
    model::CoefficientFailure,
    storage::{invalid, vector},
};
use crate::solving::towers::{dense::Dense, materialize};
use crate::solving::{
    CertificateRealCellExists, CertificateRealRootCount, RealCellData, RealEndpoint,
    RootSelectionData, SelectionMode, SolveCertificate, SolveDraft, SolveSchema, SolveStatement,
    StatementCoverage, StatementRootExistence,
};

impl Leaf<'_> {
    /// Signed infinities are structural endpoints; finite expressions retain their actual Real dependencies.
    pub(crate) fn cell(
        &mut self,
        left: RealEndpoint<Reference>,
        right: RealEndpoint<Reference>,
    ) -> Result<Handle, CoefficientFailure> {
        Ok(materialize::admit(
            self.cas,
            SolveDraft::RealCell(RealCellData {
                header: Reference::Existing(self.frame.header.clone()),
                scope: Vec::new(),
                region: Reference::Existing(self.frame.region.clone()),
                left,
                right,
                left_open: true,
                right_open: true,
            }),
            self.budget,
        )?)
    }

    /// A full-line count is useful even when no finite endpoint sign is needed.
    pub(crate) fn whole_line(
        &mut self,
        certificate: &mut CertificateRealRootCount,
        count: u32,
    ) -> Result<(), CoefficientFailure> {
        certificate.cells = vec![self.cell(
            RealEndpoint::NegativeInfinity,
            RealEndpoint::PositiveInfinity,
        )?];
        certificate.counts = vec![count];
        Ok(())
    }

    /// Count-one supplies precisely the isolated selection named by the actual cell metadata.
    pub(crate) fn selected_cells(
        &mut self,
        certificate: &CertificateRealRootCount,
        count: u32,
    ) -> Result<Vec<Handle>, CoefficientFailure> {
        let bound = self.frame.expression(
            self.cas,
            Operator::Bound { depth: 0, slot: 0 },
            &[],
            true,
            self.budget,
        )?;
        let predicate = super::construction::admit(
            self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.frame.header.clone()),
                scope: vec![vec![AtomicSort::Scalar(Field::Real)]],
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(bound.clone()),
                    Reference::Existing(bound),
                ],
            },
            self.budget,
        )?;
        let mut roots = vector(certificate.cells.len(), self.budget)?;
        for cell in &certificate.cells {
            let selection = materialize::admit(
                self.cas,
                SolveDraft::RootSelection(RootSelectionData {
                    cell: Some(Reference::Existing(cell.clone())),
                    header: Reference::Existing(self.frame.header.clone()),
                    scope: Vec::new(),
                    relations: vec![Reference::Existing(certificate.relation.clone())],
                    predicate: Reference::Existing(predicate.clone()),
                    mode: SelectionMode::Isolated,
                    convention: None,
                    interval: None,
                }),
                self.budget,
            )?;
            let shape = super::count::cell(self.cas, cell)?;
            let finite = [&shape.left, &shape.right]
                .into_iter()
                .filter(|endpoint| matches!(endpoint, RealEndpoint::RealExpression(_)))
                .count();
            let region = self.frame.region.clone();
            self.frame.push(
                SolveSchema::RealCellExists,
                SolveStatement::RootExistence(StatementRootExistence {
                    relation: selection.clone(),
                    region: region.clone(),
                }),
                SolveCertificate::RealCellExists(CertificateRealCellExists {
                    relation: certificate.relation.clone(),
                    cell: cell.clone(),
                    predicate: predicate.clone(),
                    region,
                    count,
                    definedness: vec![count; finite],
                    order: vec![count],
                }),
                self.budget,
            )?;
            roots.push(super::construction::admit(
                self.cas,
                &Draft::RootValue {
                    header: Reference::Existing(self.frame.header.clone()),
                    selection: Reference::Existing(selection),
                    slot: 0,
                },
                self.budget,
            )?);
        }
        Ok(roots)
    }

    /// Finite membership, distinctness and the complete count are all replayed against the original equation.
    pub(crate) fn counted_coverage(
        &mut self,
        mut certificate: CertificateRealRootCount,
        source: &Dense,
        scale: &crate::solving::coefficients::model::Element,
        roots: &[Handle],
    ) -> Result<Handle, CoefficientFailure> {
        if !certificate.identities.is_empty() {
            return Err(invalid().into());
        }
        let set = self.set(roots)?;
        certificate
            .identities
            .push(self.source_identity(source, scale)?);
        let region = self.frame.region.clone();
        self.frame.push(
            SolveSchema::RealRootCount,
            SolveStatement::Coverage(StatementCoverage {
                problem: self.problem.clone(),
                set: set.clone(),
                region,
            }),
            SolveCertificate::RealRootCount(certificate),
            self.budget,
        )?;
        Ok(set)
    }
}
