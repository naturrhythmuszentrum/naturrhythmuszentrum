// SPDX-License-Identifier: AGPL-3.0-only
//! Pairing proposals retain supported-action evidence and exact symbolic test derivatives.

use crate::budget::BudgetKind;
use crate::distribution_model::{DistributionCertificate, DistributionStatement};
use crate::distribution_producer::Runner;
use crate::distribution_syntax::{AT, invalid};
use crate::error::{Fault, InvalidCode};
use crate::graph::Handle;
use crate::model::Operator;
use crate::proof_model::Certificate;
use crate::rule::Primitive;
use crate::task::ActionMode;

impl Runner<'_> {
    /// Pair a finite rational sum using separately checked term actions and linearity.
    pub(crate) fn pair(
        &mut self,
        input: &Handle,
        function: &Handle,
        mode: &ActionMode,
    ) -> Result<(Handle, Handle), Fault> {
        self.ensure(input)?;
        crate::distribution_check::measure_test(self.cas, function.logical_id(), self.generation)?;
        let terms =
            crate::distribution_linear::terms(self.cas, input.logical_id(), &[], self.generation)?;
        if terms.len() == 1
            && terms.values().any(|(term, coefficient)| {
                term.as_ref() == Some(input) && *coefficient == crate::number::Rational::one()
            })
        {
            return self.pair_atom(input, function, mode);
        }
        self.generation
            .specialized(BudgetKind::PairingSteps, 1, true, AT)?;
        let pair = self.pair_node(input, function, mode)?;
        let mut premises = self.premises.clone();
        if let Some(support) = self.support_action(input, function, mode)? {
            premises.push(support);
        }
        let value = self.pair_terms(&terms, function, mode, &mut premises)?;
        let proof = self.push(
            Primitive::DistLinear,
            DistributionStatement::Pairing {
                pairing: pair,
                value: value.clone(),
                space: self.space.clone(),
            },
            &premises,
            Certificate::None,
        )?;
        Ok((value, proof))
    }

    /// Collect separately checked atom values under the original rational coefficients.
    fn pair_terms(
        &mut self,
        terms: &crate::distribution_linear::Terms,
        function: &Handle,
        mode: &ActionMode,
        premises: &mut Vec<Handle>,
    ) -> Result<Handle, Fault> {
        let mut output = crate::distribution_linear::Terms::new();
        for (term, coefficient) in terms.values() {
            let term = term.as_ref().ok_or_else(invalid)?;
            let (value, proof) = self.pair_atom(term, function, mode)?;
            premises.push(proof);
            let values = crate::distribution_linear::terms(
                self.cas,
                value.logical_id(),
                &[],
                self.generation,
            )?;
            for (key, (atom, weight)) in values {
                crate::distribution_linear::accumulate(
                    &mut output,
                    key,
                    atom,
                    coefficient.mul(&weight, self.generation)?,
                    self.generation,
                )?;
            }
        }
        crate::distribution_terms::linear(
            self.cas,
            &self.header,
            &self.space,
            &output,
            true,
            self.generation,
        )
    }

    /// Construct the distinct ordinary-test and supported-germ actions.
    fn pair_node(
        &mut self,
        input: &Handle,
        function: &Handle,
        mode: &ActionMode,
    ) -> Result<Handle, Fault> {
        let operator = match mode {
            ActionMode::Test => Operator::Pair,
            ActionMode::NearSupport { .. } => Operator::PairNearSupport,
        };
        self.node(operator, &[input.clone(), function.clone()])
    }

    /// A supported action reconstructs all support points before accepting a germ.
    fn support_action(
        &mut self,
        input: &Handle,
        function: &Handle,
        mode: &ActionMode,
    ) -> Result<Option<Handle>, Fault> {
        let ActionMode::NearSupport { neighborhood } = mode else {
            return Ok(None);
        };
        self.generation
            .specialized(BudgetKind::PairingSteps, 1, true, AT)?;
        let points =
            crate::distribution_support::support(self.cas, input.logical_id(), self.generation)?;
        let premises = self.premises.clone();
        self.push(
            Primitive::DistSupportAction,
            DistributionStatement::SupportAction {
                distribution: input.clone(),
                function: function.clone(),
                neighborhood: neighborhood.clone(),
                space: self.space.clone(),
            },
            &premises,
            Certificate::Distribution(Box::new(DistributionCertificate::Support(points))),
        )
        .map(Some)
    }

    /// Each atom uses its own closed action rule, preserving exact ordinary-test syntax.
    fn pair_atom(
        &mut self,
        input: &Handle,
        function: &Handle,
        mode: &ActionMode,
    ) -> Result<(Handle, Handle), Fault> {
        let pair = self.pair_node(input, function, mode)?;
        let mut premises = self.premises.clone();
        if let Some(support) = self.support_action(input, function, mode)? {
            premises.push(support);
        }
        self.generation
            .specialized(BudgetKind::PairingSteps, 1, true, AT)?;
        let (rule, value) = self.atom_value(input, function, mode, &pair)?;
        let proof = self.push(
            rule,
            DistributionStatement::Pairing {
                pairing: pair,
                value: value.clone(),
                space: self.space.clone(),
            },
            &premises,
            Certificate::None,
        )?;
        Ok((value, proof))
    }

    /// Propose the defining action for one admitted distribution constructor.
    fn atom_value(
        &mut self,
        input: &Handle,
        function: &Handle,
        mode: &ActionMode,
        pair: &Handle,
    ) -> Result<(Primitive, Handle), Fault> {
        let source = self.cas.state.expression(input.logical_id())?.clone();
        Ok(match source.operator {
            Operator::Delta { .. } => (
                Primitive::DistDeltaPair,
                crate::distribution_action::delta_value(
                    self.cas,
                    pair,
                    &self.space,
                    self.generation,
                )?,
            ),
            Operator::RegularEmbed if *mode == ActionMode::Test => {
                let scalar = self
                    .cas
                    .handle(*source.operands.first().ok_or_else(invalid)?);
                (
                    Primitive::DistRegular,
                    self.node(Operator::RegularAction, &[scalar, function.clone()])?,
                )
            }
            Operator::SmoothMultiply if *mode == ActionMode::Test => {
                let multiplier = self
                    .cas
                    .handle(*source.operands.first().ok_or_else(invalid)?);
                let distribution = self
                    .cas
                    .handle(*source.operands.get(1).ok_or_else(invalid)?);
                let product = self.node(Operator::TestMultiply, &[multiplier, function.clone()])?;
                (
                    Primitive::DistSmooth,
                    self.node(Operator::Pair, &[distribution, product])?,
                )
            }
            Operator::DistributionDerivative { order: 1 } if *mode == ActionMode::Test => {
                let distribution = self
                    .cas
                    .handle(*source.operands.first().ok_or_else(invalid)?);
                let differentiated = self.node(
                    Operator::TestDerivative { order: 1 },
                    std::slice::from_ref(function),
                )?;
                let action = self.node(Operator::Pair, &[distribution, differentiated])?;
                (
                    Primitive::DistDerivative,
                    self.node(Operator::Neg, &[action])?,
                )
            }
            _ => return Err(Fault::invalid(InvalidCode::PendingPremise, AT)),
        })
    }

    /// A smooth delta product expands into the independently checked signed binomial sum.
    pub(crate) fn multiply(
        &mut self,
        function: &Handle,
        input: &Handle,
    ) -> Result<(Handle, Handle), Fault> {
        self.ensure(input)?;
        let smooth = self.push(
            Primitive::DistSmooth,
            DistributionStatement::Regularity {
                function: function.clone(),
                space: self.space.clone(),
                property: crate::distribution_model::RegularityKind::Smooth,
            },
            &[],
            Certificate::None,
        )?;
        let product = self.node(Operator::SmoothMultiply, &[function.clone(), input.clone()])?;
        let output =
            crate::distribution_multiply::expand(self.cas, &product, &self.space, self.generation)?;
        let mut premises = self.premises.clone();
        premises.push(smooth);
        let proof = self.push(
            Primitive::DistDeltaMultiply,
            DistributionStatement::Equality {
                left: product,
                right: output.clone(),
                space: self.space.clone(),
            },
            &premises,
            Certificate::None,
        )?;
        Ok((output, proof))
    }
}
