// SPDX-License-Identifier: AGPL-3.0-only
//! Finite exact recognition and premise-checked trigonometric special values.

#![expect(
    clippy::redundant_pub_crate,
    reason = "closed statement, codec and checker dispatch share these entrypoints"
)]

use crate::budget::Budget;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, Vertex};
use crate::model::{AtomicSort, Constant, Field, Operator, PredicateKind, Sort};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};
use crate::rule::Primitive;

pub(crate) mod defined;
mod minus_one;
pub use minus_one::TrigRecipeOutcome;
pub(crate) mod parity;

/// Recompute the nonnegative residue without narrowing the original integer.
fn residue(k: &Rational, budget: &mut Budget) -> Result<Rational, Fault> {
    k.check(budget, AT)?;
    if !k.is_integer() {
        return Err(invalid());
    }
    let four = Rational::from_ratio(4, 1, budget)?;
    let q = k.div(&four, budget)?.floor(budget)?;
    k.sub(&q.mul(&four, budget)?, budget)
}

/// The table is evaluated only after exact Euclidean division of the retained integer.
fn circular_value(k: &Rational, cosine: bool, budget: &mut Budget) -> Result<Rational, Fault> {
    let r = residue(k, budget)?;
    let table = if cosine { [1, 0, -1, 0] } else { [0, 1, 0, -1] };
    for (index, value) in [0, 1, 2, 3].into_iter().zip(table) {
        if r == Rational::from_ratio(index, 1, budget)? {
            return Rational::from_ratio(value, 1, budget);
        }
    }
    Err(invalid())
}

/// Candidate selection grants no authority; the independent checker still validates every join.
fn candidate(
    cas: &Cas,
    operator: &Operator,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<Option<(Primitive, Handle, Rational)>, Fault> {
    Ok(Some(match (operator, premises) {
        (Operator::Sine | Operator::Cosine, [witness]) => {
            let Statement::HalfPiWitness { k, domain, .. } = &witness.judgment.statement else {
                return Ok(None);
            };
            let cosine = *operator == Operator::Cosine;
            (
                if cosine {
                    Primitive::CosHalfPi
                } else {
                    Primitive::SinHalfPi
                },
                domain.clone(),
                circular_value(k, cosine, budget)?,
            )
        }
        (Operator::Arctangent, [_, equality]) => {
            let Statement::Equality { right, domain, .. } = &equality.judgment.statement else {
                return Ok(None);
            };
            let constant = cas.state.expression(cas.checked(right, budget)?, budget)?;
            let Operator::Rational(value) = &constant.operator else {
                return Ok(None);
            };
            value.check(budget, AT)?;
            if value.is_zero() {
                (Primitive::AtanZero, domain.clone(), Rational::zero())
            } else if *value == Rational::one() {
                (
                    Primitive::AtanOne,
                    domain.clone(),
                    Rational::from_ratio(1, 4, budget)?,
                )
            } else {
                return Ok(None);
            }
        }
        _ => return Ok(None),
    }))
}

impl Cas {
    /// Produce an exact trigonometric equality from ordered checked proof handles.
    ///
    /// Sine and cosine consume one half-Pi witness. Arctangent consumes original
    /// definedness and equality to a literal zero or one. Missing recognition or
    /// another valid input returns `None`, without a negative mathematical claim.
    /// All qualified hypotheses and original divisor requirements remain attached.
    /// # Errors
    /// Rejects foreign handles, malformed premise joins and exhausted resources.
    pub fn trig_special_value(
        &mut self,
        function: &Handle,
        proofs: &[Handle],
        budget: &mut Budget,
    ) -> Result<Option<crate::proof_model::VerifiedWitness>, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let expression = cas
                    .state
                    .expression(cas.checked(function, budget)?, budget)?;
                budget.work(1, AT)?;
                if proofs.len() > 2 {
                    return Err(invalid());
                }
                let operator = match &expression.operator {
                    Operator::Sine => Operator::Sine,
                    Operator::Cosine => Operator::Cosine,
                    Operator::Arctangent => Operator::Arctangent,
                    _ => return Ok(None),
                };
                let mut premises = Vec::new();
                for proof in proofs {
                    premises.push(crate::proof_view::application(
                        cas,
                        cas.checked(proof, budget)?,
                        budget,
                    )?);
                }
                let Some((rule, domain, value)) = candidate(cas, &operator, &premises, budget)?
                else {
                    return Ok(None);
                };
                let context = premises
                    .first()
                    .ok_or_else(invalid)?
                    .judgment
                    .context
                    .clone();
                let mut right =
                    value_term(cas, function, Operator::Rational(value), Vec::new(), budget)?;
                if rule == Primitive::AtanOne {
                    let pi = value_term(
                        cas,
                        function,
                        Operator::Constant(Constant::Pi),
                        Vec::new(),
                        budget,
                    )?;
                    right = value_term(cas, function, Operator::Mul, vec![pi, right], budget)?;
                }
                let draft = ApplicationDraft {
                    rule,
                    context,
                    conclusion: Statement::Equality {
                        left: function.clone(),
                        right,
                        domain,
                        domain_contract: crate::proof_model::DomainContract::DefinedAndEqual,
                    },
                    premises: Vec::new(),
                    substitutions: Vec::new(),
                    certificate: Certificate::None,
                };
                let mut checked =
                    crate::scoped_check::check_application(cas, &draft, &premises, budget)?;
                checked.premises = proofs.iter().map(Handle::logical_id).collect();
                let proof = cas.publish_scoped(checked, budget)?;
                cas.seal_scoped(&proof, budget).map(Some)
            })
        })
    }
}

