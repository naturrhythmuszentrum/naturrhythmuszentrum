// SPDX-License-Identifier: AGPL-3.0-only
//! The named minus-one recipe expands into existing checked applications.

use super::{defined, definedness, invalid, value_term};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::ContextDraft;
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{AssumptionDraft, Cas, Draft, Handle, Reference, Vertex};
use crate::model::{Constant, Operator};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, ConstructorLaw, DomainContract, Statement, VerifiedWitness,
};
use crate::rule::Primitive;

/// Recipe construction and aggregate premise workspace share this attribution.
const AT: Location = Location::new("trig.atan-minus-one recipe");

/// Checked expansion or an unsupported derivation capability, never a negative theorem.
#[derive(Clone, Debug)]
pub enum TrigRecipeOutcome {
    /// The complete primitive expansion retains every original condition.
    Proved(Box<VerifiedWitness>),
    /// Recognition or lossless scalar condition transport is not supported.
    /// No candidate graph records survive this outcome.
    Unresolved,
}

/// Constant-size recipe syntax retains the original operator frame through metered admission.
struct Route<'a> {
    /// Candidate store inside the enclosing atomic recipe transaction.
    cas: &'a mut Cas,
    /// Shared monotone allowance for admission, checking and publication.
    budget: &'a mut Budget,
    /// Original arctangent application and source of all output metadata.
    function: Handle,
    /// Original premise context retained after local closure.
    context: Handle,
    /// Exact original region restored after local closure.
    domain: Handle,
}

