// SPDX-License-Identifier: AGPL-3.0-only
//! Quantity coordinate derivatives retain physical originals and composed output measures.

mod power;
mod quotient;
mod root;

use super::{
    CheckedSensitivity, JacobianEntry, JacobianEvidence, SensitivityEntry, SensitivityError,
    SensitivityKernel, SensitivityRecipe, graph::Graph, native, replay,
};
use crate::{BinaryOp, CheckedDeclarations, CheckedExpression, Context, PairingWitness};
use nrz_cas::{limits::LimitsOverride, source::OriginalForm, syntax::Operator};

impl Context {
    /// Check the ordered product and its coordinate partials `(y, x)`.
    ///
    /// The two distinct declared keys name independent symbolic coordinates. Numerical
    /// operands may share statistical sources; that relation belongs to the joint model.
    /// Original physical admission precedes differentiation of the stored-coordinate
    /// law. The output retains the exact product measure, including its reporting name.
    /// # Errors
    /// Refuses invalid originals, other syntax, wrong coordinate order, unsupported
    /// physical rules, incomplete replayed identities and exhausted finite budgets.
    pub fn check_quantity_product_sensitivity(
        self,
        original: &OriginalForm,
        coordinates: &[String],
        limits: &LimitsOverride,
    ) -> Result<CheckedSensitivity, SensitivityError> {
        let checks = self.check_all()?;
        let inputs = native::coordinate_checks(&self, &checks, coordinates)?;
        let original = self.check(original)?;
        let mut graph = Graph::new(&self, limits)?;
        select(&mut graph, &original, coordinates)?;
        let output = self.registry.coordinate_binary(
            BinaryOp::Multiply,
            inputs[0].descriptor(),
            inputs[1].descriptor(),
            &mut graph.budget,
        )?;
        let (jacobian, entries) = lower(&mut graph, &original, &inputs, &output, coordinates)?;
        drop(graph);
        Ok(CheckedSensitivity {
            declarations: CheckedDeclarations::new(self, checks),
            recipe: SensitivityRecipe::QuantityProduct,
            coordinates: coordinates.to_vec(),
            originals: vec![original],
            auxiliary: Vec::new(),
            inputs: inputs
                .iter()
                .map(|input| input.descriptor().clone())
                .collect(),
            outputs: vec![output],
            jacobian,
            entries,
            obligations: Vec::new(),
        })
    }
}

/// Only the two ordered declared operands of the original product authorize this law.
fn select(
    graph: &mut Graph<'_>,
    original: &CheckedExpression,
    coordinates: &[String],
) -> Result<(), SensitivityError> {
    if coordinates.len() != 2 {
        return Err(SensitivityError::Coordinates);
    }
    let syntax = &original.original().syntax;
    if graph.cas.expression(syntax, &mut graph.budget)?.0 != &Operator::Mul {
        return Err(SensitivityError::UnsupportedRecipe);
    }
    let operands = graph.cas.operands(syntax, &mut graph.budget)?;
    if operands.len() != coordinates.len() {
        return Err(SensitivityError::UnsupportedRecipe);
    }
    for (operand, key) in operands.iter().zip(coordinates) {
        if graph.cas.expression(operand, &mut graph.budget)?.0 != &Operator::Free(key.clone()) {
            return Err(SensitivityError::Coordinates);
        }
    }
    Ok(())
}

/// Independently replay each scalar partial and its identity to the other coordinate.
fn lower(
    graph: &mut Graph<'_>,
    original: &CheckedExpression,
    inputs: &[CheckedExpression],
    output: &crate::Descriptor,
    coordinates: &[String],
) -> Result<(JacobianEvidence, Vec<SensitivityEntry>), SensitivityError> {
    graph.extent(1, coordinates.len())?;
    let mut witnesses = Vec::new();
    let mut derivatives = Vec::new();
    let mut entries = Vec::new();
    for (column, (input, key)) in inputs.iter().zip(coordinates).enumerate() {
        let other = 1 - column;
        let witness = graph.coordinate_partial(original, key)?;
        let expression = replay::coordinate_derivative(graph, &witness, original, key)?;
        let identity = graph.equal(&expression, &inputs[other].original().syntax)?;
        let pairing = PairingWitness::sensitivity(output, input.descriptor())?;
        derivatives.push(JacobianEntry {
            expression,
            witness_index: witnesses.len(),
            pairing: pairing.clone(),
            identity,
        });
        witnesses.push(witness);
        entries.push(SensitivityEntry {
            pairing,
            kernel: SensitivityKernel::QuantityProduct { coordinate: other },
        });
    }
    Ok((
        JacobianEvidence {
            originals: vec![original.clone()],
            coordinates: coordinates.to_vec(),
            witnesses,
            work: graph.work,
            entries: derivatives,
        },
        entries,
    ))
}