/// Copy only output scope metadata, after its measured copy charge and live reservation.
fn value_term(
    cas: &mut Cas,
    function: &Handle,
    operator: Operator,
    operands: Vec<Handle>,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let expression = cas
        .state
        .expression(cas.checked(function, budget)?, budget)?;
    let header = cas.handle(expression.header);
    let bytes = if expression.scope.is_empty() {
        // An empty scope owns no buffer and needs no copy or temporary workspace.
        0
    } else {
        crate::validation::scope_limit(&expression.scope, budget)?;
        let mut out = crate::encoding::Encoder::new(budget, AT);
        crate::encoding::scope(&expression.scope, &mut out)?;
        let bytes = crate::graph::size(out.finish().len(), AT)?;
        budget.work(bytes, AT)?;
        bytes
    };
    budget.with_retained_payload(bytes, AT, |budget| {
        let source = &cas
            .state
            .expression(cas.checked(function, budget)?, budget)?
            .scope;
        let mut scope = Vec::new();
        scope
            .try_reserve_exact(source.len())
            .map_err(|_| Fault::allocation(AT))?;
        for frame in source {
            let mut copied = Vec::new();
            copied
                .try_reserve_exact(frame.len())
                .map_err(|_| Fault::allocation(AT))?;
            copied.extend(frame.iter().cloned());
            scope.push(copied);
        }
        cas.admit(
            &[crate::graph::Draft::Expression(
                crate::graph::ExpressionDraft {
                    header: crate::graph::Reference::Existing(header),
                    space: None,
                    operator,
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                    scope,
                    binding: None,
                    operands: operands
                        .into_iter()
                        .map(crate::graph::Reference::Existing)
                        .collect(),
                },
            )],
            budget,
        )?
        .pop()
        .ok_or_else(invalid)
    })
}

/// Four analytic schemas reuse ordinary equality and the empty certificate grammar.
pub(crate) fn value_check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    use crate::proof_model::DomainContract;
    let Statement::Equality {
        left,
        right,
        domain,
        domain_contract: DomainContract::DefinedAndEqual,
    } = &draft.conclusion
    else {
        return Err(invalid());
    };
    if draft.certificate != Certificate::None || !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    let operator = match draft.rule {
        Primitive::SinHalfPi => Operator::Sine,
        Primitive::CosHalfPi => Operator::Cosine,
        Primitive::AtanZero | Primitive::AtanOne => Operator::Arctangent,
        _ => return Err(invalid()),
    };
    let u = cas.handle(crate::rule_check::unary(
        &cas.state,
        cas.checked(left, budget)?,
        &operator,
        budget,
    )?);
    let operand = cas.state.expression(cas.checked(&u, budget)?, budget)?;
    let output = cas.state.expression(cas.checked(right, budget)?, budget)?;
    if operand.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || output.sort != operand.sort
        || output.scope != operand.scope
        || output.ambient != operand.ambient
        || output.space.is_some()
    {
        return Err(invalid());
    }
    for premise in premises {
        if premise.judgment.context != draft.context {
            return Err(invalid());
        }
    }
    match draft.rule {
        Primitive::SinHalfPi | Primitive::CosHalfPi => {
            let [witness] = premises else {
                return Err(invalid());
            };
            let Statement::HalfPiWitness {
                u: original,
                k,
                context,
                domain: region,
            } = &witness.judgment.statement
            else {
                return Err(invalid());
            };
            if original != &u || context != &draft.context || region != domain {
                return Err(invalid());
            }
            let expected = circular_value(k, draft.rule == Primitive::CosHalfPi, budget)?;
            if output.operator != Operator::Rational(expected) {
                return Err(invalid());
            }
        }
        Primitive::AtanZero | Primitive::AtanOne => {
            let [guard, equality] = premises else {
                return Err(invalid());
            };
            definedness(cas, &u, domain, &guard.judgment.statement, budget)?;
            let Statement::Equality {
                left: original,
                right: constant,
                domain: region,
                ..
            } = &equality.judgment.statement
            else {
                return Err(invalid());
            };
            let one = draft.rule == Primitive::AtanOne;
            if original != &u
                || region != domain
                || !crate::rule_check::literal(
                    &cas.state,
                    cas.checked(constant, budget)?,
                    i64::from(one),
                    budget,
                )?
            {
                return Err(invalid());
            }
            atan_output(cas, right, one, budget)?;
        }
        _ => return Err(invalid()),
    }
    let mut conditions = ScopedConditions::default();
    for premise in premises {
        crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
    }
    Ok(conditions)
}

