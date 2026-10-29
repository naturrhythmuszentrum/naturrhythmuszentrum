// SPDX-License-Identifier: AGPL-3.0-only
//! Portable reuse identity is a canonical retained judgment, never a handle import mechanism.

use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, size};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, Statement, VerifiedWitness,
};
use crate::rule::{PRIMITIVES, Primitive};
use crate::wire_build::{Entity, envelope, indexed_table};
use std::collections::BTreeSet;

/// Exact portable bytes covering the original relation, declarations and every retained guard.
///
/// Equality compares portable contents only. The private owner still rejects use in
/// another store; that consumer must recompute its own key from authenticated handles.
/// This value provides identity, not a proof that its retained guards hold numerically.
#[derive(Clone, Debug)]
pub struct ReuseKey {
    /// Authenticated proof root in the producing store, absent from the portable bytes.
    owner: Handle,
    /// Existing canonical record table framed by the complete kernel revision manifest.
    bytes: Vec<u8>,
}

impl ReuseKey {
    /// Borrow the complete collision-free canonical identity, without a lossy digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

impl PartialEq for ReuseKey {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl Eq for ReuseKey {}

impl Cas {
    /// Key an exact expression or defined reading under its complete context and domains.
    ///
    /// Reflexivity reconstructs original definedness requirements without normalizing
    /// away a denominator. Conditional guards remain in the key. The checked identity
    /// and any reconstructed predicate syntax are admitted atomically on success.
    /// # Errors
    /// Refuses foreign handles, incompatible contexts, refuted domains and exhausted
    /// cumulative proof, canonical-table, staged-payload or output resources.
    pub fn expression_reuse_key(
        &mut self,
        context: &Handle,
        expression: &Handle,
        budget: &mut Budget,
    ) -> Result<ReuseKey, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                cas.state.expression(cas.checked(expression)?)?;
                let step = ApplicationDraft {
                    rule: Primitive::Reflexivity,
                    context: context.clone(),
                    conclusion: Statement::Equality {
                        left: expression.clone(),
                        right: expression.clone(),
                        domain: cas.admissible_region(context)?,
                        domain_contract: DomainContract::DefinedAndEqual,
                    },
                    premises: Vec::new(),
                    substitutions: Vec::new(),
                    certificate: Certificate::None,
                };
                let witness = cas.check_proof(context, &[step], budget)?;
                cas.witness_reuse_key(&witness, budget)
            })
        })
    }

    /// Key a checked original relation with all its retained requirements and used hypotheses.
    ///
    /// Proof derivation order is excluded. Exact representation syntax, declaration
    /// snapshots, context/domain/stratum, denominator guards and every compiled rule
    /// revision participate through the existing canonical record machinery.
    /// # Errors
    /// Refuses foreign or altered witness caches and exhausted cumulative resources.
    pub fn witness_reuse_key(
        &self,
        witness: &VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<ReuseKey, Fault> {
        self.inventory_limits(budget, |budget| {
            let id = self.checked(witness.proof())?;
            let application = crate::proof_view::application(self, id, budget)?;
            authenticate(self, witness, &application, budget)?;
            self.reuse_application(witness.proof(), &application, budget)
        })
    }

    /// Authenticate a local key and recompute its complete portable contents.
    ///
    /// Equal portable bytes from another store still refuse as `ForeignHandle`.
    /// A consumer there recomputes its own key before comparing the public byte views.
    /// # Errors
    /// Refuses foreign owners, changed key contents and exhausted cumulative resources.
    pub fn validate_reuse_key(&self, key: &ReuseKey, budget: &mut Budget) -> Result<(), Fault> {
        self.inventory_limits(budget, |budget| {
            let id = self.checked(&key.owner)?;
            let retained = size(key.bytes.len(), AT)?.saturating_add(64);
            let previous = budget.reserve_temporary(retained, AT)?;
            let result = (|| {
                let application = crate::proof_view::application(self, id, budget)?;
                let actual = self.reuse_application(&key.owner, &application, budget)?;
                budget.work(size(key.bytes.len().max(actual.bytes.len()), AT)?, AT)?;
                if key.bytes != actual.bytes {
                    return Err(invalid(InvalidCode::ReuseKeyMismatch));
                }
                Ok(())
            })();
            budget.restore_limits(previous);
            result
        })
    }

    /// Retain all obligations, including discharged denominators, as canonical guard records.
    fn reuse_application(
        &self,
        owner: &Handle,
        application: &CheckedApplication,
        budget: &mut Budget,
    ) -> Result<ReuseKey, Fault> {
        let retained = application_bytes(application, budget)?;
        let previous = budget.reserve_temporary(retained, AT)?;
        let result = (|| {
            let mut judgment = application.judgment.clone();
            let mut guards = application.conditions.requirements.clone();
            guards.extend_from_slice(&judgment.residuals);
            judgment.residuals = crate::statement::union(self, &guards, budget)?;
            let root =
                Entity::conclusion(None, judgment, application.conditions.hypotheses.clone());
            let (records, root, rules, assigned) = indexed_table(self, root, budget)?;
            let meter = super::inventory_budget::table_meter(&records, &assigned, budget)?;
            budget.work(size(PRIMITIVES.len(), AT)?.saturating_mul(64), AT)?;
            let revisions: BTreeSet<_> = PRIMITIVES.into_iter().collect();
            let bytes = meter.run(budget, |budget| {
                let body = envelope(records, root, &rules, b"NRZJDG04", budget)?;
                let previous = budget.reserve_temporary(size(body.len(), AT)?, AT)?;
                let result = envelope(
                    std::iter::once(body.as_slice()),
                    0,
                    &revisions,
                    b"NRZREU04",
                    budget,
                );
                budget.restore_limits(previous);
                result
            })?;
            budget.check(BudgetKind::OutputBytes, size(bytes.len(), AT)?, AT)?;
            Ok(ReuseKey {
                owner: owner.clone(),
                bytes,
            })
        })();
        budget.restore_limits(previous);
        result
    }
}