impl Route<'_> {
    /// Admit a bounded constructor with original scope copies prepaid by the shared builder.
    fn term(&mut self, operator: Operator, operands: Vec<Handle>) -> Result<Handle, Fault> {
        value_term(self.cas, &self.function, operator, operands, self.budget)
    }

    /// Each input-controlled proof copy is prepaid and its aggregate workspace stays live.
    fn push(
        &mut self,
        context: &Handle,
        rule: Primitive,
        conclusion: Statement,
        premises: &[Handle],
        certificate: Certificate,
    ) -> Result<Handle, Fault> {
        let mut bytes = 0_u64;
        for proof in premises {
            let proof_id = self.cas.checked(proof, self.budget)?;
            bytes = bytes
                .checked_add(crate::proof_view::application_bytes(
                    self.cas,
                    proof_id,
                    self.budget,
                )?)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        }
        self.budget.with_retained_payload(bytes, AT, |budget| {
            let mut earlier = Vec::new();
            earlier
                .try_reserve_exact(premises.len())
                .map_err(|_| Fault::allocation(AT))?;
            for proof in premises {
                earlier.push(crate::proof_view::application(
                    self.cas,
                    self.cas.checked(proof, budget)?,
                    budget,
                )?);
            }
            let mut draft = ApplicationDraft {
                context: context.clone(),
                rule,
                conclusion,
                premises: (0..premises.len())
                    .map(|n| u32::try_from(n).map_err(|_| invalid()))
                    .collect::<Result<Vec<_>, _>>()?,
                substitutions: Vec::new(),
                certificate,
            };
            if rule == Primitive::CaseDomain {
                let (evidence, _) =
                    crate::case_check::reconstruct(self.cas, &draft, &earlier, budget)?;
                draft.certificate = Certificate::Cases(Box::new(evidence));
            } else if rule == Primitive::RegionDecide {
                draft.certificate = Certificate::Region(crate::region_check::evidence(
                    self.cas,
                    &draft.conclusion,
                    budget,
                )?);
            }
            let mut checked =
                crate::scoped_check::check_application(self.cas, &draft, &earlier, budget)?;
            checked.premises = premises.iter().map(Handle::logical_id).collect();
            self.cas.publish_scoped(checked, budget)
        })
    }

    /// Check an equality link with explicit ordered proof premises and the exact region.
    fn equality(
        &mut self,
        context: &Handle,
        domain: &Handle,
        rule: Primitive,
        endpoints: [Handle; 2],
        premises: &[Handle],
        certificate: Certificate,
    ) -> Result<Handle, Fault> {
        let [left, right] = endpoints;
        self.push(
            context,
            rule,
            Statement::Equality {
                left,
                right,
                domain: domain.clone(),
                domain_contract: DomainContract::DefinedAndEqual,
            },
            premises,
            certificate,
        )
    }

    /// Reconstruct a Defined predicate and check its named domain rule.
    fn fact(
        &mut self,
        context: &Handle,
        domain: &Handle,
        function: &Handle,
        rule: Primitive,
        premises: &[Handle],
    ) -> Result<Handle, Fault> {
        let predicate = defined::predicate(self.cas, function, self.budget)?;
        self.push(
            context,
            rule,
            Statement::PredicateFact {
                region: domain.clone(),
                predicate,
            },
            premises,
            Certificate::None,
        )
    }

    /// An empty local frame restricts parent facts without adding an undischarged assumption.
    fn local(&mut self, hypotheses: Vec<AssumptionDraft>) -> Result<Handle, Fault> {
        self.cas
            .admit(
                &[Draft::Context(ContextDraft::Local {
                    parent: Reference::Existing(self.context.clone()),
                    region: Reference::Existing(self.domain.clone()),
                    hypotheses,
                })],
                self.budget,
            )?
            .pop()
            .ok_or_else(invalid)
    }

    /// Restrict a parent proof through an explicit checked subset premise.
    fn weaken(
        &mut self,
        context: &Handle,
        proof: &Handle,
        subset: &Handle,
        statement: Statement,
    ) -> Result<Handle, Fault> {
        self.push(
            context,
            Primitive::ContextWeaken,
            statement,
            &[proof.clone(), subset.clone()],
            Certificate::None,
        )
    }

    /// All four endpoint guards are explicit totality results, deduplicated by exact identity.
    fn domains(
        &mut self,
        guard: &Handle,
        functions: &[Handle; 4],
        operands: &[Handle; 4],
    ) -> Result<(Handle, Vec<Handle>, Vec<Handle>), Fault> {
        let context = self.context.clone();
        let domain = self.domain.clone();
        let names = [
            "atan-original",
            "atan-negative-literal",
            "atan-negated-one",
            "atan-one",
        ];
        let mut predicates = Vec::new();
        let mut facts = Vec::new();
        let mut hypotheses = Vec::new();
        for (index, function) in functions.iter().enumerate() {
            let predicate = defined::predicate(self.cas, function, self.budget)?;
            if predicates.contains(&predicate) {
                continue;
            }
            let original = if index == 0 {
                guard.clone()
            } else {
                self.fact(
                    &context,
                    &domain,
                    &operands[index],
                    Primitive::CaseDomain,
                    &[],
                )?
            };
            facts.push(self.fact(
                &context,
                &domain,
                function,
                Primitive::TrigDefined,
                &[original],
            )?);
            hypotheses.push(AssumptionDraft {
                key: names[index].into(),
                condition: Reference::Existing(predicate.clone()),
            });
            predicates.push(predicate);
        }
        Ok((self.local(hypotheses)?, predicates, facts))
    }

    /// Closure cites each restricted totality fact at the same exact local region.
    fn close(
        &mut self,
        local: &Handle,
        proof: Handle,
        predicates: Vec<Handle>,
        facts: Vec<Handle>,
        output: Handle,
    ) -> Result<VerifiedWitness, Fault> {
        let parent = self.context.clone();
        let region = self.cas.admissible_region(local, self.budget)?;
        let empty = self.local(Vec::new())?;
        if self.cas.admissible_region(&empty, self.budget)? != region {
            return Err(invalid());
        }
        let construction = self.push(
            &parent,
            Primitive::RegionConstruct,
            Statement::Constructor {
                region: region.clone(),
                law: ConstructorLaw::Membership,
            },
            &[],
            Certificate::None,
        )?;
        let subset = self.push(
            &parent,
            Primitive::RegionConstruct,
            Statement::Subset {
                subset: region.clone(),
                superset: self.domain.clone(),
            },
            &[],
            Certificate::None,
        )?;
        let mut premises = vec![proof, construction.clone()];
        for (predicate, fact) in predicates.into_iter().zip(facts) {
            let statement = Statement::PredicateFact {
                region: region.clone(),
                predicate,
            };
            let restricted = self.weaken(&empty, &fact, &subset, statement.clone())?;
            premises.push(self.push(
                &parent,
                Primitive::ContextClose,
                statement,
                &[restricted, construction.clone()],
                Certificate::Close {
                    frame: empty.clone(),
                },
            )?);
        }
        let statement = Statement::Equality {
            left: self.function.clone(),
            right: output.clone(),
            domain: region.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        };
        let closed = self.push(
            &parent,
            Primitive::ContextClose,
            statement,
            &premises,
            Certificate::Close {
                frame: local.clone(),
            },
        )?;
        let back = self.push(
            &parent,
            Primitive::RegionDecide,
            Statement::Subset {
                subset: self.domain.clone(),
                superset: region,
            },
            &[],
            Certificate::None,
        )?;
        let domain = self.domain.clone();
        let result = self.equality(
            &parent,
            &domain,
            Primitive::ContextWeaken,
            [self.function.clone(), output],
            &[closed, back],
            Certificate::None,
        )?;
        self.cas.seal_scoped(&result, self.budget)
    }

    /// Congruence, parity, the one-value law and negation congruence form the complete chain.
    fn expansion(
        &mut self,
        guard: &Handle,
        equality: &Handle,
        original: Handle,
        minus: Handle,
    ) -> Result<VerifiedWitness, Fault> {
        let one = self.term(Operator::Rational(Rational::one()), Vec::new())?;
        let negative = self.term(Operator::Neg, vec![one.clone()])?;
        let at_minus = self.term(Operator::Arctangent, vec![minus.clone()])?;
        let at_negative = self.term(Operator::Arctangent, vec![negative.clone()])?;
        let at_one = self.term(Operator::Arctangent, vec![one.clone()])?;
        let neg_at_one = self.term(Operator::Neg, vec![at_one.clone()])?;
        let pi = self.term(Operator::Constant(Constant::Pi), Vec::new())?;
        let quarter = Rational::from_ratio(1, 4, self.budget)?;
        let quarter = self.term(Operator::Rational(quarter), Vec::new())?;
        let value = self.term(Operator::Mul, vec![pi, quarter])?;
        let output = self.term(Operator::Neg, vec![value.clone()])?;
        let (local, predicates, facts) = self.domains(
            guard,
            &[
                self.function.clone(),
                at_minus.clone(),
                at_negative.clone(),
                at_one.clone(),
            ],
            &[
                original.clone(),
                minus.clone(),
                negative.clone(),
                one.clone(),
            ],
        )?;
        let region = self.cas.admissible_region(&local, self.budget)?;
        let subset = self.push(
            &local,
            Primitive::RegionConstruct,
            Statement::Subset {
                subset: region.clone(),
                superset: self.domain.clone(),
            },
            &[],
            Certificate::None,
        )?;
        let original_eq = self.equality(
            &local,
            &region,
            Primitive::ContextWeaken,
            [original, minus.clone()],
            &[equality.clone(), subset],
            Certificate::None,
        )?;
        let first = self.equality(
            &local,
            &region,
            Primitive::Congruence,
            [self.function.clone(), at_minus.clone()],
            &[original_eq],
            Certificate::Congruence(vec![0]),
        )?;
        let next = self.literal_join(
            &local,
            &region,
            [negative, minus, at_minus, at_negative.clone()],
        )?;
        let one_guard = self.fact(&local, &region, &one, Primitive::CaseDomain, &[])?;
        let parity = self.equality(
            &local,
            &region,
            Primitive::AtanOdd,
            [at_negative.clone(), neg_at_one.clone()],
            std::slice::from_ref(&one_guard),
            Certificate::None,
        )?;
        let negated = self.negated_one(
            &local,
            &region,
            one_guard,
            [one, at_one, value, neg_at_one.clone(), output.clone()],
        )?;
        let mut chain = first;
        for (endpoint, link) in [
            (at_negative, next),
            (neg_at_one, parity),
            (output.clone(), negated),
        ] {
            chain = self.equality(
                &local,
                &region,
                Primitive::Transitivity,
                [self.function.clone(), endpoint],
                &[chain, link],
                Certificate::None,
            )?;
        }
        self.close(&local, chain, predicates, facts, output)
    }

    /// Checked arithmetic joins the negation constructor to the literal before congruence.
    fn literal_join(
        &mut self,
        context: &Handle,
        region: &Handle,
        endpoints: [Handle; 4],
    ) -> Result<Handle, Fault> {
        let [negative, minus, at_minus, at_negative] = endpoints;
        let literal = self.equality(
            context,
            region,
            Primitive::Exact,
            [negative.clone(), minus.clone()],
            &[],
            Certificate::None,
        )?;
        let reversed = self.equality(
            context,
            region,
            Primitive::Symmetry,
            [minus, negative],
            &[literal],
            Certificate::None,
        )?;
        self.equality(
            context,
            region,
            Primitive::Congruence,
            [at_minus, at_negative],
            &[reversed],
            Certificate::Congruence(vec![0]),
        )
    }

    /// The one-value theorem and checked negation congruence provide the last equality link.
    fn negated_one(
        &mut self,
        context: &Handle,
        region: &Handle,
        guard: Handle,
        endpoints: [Handle; 5],
    ) -> Result<Handle, Fault> {
        let [one, at_one, value, negative, output] = endpoints;
        let reflexive = self.equality(
            context,
            region,
            Primitive::Reflexivity,
            [one.clone(), one],
            &[],
            Certificate::None,
        )?;
        let special = self.equality(
            context,
            region,
            Primitive::AtanOne,
            [at_one, value],
            &[guard, reflexive],
            Certificate::None,
        )?;
        self.equality(
            context,
            region,
            Primitive::Congruence,
            [negative, output],
            &[special],
            Certificate::Congruence(vec![0]),
        )
    }
}