/// Reconstruct the exact arctangent endpoint without a transcendental evaluation.
fn atan_output(cas: &Cas, right: &Handle, one: bool, budget: &mut Budget) -> Result<(), Fault> {
    if one {
        let (pi, quarter) = crate::rule_check::binary(
            &cas.state,
            cas.checked(right, budget)?,
            &Operator::Mul,
            budget,
        )?;
        if cas.state.expression(pi, budget)?.operator != Operator::Constant(Constant::Pi)
            || cas.state.expression(quarter, budget)?.operator
                != Operator::Rational(Rational::from_ratio(1, 4, budget)?)
        {
            return Err(invalid());
        }
    } else if !crate::rule_check::literal(&cas.state, cas.checked(right, budget)?, 0, budget)? {
        return Err(invalid());
    }
    Ok(())
}

/// Recognition never supplies a trigonometric value or a numerical approximation.
const AT: Location = Location::new("integer half-Pi witness");

/// Forged recognition is malformed evidence, not a mathematical negative result.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}

/// Validate the value payload and graph roles independently of any supplied certificate.
pub(crate) fn roles(
    cas: &Cas,
    u: &Handle,
    k: &Rational,
    context: &Handle,
    domain: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    k.check(budget, AT)?;
    budget.work(1, AT)?;
    if !k.is_integer() {
        return Err(invalid());
    }
    let operand = cas.state.expression(cas.checked(u, budget)?, budget)?;
    let frame = cas.state.context(cas.checked(context, budget)?, budget)?;
    let region = cas.state.region(cas.checked(domain, budget)?, budget)?;
    if operand.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || operand.space.is_some()
        || (!region.scope.is_empty() && region.scope != operand.scope)
    {
        return Err(Fault::invalid(InvalidCode::Sort, AT));
    }
    for header in [operand.header, region.header] {
        if !crate::header::embeds(&cas.state, header, frame.header_id, budget)? {
            return Err(Fault::invalid(InvalidCode::Context, AT));
        }
    }
    Ok(())
}

/// Both inputs have already been independently checked by the ordinary backward proof pipeline.
/// Their complete conditions survive recognition, including original divisor requirements.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Statement::HalfPiWitness {
        u,
        k,
        context,
        domain,
    } = &draft.conclusion
    else {
        return Err(invalid());
    };
    let Certificate::HalfPiWitness {
        u: original,
        k: integer,
        context: home,
        domain: region,
        defined,
        equality,
    } = &draft.certificate
    else {
        return Err(invalid());
    };
    roles(cas, u, k, context, domain, budget)?;
    integer.check(budget, AT)?;
    if (u, k, context, domain) != (original, integer, home, region)
        || *context != draft.context
        || (*defined, *equality) != (0, 1)
        || !draft.substitutions.is_empty()
    {
        return Err(invalid());
    }
    let [guard, equality] = premises else {
        return Err(invalid());
    };
    for premise in premises {
        if premise.judgment.context != *context {
            return Err(invalid());
        }
    }
    definedness(cas, u, domain, &guard.judgment.statement, budget)?;
    let Statement::Equality {
        left,
        right,
        domain: equality_domain,
        ..
    } = &equality.judgment.statement
    else {
        return Err(invalid());
    };
    if left != u || equality_domain != domain {
        return Err(invalid());
    }
    coefficient(cas, u, right, k, budget)?;
    let mut conditions = ScopedConditions::default();
    for premise in premises {
        crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
    }
    Ok(conditions)
}