impl CheckedSensitivity {
    /// Check reuse against the complete retained non-Work symbolic policy.
    ///
    /// Work bounds the current lookup and numerical call, rather than identifying a
    /// previously completed proof. Every other ceiling, including replay, must match.
    /// # Errors
    /// Refuses an exhausted caller ledger before copying or comparing policy words.
    pub fn matches_symbolic_policy(
        &self,
        budget: &mut crate::Budget,
    ) -> Result<bool, SensitivityError> {
        Ok(self.declarations.matches_symbolic_policy(budget)?)
    }

    /// Prepare one exact quantity-product identity on the original caller Work ledger.
    ///
    /// Context admission, original checking, construction, generation and replay retain
    /// spent Work on refusal. Each service receives disjoint generation/replay shares
    /// of remaining Work; no private default allowance replenishes the caller budget.
    /// # Errors
    /// Preserves physical, original, symbolic, finite-resource and replay refusals.
    pub fn quantity_product(
        registry: &crate::Registry,
        inputs: [&crate::Descriptor; 2],
        ledger: &mut crate::Budget,
    ) -> Result<Self, SensitivityError> {
        let mut builder = Context::quantity_context(registry.clone(), &["x", "y"], ledger)?;
        for (key, descriptor) in ["x", "y"].into_iter().zip(inputs) {
            builder.declare(
                key.into(),
                crate::Declaration::Input {
                    descriptor: descriptor.clone(),
                    nonzero: false,
                    evidence: crate::Evidence::default(),
                },
            )?;
        }
        let original = builder.parse_cumulative("quantity product derivative", "x*y", ledger)?;
        let checks = builder.check_all_cumulative(ledger)?;
        let coordinates = [String::from("x"), String::from("y")];
        let inputs = native::coordinate_checks(&builder, &checks, &coordinates)?;
        let original = builder.check_cumulative(&original, ledger)?;
        let mut graph = Graph::cumulative(&builder, ledger)?;
        let result = (|| {
            select(&mut graph, &original, &coordinates)?;
            let output = builder.registry.coordinate_binary(
                BinaryOp::Multiply,
                inputs[0].descriptor(),
                inputs[1].descriptor(),
                &mut graph.budget,
            )?;
            let (jacobian, entries) = lower(&mut graph, &original, &inputs, &output, &coordinates)?;
            Ok::<_, SensitivityError>((output, jacobian, entries))
        })();
        graph.return_ledger(ledger);
        drop(graph);
        let (output, jacobian, entries) = result?;
        Ok(Self {
            declarations: CheckedDeclarations::new(builder, checks),
            recipe: SensitivityRecipe::QuantityProduct,
            coordinates: coordinates.into(),
            originals: vec![original],
            auxiliary: Vec::new(),
            inputs: inputs
                .iter()
                .map(|input| input.descriptor().clone())
                .collect(),
            outputs: vec![output],
            jacobian,
            entries,
            obligations: Vec::new(),
        })
    }
}

/// Debit exact exposed checker usage, or the reserved checker share when no receipt is exposed.
pub(super) fn service_work(
    outcome: &nrz_cas::request::Outcome,
    reserved: u64,
) -> Result<u64, SensitivityError> {
    use nrz_cas::request::Outcome;
    let (generation, replay) = match outcome {
        Outcome::Proved { usage, result, .. }
        | Outcome::Conditional { usage, result, .. }
        | Outcome::Nonexistent { usage, result, .. }
        | Outcome::Partial { usage, result, .. } => (usage.work, result.replay_usage.work),
        Outcome::Exhausted {
            usage,
            replay_usage,
            ..
        } => (usage.work, replay_usage.work),
        Outcome::UnknownTotality { usage, partial, .. }
        | Outcome::PendingPremise { usage, partial, .. } => (
            usage.work,
            partial
                .as_ref()
                .map_or(reserved, |result| result.replay_usage.work),
        ),
        Outcome::Solved { usage, .. }
        | Outcome::SolvePartial { usage, .. }
        | Outcome::Unresolved { usage, .. }
        | Outcome::Unsupported { usage, .. }
        | Outcome::Refused { usage, .. }
        | Outcome::Invalid { usage, .. } => (usage.work, reserved),
    };
    generation
        .checked_add(replay)
        .ok_or(SensitivityError::ResourceLimit)
}
