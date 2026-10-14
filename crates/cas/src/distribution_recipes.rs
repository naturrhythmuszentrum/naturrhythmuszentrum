// SPDX-License-Identifier: AGPL-3.0-only
//! Derived distribution recipes; none of these names is a primitive registry key.

use crate::boundary_model::{CaseName, NamedCase};
use crate::distribution_model::DistributionStatement;
use crate::distribution_producer::Runner;
use crate::distribution_syntax::invalid;
use crate::error::Fault;
use crate::graph::Handle;
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::Certificate;
use crate::rule::Primitive;

impl Runner<'_> {
    /// Reuse only an older exact typed equality, retaining its primitive expansion and cost.
    pub(crate) fn named_route(
        &mut self,
        input: &Handle,
        order: u32,
        known: &[crate::distribution_known::Known],
    ) -> Result<(Handle, Handle), Fault> {
        let formal = self.node(
            Operator::DistributionDerivative { order },
            std::slice::from_ref(input),
        )?;
        for route in known {
            if route.left != formal.logical_id()
                || route.judgment.space.as_ref() != Some(&self.space)
            {
                continue;
            }
            let output = self.cas.handle(route.right);
            let expansion = self.cas.handle(route.proof);
            self.plan.account_native(route.cost, self.generation)?;
            let proof = self.push(
                Primitive::DistDerivative,
                DistributionStatement::Derivative {
                    distribution: input.clone(),
                    derivative: output.clone(),
                    space: self.space.clone(),
                    order,
                },
                &[expansion],
                Certificate::None,
            )?;
            return Ok((output, proof));
        }
        self.named_derivative(input, order)
    }

    /// Named derivatives expand to jump, finite-exception transport and ordinary composition.
    pub(crate) fn named_derivative(
        &mut self,
        input: &Handle,
        order: u32,
    ) -> Result<(Handle, Handle), Fault> {
        let (mut output, mut proof) = self.named_once(input)?;
        for level in 2..=order {
            let (next, step) = self.named_once(&output)?;
            proof = self.push(
                Primitive::DistDerivative,
                DistributionStatement::Derivative {
                    distribution: input.clone(),
                    derivative: next.clone(),
                    space: self.space.clone(),
                    order: level,
                },
                &[proof, step],
                Certificate::None,
            )?;
            output = next;
        }
        Ok((output, proof))
    }

    /// Abs and ramp retain named first derivatives through an explicit almost-everywhere bridge.
    fn named_once(&mut self, input: &Handle) -> Result<(Handle, Handle), Fault> {
        let value = self.cas.state.expression(input.logical_id())?;
        let step = value.operator == Operator::RegularEmbed
            && value.operands.first().is_some_and(|id| self.cas.state.expression(*id).is_ok_and(|function|
                matches!(&function.operator, Operator::NamedCase(name) if name.name == CaseName::Step)));
        let (output, proof) = if step {
            self.delta_primitive(input)?
        } else {
            self.once(input)?
        };
        let Some(name) = named_target(self.cas, input)? else {
            return Ok((output, proof));
        };
        let domain = self
            .cas
            .handle(self.cas.state.space(self.space.logical_id())?.domain);
        let representative = self.cas.named_case(&domain, &name, self.generation)?;
        let (target, _) = self.embed(&representative)?;
        let source = crate::distribution_ae::representative(self.cas, &output)?;
        let certificate = crate::distribution_ae::certificate(
            self.cas,
            &source,
            &representative,
            &self.space,
            self.generation,
        )?;
        let (source_fact, _) = self.regular(&source)?;
        let (target_fact, _) = self.regular(&representative)?;
        let equality = self.push(
            Primitive::DistAe,
            DistributionStatement::Equality {
                left: output,
                right: target.clone(),
                space: self.space.clone(),
            },
            &[source_fact, target_fact],
            Certificate::Distribution(Box::new(certificate)),
        )?;
        let transported = self.push(
            Primitive::DistDerivative,
            DistributionStatement::Derivative {
                distribution: input.clone(),
                derivative: target.clone(),
                space: self.space.clone(),
                order: 1,
            },
            &[proof, equality],
            Certificate::None,
        )?;
        Ok((target, transported))
    }

    /// The delta-primitive recipe is exactly the first-order checked step jump equation.
    pub(crate) fn delta_primitive(&mut self, input: &Handle) -> Result<(Handle, Handle), Fault> {
        self.once(input)
    }

    /// Integration by parts uses the defining signed test action and its replayed expansion.
    pub(crate) fn integration_by_parts(
        &mut self,
        input: &Handle,
        test: &Handle,
    ) -> Result<(Handle, Handle), Fault> {
        self.pair(input, test, &crate::task::ActionMode::Test)
    }

    /// Zero jumps bridge a regular classical derivative through the same first-order jump rule.
    pub(crate) fn classical_bridge(
        &mut self,
        input: &Handle,
    ) -> Result<Option<(Handle, Handle)>, Fault> {
        let function = crate::distribution_ae::representative(self.cas, input)?;
        let definition = self.cas.state.space(self.space.logical_id())?.clone();
        let mut classical = None;
        for proof in &self.premises {
            let application =
                crate::proof_view::application(self.cas, proof.logical_id(), self.generation)?;
            if let Some(derivative) = classical_endpoint(
                &application.judgment.statement,
                &function,
                &definition.coordinate,
                definition.domain,
            ) {
                classical = Some((derivative, proof.clone()));
                break;
            }
        }
        let Some((derivative, classical)) = classical else {
            return Ok(None);
        };
        let (source, certificate) = self.regular(&function)?;
        if !crate::distribution_jump::impulses(&certificate, self.generation)?.is_empty() {
            return Err(invalid());
        }
        let (target, _) = self.regular(&derivative)?;
        let output = self.node(Operator::RegularEmbed, &[derivative])?;
        let proof = self.push(
            Primitive::DistJump,
            DistributionStatement::Derivative {
                distribution: input.clone(),
                derivative: output.clone(),
                space: self.space.clone(),
                order: 1,
            },
            &[source, target, classical],
            Certificate::None,
        )?;
        Ok(Some((output, proof)))
    }
}