/// Only the original domain or the exact Defined predicate supplies the first slot.
fn definedness(
    cas: &Cas,
    u: &Handle,
    domain: &Handle,
    statement: &Statement,
    budget: &mut Budget,
) -> Result<(), Fault> {
    match statement {
        Statement::DefinedDomain {
            function,
            domain: region,
        } if function == u && region == domain => Ok(()),
        Statement::PredicateFact { region, predicate } if region == domain => {
            let Vertex::Condition(value) =
                cas.state.node(cas.checked(predicate, budget)?, budget)?
            else {
                return Err(invalid());
            };
            if value.kind != PredicateKind::Defined || value.arguments != [cas.checked(u, budget)?]
            {
                return Err(invalid());
            }
            Ok(())
        }
        _ => Err(invalid()),
    }
}

/// Reconstruct half the integer using bounded exact arithmetic, never printed digits.
fn coefficient(
    cas: &Cas,
    u: &Handle,
    right: &Handle,
    k: &Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let operand = cas.state.expression(cas.checked(u, budget)?, budget)?;
    let endpoint = cas.state.expression(cas.checked(right, budget)?, budget)?;
    if endpoint.sort != operand.sort
        || endpoint.scope != operand.scope
        || endpoint.ambient != operand.ambient
    {
        return Err(invalid());
    }
    let two = Rational::from_ratio(2, 1, budget)?;
    let half = k.div(&two, budget)?;
    let (pi, rational) = crate::rule_check::binary(
        &cas.state,
        cas.checked(right, budget)?,
        &Operator::Mul,
        budget,
    )?;
    let pi = cas.state.expression(pi, budget)?;
    let rational = cas.state.expression(rational, budget)?;
    let Operator::Rational(q) = &rational.operator else {
        return Err(invalid());
    };
    q.check(budget, AT)?;
    if pi.operator != Operator::Constant(Constant::Pi) || *q != half || q.mul(&two, budget)? != *k {
        return Err(invalid());
    }
    Ok(())
}

/// Statement subtype 31 follows the lattice nearest-selection subtype 30.
pub(crate) fn encode_statement(
    value: &Statement,
    out: &mut crate::encoding::Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut crate::encoding::Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    let Statement::HalfPiWitness {
        u,
        k,
        context,
        domain,
    } = value
    else {
        return Err(invalid());
    };
    out.byte(31)?;
    write(u, out)?;
    out.number(k)?;
    write(context, out)?;
    write(domain, out)
}

/// Certificate mode 19 follows lattice selection mode 18, independently of statement tags.
pub(crate) fn encode_certificate(
    value: &Certificate,
    out: &mut crate::encoding::Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut crate::encoding::Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    let Certificate::HalfPiWitness {
        u,
        k,
        context,
        domain,
        defined,
        equality,
    } = value
    else {
        return Err(invalid());
    };
    out.byte(19)?;
    write(u, out)?;
    out.number(k)?;
    write(context, out)?;
    write(domain, out)?;
    out.unsigned(u64::from(*defined))?;
    out.unsigned(u64::from(*equality))
}

#[cfg(test)]
mod tests {
    use super::{residue, value_term};
    use crate::budget::{Budget, BudgetKind, Limits};
    use crate::domain_model::HeaderDraft;
    use crate::graph::{Cas, Draft, ExpressionDraft, Reference};
    use crate::model::{AtomicSort, Environment, Field, Operator, Sort};
    use crate::number::Rational;
    use crate::{FaultKind, Location};

    fn scope_attempt(
        kind: BudgetKind,
        amount: u64,
    ) -> Result<(bool, u64), Box<dyn std::error::Error>> {
        let mut setup = Budget::new(Limits::default())?;
        let mut cas = Cas::new(Limits::default())?;
        let header = cas
            .admit(
                &[Draft::Header(HeaderDraft::Core(Environment::default()))],
                &mut setup,
            )?
            .pop()
            .ok_or("header")?;
        let scope = vec![vec![AtomicSort::Scalar(Field::Named("field".repeat(20))); 8]; 16];
        let source = cas
            .admit(
                &[Draft::Expression(ExpressionDraft {
                    header: Reference::Existing(header),
                    space: None,
                    operator: Operator::Rational(Rational::zero()),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                    scope: scope.clone(),
                    binding: None,
                    operands: Vec::new(),
                })],
                &mut setup,
            )?
            .pop()
            .ok_or("source")?;
        let before = cas.canonical_snapshot(&mut Budget::new(Limits::default())?)?;
        let mut limits = Limits::default();
        match kind {
            BudgetKind::Work => limits.work = amount,
            BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
            _ => return Err("resource".into()),
        }
        let mut budget = Budget::new(limits.clone())?;
        budget.work(7, Location::new("prior scope Work"))?;
        let result =
            budget.with_retained_payload(19, Location::new("prior scope payload"), |budget| {
                value_term(
                    &mut cas,
                    &source,
                    Operator::Rational(Rational::one()),
                    Vec::new(),
                    budget,
                )
            });
        assert_eq!(budget.limits(), &limits);
        match &result {
            Ok(handle) => assert_eq!(
                cas.state
                    .expression({ cas.checked(handle, &mut budget)? }, &mut budget)?
                    .scope,
                scope
            ),
            Err(fault) => {
                assert!(
                    matches!(fault.kind,FaultKind::Exhausted {budget,..} if budget==kind),
                    "{fault:?}"
                );
                assert_eq!(
                    cas.canonical_snapshot(&mut Budget::new(Limits::default())?)?,
                    before
                );
            }
        }
        Ok((result.is_ok(), budget.usage().work - 7))
    }

