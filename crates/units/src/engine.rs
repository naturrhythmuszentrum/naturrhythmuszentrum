// SPDX-License-Identifier: AGPL-3.0-only
//! One CAS store supplies original parsing, syntax ownership and equality proofs.

use std::cell::{Ref, RefCell, RefMut};

use nrz_cas::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use nrz_cas::limits::{Limits, LimitsOverride};
use nrz_cas::request::Outcome;
use nrz_cas::source::{OriginalForm, ParseError, ParseFailure, parse_original};
use nrz_cas::syntax::{AtomicSort, Declaration, Draft, Environment, Field, Sort};
use nrz_cas::{Cas, Fault, Handle, InvalidCode, Location as CasLocation, Reference};

use crate::exact::budget;
use crate::{CheckedExpression, Context, Diagnostic, Expectation, Failure, Location, Registry};

impl Context {
    /// Create physical rules over explicit mathematical declaration keys.
    ///
    /// Forward references belong in `environment` before their physical declarations.
    /// Standard scalar function signatures are added to the mathematical header.
    /// Parsing uses the store defaults; `limits` govern each physical check.
    ///
    /// # Errors
    /// Refuses invalid mathematical declarations or exhausted store admission.
    pub fn new(
        registry: Registry,
        limits: Limits,
        mut environment: Environment,
    ) -> Result<Self, Fault> {
        limits.validate()?;
        for (key, arity) in [
            ("sin", 1),
            ("cos", 1),
            ("tan", 1),
            ("asin", 1),
            ("acos", 1),
            ("atan", 1),
            ("exp", 1),
            ("log", 1),
            ("sqrt", 1),
            ("count_ratio", 2),
        ] {
            environment.declarations.push(Declaration {
                key: key.into(),
                display_name: key.into(),
                sort: Sort::Function {
                    inputs: vec![AtomicSort::Scalar(Field::Real); arity],
                    output: AtomicSort::Scalar(Field::Real),
                },
            });
        }
        let mut cas = Cas::new(Limits::default())?;
        let semantic = semantic_context(&mut cas, environment)?;
        Ok(Self {
            cas: RefCell::new(cas),
            semantic,
            registry,
            limits,
            declarations: std::collections::BTreeMap::new(),
            functions: std::collections::BTreeMap::new(),
        })
    }

    /// Borrow the owning CAS for read-only witness and original-syntax inspection.
    ///
    /// # Errors
    /// Refuses a conflicting active borrow instead of panicking.
    pub fn cas(&self) -> Result<Ref<'_, Cas>, Fault> {
        self.cas.try_borrow().map_err(|_| borrow_fault())
    }

    /// Mutate the owning store only while no inspection borrow is live.
    pub(super) fn engine(&self) -> Result<RefMut<'_, Cas>, Fault> {
        self.cas.try_borrow_mut().map_err(|_| borrow_fault())
    }

    /// Parse text against this context's actual semantic header, without normalization.
    ///
    /// # Errors
    /// Retains parser byte spans for syntax, declaration and resource refusals.
    pub fn parse(&self, key: &str, text: &str) -> Result<OriginalForm, Diagnostic> {
        let setup = || {
            let cas = self.engine()?;
            let header = cas.context_header(&self.semantic)?;
            Ok::<_, Fault>((cas, header, budget()?))
        };
        let (mut cas, header, mut ledger) =
            setup().map_err(|error| Diagnostic::declaration(error.into()))?;
        parse_original(&mut cas, key, text, &header, &mut ledger)
            .map_err(|error| parse_diagnostic(error, text))
    }

    /// Request a proved equality of checked coherent coordinates and an explicit target.
    ///
    /// Conditional outcomes retain the CAS's nonzero and other domain residuals.
    /// Physical declarations supply no additional mathematical assumptions.
    ///
    /// # Errors
    /// Refuses foreign inputs, incomplete coverage or exhausted request construction.
    pub fn prove_equal(
        &self,
        checked: &CheckedExpression,
        target: &OriginalForm,
        limits: LimitsOverride,
    ) -> Result<Outcome, Fault> {
        let mut cas = self.engine()?;
        let request = checked.equality_request(&mut cas, &self.semantic, target, limits)?;
        Ok(cas.run(&request))
    }
}

/// Admit a single root context, its ambient region and its semantic header atomically.
fn semantic_context(cas: &mut Cas, environment: Environment) -> Result<Handle, Fault> {
    let handles = cas.admit(
        &[
            Draft::Header(HeaderDraft::Core(environment)),
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
        ],
        &mut budget()?,
    )?;
    handles
        .get(2)
        .cloned()
        .ok_or_else(|| Fault::invalid(InvalidCode::Reference, CasLocation::new("units context")))
}

/// Store borrowing failures remain ordinary errors at the public boundary.
const fn borrow_fault() -> Fault {
    Fault::invalid(InvalidCode::Context, CasLocation::new("units store borrow"))
}

/// Own borrowed parser diagnostics while keeping their original byte intervals.
fn parse_diagnostic(error: ParseError<'_>, text: &str) -> Diagnostic {
    let failure = match error.failure {
        ParseFailure::Fault(fault) => fault.into(),
        ParseFailure::Syntax(code) => Failure::Syntax(code),
        ParseFailure::Arity {
            expected, actual, ..
        } => Failure::Arity {
            expected,
            derived: actual,
        },
        ParseFailure::NotCallable { name } => Failure::UnknownFunction(name.into()),
        ParseFailure::UnknownName { name } => {
            let tail = usize::try_from(error.span.end)
                .ok()
                .and_then(|end| text.get(end..));
            if tail.is_some_and(|tail| tail.trim_start().starts_with('(')) {
                Failure::UnknownFunction(name.into())
            } else {
                Failure::UnknownSymbol(name.into())
            }
        }
    };
    Diagnostic {
        location: Box::new(Location {
            path: Vec::new(),
            span: Some(error.span),
            definitions: Vec::new(),
        }),
        expected: Expectation::Rule("declared original expression syntax"),
        derived: Vec::new(),
        failure,
    }
}
