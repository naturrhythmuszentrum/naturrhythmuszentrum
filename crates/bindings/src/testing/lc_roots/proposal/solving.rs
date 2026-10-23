// SPDX-License-Identifier: AGPL-3.0-only
//! Only the candidate set's closed typed solving vocabulary is copied.

use super::{Importer, TestResult};
use nrz_bindings::readings::algebra::{
    Handle,
    solving::{
        CoefficientValueObject, ExtensionLevelData, RootRelationData, RootSelectionData,
        SolutionBranchData, SolutionBranchKind, SolutionSetData, SolveDraft, SolveView, TupleData,
    },
};

impl Importer<'_> {
    pub(super) fn solving(&mut self, view: SolveView<'_>) -> TestResult<SolveDraft> {
        Ok(match view {
            SolveView::CoefficientDomain(data) => SolveDraft::CoefficientDomain(self.domain(data)?),
            SolveView::CoefficientValue(data) => {
                SolveDraft::CoefficientValue(CoefficientValueObject {
                    domain: self.reference(&data.domain)?,
                    value: self.value(&data.value)?,
                })
            }
            SolveView::Polynomial(data) => SolveDraft::Polynomial(self.polynomial(data)?),
            SolveView::CoefficientMap(data) => SolveDraft::CoefficientMap(self.mapping(data)?),
            SolveView::ExtensionLevel(data) => SolveDraft::ExtensionLevel(ExtensionLevelData {
                parent: self.reference(&data.parent)?,
                polynomial: self.reference(&data.polynomial)?,
                generator: data.generator.clone(),
                square_free: data.square_free,
            }),
            SolveView::RootRelation(data) => SolveDraft::RootRelation(self.relation(data)?),
            SolveView::RootSelection(data) => SolveDraft::RootSelection(self.selection(data)?),
            SolveView::Tuple(data) => SolveDraft::Tuple(TupleData {
                header: self.reference(&data.header)?,
                scope: data.scope.clone(),
                coordinates: self.reference(&data.coordinates)?,
                entries: self.references(&data.entries)?,
            }),
            SolveView::SolutionBranch(data) => SolveDraft::SolutionBranch(self.branch(data)?),
            SolveView::SolutionSet(data) => SolveDraft::SolutionSet(SolutionSetData {
                problem: self.reference(&data.problem)?,
                branches: self.references(&data.branches)?,
            }),
            _ => return Err(format!("unsupported LC candidate object: {view:?}").into()),
        })
    }

    fn relation(&mut self, data: &RootRelationData<Handle>) -> TestResult<RootRelationData> {
        Ok(RootRelationData {
            header: self.reference(&data.header)?,
            scope: data.scope.clone(),
            ambient: data.ambient.clone(),
            polynomial: self.reference(&data.polynomial)?,
            binder: data.binder.clone(),
            map: self.reference(&data.map)?,
            region: self.reference(&data.region)?,
            multiplicity: data.multiplicity,
        })
    }

    fn selection(&mut self, data: &RootSelectionData<Handle>) -> TestResult<RootSelectionData> {
        Ok(RootSelectionData {
            cell: data
                .cell
                .as_ref()
                .map(|value| self.reference(value))
                .transpose()?,
            header: self.reference(&data.header)?,
            scope: data.scope.clone(),
            relations: self.references(&data.relations)?,
            predicate: self.reference(&data.predicate)?,
            mode: data.mode,
            convention: data.convention.clone(),
            interval: data.interval.clone(),
        })
    }

    fn branch(&mut self, data: &SolutionBranchData<Handle>) -> TestResult<SolutionBranchData> {
        let SolutionBranchKind::Finite { tuples } = &data.kind else {
            return Err("LC finite candidate".into());
        };
        Ok(SolutionBranchData {
            problem: self.reference(&data.problem)?,
            region: self.reference(&data.region)?,
            domain: self.reference(&data.domain)?,
            kind: SolutionBranchKind::Finite {
                tuples: self.references(tuples)?,
            },
        })
    }
}