    /// Complete output admission retains the scope copy alongside nested admission workspace.
    /// ´test:unit:trig-value-scope-boundaries´
    #[test]
    fn trig_value_scope_boundaries() -> Result<(), Box<dyn std::error::Error>> {
        for (kind, cost, prefix) in [
            (BudgetKind::Work, 260_823, 260_821),
            (BudgetKind::StagedPayloadBytes, 138_908, 260_817),
        ] {
            let prior = if kind == BudgetKind::Work { 7 } else { 0 };
            assert_eq!(scope_attempt(kind, cost + prior)?, (true, 260_823));
            assert_eq!(scope_attempt(kind, cost + prior - 1)?, (false, prefix));
        }
        // The metered measuring buffer fits; prepayment refuses before any scope clone.
        assert_eq!(scope_attempt(BudgetKind::Work, 27_534)?, (false, 13_892));
        Ok(())
    }

    fn attempt(
        input: &str,
        expected: i64,
        kind: BudgetKind,
        amount: u64,
    ) -> Result<(bool, u64), Box<dyn std::error::Error>> {
        let mut setup = Budget::new(Limits::default())?;
        let value = Rational::parse(input, &mut setup)?;
        let original = value.clone();
        let output = Rational::from_ratio(expected, 1, &mut setup)?;
        let mut limits = Limits::default();
        match kind {
            BudgetKind::Work => limits.work = amount,
            BudgetKind::ArithmeticSteps => limits.arithmetic_steps = amount,
            BudgetKind::ComponentBits => limits.component_bits = amount,
            BudgetKind::IntermediateBits => {
                limits.intermediate_bits = amount;
                limits.component_bits = amount.min(limits.component_bits);
            }
            _ => return Err("residue resource".into()),
        }
        let mut budget = Budget::new(limits)?;
        budget.work(7, Location::new("prior residue Work"))?;
        let result = residue(&value, &mut budget);
        match &result {
            Ok(actual) => assert_eq!(*actual, output),
            Err(fault) => assert!(
                matches!(fault.kind,FaultKind::Exhausted {budget,..} if budget==kind || (kind==BudgetKind::IntermediateBits && budget==BudgetKind::ComponentBits)),
                "{fault:?}"
            ),
        }
        assert_eq!(value, original);
        Ok((result.is_ok(), budget.usage().work - 7))
    }

    /// Exact Euclidean division includes construction, division, floor, product and subtraction.
    /// Arithmetic adapters prepay each step; a refusal retains completed earlier steps.
    /// ´test:unit:trig-residue-resource-boundaries´
    #[test]
    fn residue_resource_boundaries() -> Result<(), Box<dyn std::error::Error>> {
        let kinds = [
            BudgetKind::Work,
            BudgetKind::ArithmeticSteps,
            BudgetKind::ComponentBits,
            BudgetKind::IntermediateBits,
        ];
        for (input, expected, costs, prefixes) in [
            ("0", 0, [1664, 52, 3, 3], [1632, 1632, 128, 0]),
            (
                "18446744073709551617",
                1,
                [2480, 55, 65, 66],
                [2448, 2448, 0, 568],
            ),
            (
                "-18446744073709551617",
                3,
                [2512, 56, 65, 66],
                [2480, 2480, 0, 568],
            ),
        ] {
            for ((kind, cost), prefix) in kinds.into_iter().zip(costs).zip(prefixes) {
                let amount = cost + if kind == BudgetKind::Work { 7 } else { 0 };
                assert_eq!(attempt(input, expected, kind, amount)?, (true, costs[0]));
                assert_eq!(attempt(input, expected, kind, amount - 1)?, (false, prefix));
            }
        }
        Ok(())
    }
}
