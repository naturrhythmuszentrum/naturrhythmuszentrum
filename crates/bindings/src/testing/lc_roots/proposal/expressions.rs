// SPDX-License-Identifier: AGPL-3.0-only
//! Root slots, exact coefficients and predicate syntax introduce no proof application.

use super::{Importer, TestResult};
use nrz_bindings::readings::algebra::{
    Handle,
    source::{SyntaxDescriptor, SyntaxNode, SyntaxRole},
    syntax::{Draft, ExpressionDraft, Operator},
};

impl Importer<'_> {
    pub(super) fn expression(&mut self, source: &Handle) -> TestResult<Draft> {
        let syntax = self.source.describe_syntax(source, self.budget)?;
        let header = self.reference(&syntax.header)?;
        match &syntax.node {
            SyntaxNode::Condition(kind) => Ok(Draft::Condition {
                header,
                scope: syntax.scope.clone(),
                kind: kind.clone(),
                arguments: self.operands(&syntax)?,
            }),
            SyntaxNode::Expression(value) => match value.operator {
                Operator::RootValue { slot } => Ok(Draft::RootValue {
                    header,
                    slot,
                    selection: self.reference(dependency(&syntax, SyntaxRole::Semantic(0))?)?,
                }),
                Operator::ExactCoefficient => Ok(Draft::ExactCoefficient {
                    header,
                    value: self.reference(dependency(&syntax, SyntaxRole::Semantic(0))?)?,
                    map: self.reference(dependency(&syntax, SyntaxRole::Semantic(1))?)?,
                }),
                _ => Ok(Draft::Expression(ExpressionDraft {
                    header,
                    scope: syntax.scope.clone(),
                    operator: value.operator.clone(),
                    sort: value.sort.clone(),
                    operands: self.operands(&syntax)?,
                    space: None,
                    binding: syntax.binding.clone(),
                })),
            },
            _ => Err(format!("unsupported LC candidate syntax: {syntax:?}").into()),
        }
    }

    fn operands(
        &mut self,
        syntax: &SyntaxDescriptor,
    ) -> TestResult<Vec<nrz_bindings::readings::algebra::Reference>> {
        let mut values = Vec::new();
        for dependency in &syntax.dependencies {
            if !matches!(dependency.role, SyntaxRole::Operand(_)) {
                return Err("unexpected LC candidate expression dependency".into());
            }
            values.push(self.reference(&dependency.target)?);
        }
        Ok(values)
    }
}

fn dependency(syntax: &SyntaxDescriptor, role: SyntaxRole) -> TestResult<&Handle> {
    syntax
        .dependencies
        .iter()
        .find(|entry| entry.role == role)
        .map(|entry| &entry.target)
        .ok_or_else(|| "LC candidate dependency".into())
}