/// Accept either existing classical theorem family only on the complete exact interval.
fn classical_endpoint(
    statement: &crate::proof_model::Statement,
    function: &Handle,
    coordinate: &str,
    domain: crate::graph::LogicalId,
) -> Option<Handle> {
    let (actual, derivative, variable, region) = match statement {
        crate::proof_model::Statement::DerivativeOn {
            function,
            derivative,
            coordinate,
            domain,
        } => (function, derivative, coordinate, domain),
        crate::proof_model::Statement::CaseDerivative(value) if value.order == 1 => (
            &value.function,
            &value.derivative,
            &value.coordinate,
            &value.domain,
        ),
        _ => return None,
    };
    (actual == function && variable == coordinate && region.logical_id() == domain)
        .then(|| derivative.clone())
}

/// Name recognition supplies an expanded candidate, never a trusted analytic shortcut.
fn named_target(cas: &crate::graph::Cas, input: &Handle) -> Result<Option<NamedCase>, Fault> {
    let value = cas.state.expression(input.logical_id())?;
    if value.operator != Operator::RegularEmbed {
        return Ok(None);
    }
    let function = cas
        .state
        .expression(*value.operands.first().ok_or_else(invalid)?)?;
    let Operator::NamedCase(name) = &function.operator else {
        return Ok(None);
    };
    let (family, parameter) = match name.name {
        CaseName::Abs => (CaseName::Sign, None),
        CaseName::Ramp => (CaseName::Step, Some(Rational::zero())),
        _ => return Ok(None),
    };
    Ok(Some(NamedCase {
        name: family,
        coordinate: name.coordinate.clone(),
        parameter,
    }))
}

/// Both named and expanded producers are eligible before their completed proofs are compared.
pub(crate) fn named(cas: &crate::graph::Cas, input: &Handle) -> Result<bool, Fault> {
    let value = cas.state.expression(input.logical_id())?;
    if value.operator != Operator::RegularEmbed {
        return Ok(false);
    }
    Ok(matches!(
        cas.state
            .expression(*value.operands.first().ok_or_else(invalid)?)?
            .operator,
        Operator::NamedCase(_)
    ))
}