/// Only the typed guarded-transport capability refusal becomes an unresolved recipe.
const fn unsupported_transport(fault: &Fault) -> bool {
    matches!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::ScalarTransportUnsupported)
    )
}

impl Cas {
    /// Expand `trig.atan-minus-one@1` from original definedness and equality to minus one.
    ///
    /// The named recipe adds no primitive key. Its portable witness retains congruence,
    /// parity at one, the one-value theorem, negation congruence and explicit totality closure.
    /// Checked original equalities retain their proved requirements and unresolved residuals.
    /// Only `InvalidCode::ScalarTransportUnsupported` maps to `TrigRecipeOutcome::Unresolved`
    /// after transaction rollback; malformed evidence remains an invalid-input fault.
    /// # Errors
    /// Refuses malformed original joins and exhausted resources.
    pub fn trig_atan_minus_one(
        &mut self,
        function: &Handle,
        guard: &Handle,
        equality: &Handle,
        budget: &mut Budget,
    ) -> Result<TrigRecipeOutcome, Fault> {
        let result = self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                budget.work(1, AT)?;
                if cas
                    .state
                    .expression(cas.checked(function, budget)?, budget)?
                    .operator
                    != Operator::Arctangent
                {
                    return Ok(None);
                }
                let original = defined::operand(cas, function, budget)?;
                for premise in [guard, equality] {
                    budget.work(1, AT)?;
                    if !matches!(
                        cas.state.node(cas.checked(premise, budget)?, budget)?,
                        Vertex::Proof(_)
                    ) {
                        return Err(Fault::invalid(InvalidCode::Reference, AT));
                    }
                }
                let bytes =
                    crate::proof_view::application_bytes(cas, cas.checked(guard, budget)?, budget)?
                        .checked_add(crate::proof_view::application_bytes(
                            cas,
                            cas.checked(equality, budget)?,
                            budget,
                        )?)
                        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
                let candidate = budget.with_retained_payload(bytes, AT, |budget| {
                    let guard =
                        crate::proof_view::application(cas, cas.checked(guard, budget)?, budget)?;
                    let equality = crate::proof_view::application(
                        cas,
                        cas.checked(equality, budget)?,
                        budget,
                    )?;
                    let Statement::Equality {
                        left,
                        right,
                        domain,
                        domain_contract: DomainContract::DefinedAndEqual,
                    } = &equality.judgment.statement
                    else {
                        return Err(invalid());
                    };
                    if *left != original || guard.judgment.context != equality.judgment.context {
                        return Err(invalid());
                    }
                    definedness(cas, &original, domain, &guard.judgment.statement, budget)?;
                    let Operator::Rational(value) = &cas
                        .state
                        .expression(cas.checked(right, budget)?, budget)?
                        .operator
                    else {
                        return Ok(None);
                    };
                    value.check(budget, AT)?;
                    if *value != Rational::from_ratio(-1, 1, budget)? {
                        return Ok(None);
                    }
                    Ok(Some((
                        equality.judgment.context.clone(),
                        domain.clone(),
                        right.clone(),
                    )))
                })?;
                let Some((context, domain, minus)) = candidate else {
                    return Ok(None);
                };
                let mut route = Route {
                    cas,
                    budget,
                    function: function.clone(),
                    context,
                    domain,
                };
                route.expansion(guard, equality, original, minus).map(Some)
            })
        });
        match result {
            Ok(Some(witness)) => Ok(TrigRecipeOutcome::Proved(Box::new(witness))),
            Ok(None) => Ok(TrigRecipeOutcome::Unresolved),
            Err(fault) if unsupported_transport(&fault) => Ok(TrigRecipeOutcome::Unresolved),
            Err(fault) => Err(fault),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::unsupported_transport;
    use crate::error::{Fault, InvalidCode, Location};

    /// Classification follows the typed meaning independently of diagnostic wording.
    /// ´test:unit:trig-minus-one-transport-classification´
    #[test]
    fn trig_minus_one_transport_classification() {
        for location in [
            "scalar guarded-region transport",
            "scalar statement view",
            "diagnostic spelling",
        ] {
            let at = Location::new(location);
            assert!(unsupported_transport(&Fault::invalid(
                InvalidCode::ScalarTransportUnsupported,
                at
            )));
            assert!(!unsupported_transport(&Fault::invalid(
                InvalidCode::UnsupportedFragment,
                at
            )));
            assert!(!unsupported_transport(&Fault::invalid(
                InvalidCode::Reference,
                at
            )));
        }
    }
}