/// Serialized typed metadata bounds its clone and the original retained application together.
fn application_bytes(application: &CheckedApplication, budget: &mut Budget) -> Result<u64, Fault> {
    let mut out = Encoder::new(budget, AT);
    crate::statement::judgment(&application.judgment, &mut out)?;
    for guard in &application.conditions.requirements {
        crate::statement::guard(guard, &mut out, crate::statement::handle)?;
    }
    for hypothesis in &application.conditions.hypotheses {
        crate::statement::handle(&hypothesis.context, &mut out)?;
        out.unsigned(u64::from(hypothesis.slot))?;
    }
    let bytes = size(out.finish().len(), AT)?
        .saturating_mul(8)
        .saturating_add(512);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    Ok(bytes)
}

/// Authenticate all cached witness views against the graph-owned application and canonical export.
fn authenticate(
    cas: &Cas,
    witness: &VerifiedWitness,
    application: &CheckedApplication,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut out = Encoder::new(budget, AT);
    crate::statement::judgment(witness.judgment(), &mut out)?;
    let length = size(out.finish().len(), AT)?;
    budget.work(
        length.saturating_add(size(witness.hypotheses.len(), AT)?),
        AT,
    )?;
    let same_judgment = application.judgment == witness.judgment;
    let same_hypotheses = application.conditions.hypotheses == witness.hypotheses;
    if !same_judgment || !same_hypotheses {
        return Err(invalid(InvalidCode::DependencyWitness));
    }
    let actual = crate::wire_build::encode(cas, cas.checked(witness.proof())?, budget)?;
    budget.work(size(actual.len().max(witness.bytes.len()), AT)?, AT)?;
    if actual != witness.bytes {
        return Err(invalid(InvalidCode::DependencyWitness));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Limits;
    use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
    use crate::graph::{Draft, ExpressionDraft, Reference};
    use crate::model::{AtomicSort, Environment, Field, Operator, Sort};
    use crate::number::Rational;

    /// Altered private key contents cannot authenticate against a retained local proof.
    /// ´test:unit:reuse-key-rejects-altered-bytes´
    #[test]
    fn reuse_key_rejects_altered_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let mut cas = Cas::new(Limits::default())?;
        let mut budget = Budget::new(Limits::default())?;
        let handles = cas.admit(
            &[
                Draft::Header(HeaderDraft::Core(Environment::default())),
                Draft::Region(RegionDraft {
                    header: Reference::Local(0),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Assignments,
                    expression: RegionExpr::Ambient,
                }),
                Draft::Context(ContextDraft::Root {
                    header: Reference::Local(0),
                    admissible: Reference::Local(1),
                    assumptions: Vec::new(),
                }),
                Draft::Expression(ExpressionDraft {
                    operator: Operator::Rational(Rational::one()),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                    header: Reference::Local(0),
                    scope: Vec::new(),
                    binding: None,
                    space: None,
                    operands: Vec::new(),
                }),
            ],
            &mut budget,
        )?;
        let context = handles.get(2).ok_or("context")?;
        let expression = handles.get(3).ok_or("expression")?;
        let mut key = cas.expression_reuse_key(context, expression, &mut budget)?;
        key.bytes.push(0);
        assert!(matches!(
            cas.validate_reuse_key(&key, &mut budget),
            Err(Fault {
                kind: crate::error::FaultKind::Invalid(InvalidCode::ReuseKeyMismatch),
                ..
            })
        ));
        Ok(())
    }
}
