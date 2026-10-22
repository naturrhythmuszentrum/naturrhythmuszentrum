// SPDX-License-Identifier: AGPL-3.0-only
//! Successful checks retain original operations independently of normalized coordinates.
//!
//! # Retained checking and propagation evidence
//!
//! A successful check owns its original expression, postorder node descriptors, exact conversion maps and the declarations it used, including source and assurance claims. A requested output descriptor additionally records conversion from coherent coordinates to that output measure. `Context::prove_equal` submits a `Task::Equal` request for checked coherent coordinates and an explicit original target. The simplifier returns a proved, conditional, refused or unfinished outcome; conditional equality retains its residual domain obligations. A coordinate equality carries no physical proof; a zero coordinate retains its descriptor in the checked result. Normalization cannot be passed back as a substitute for original operations. Refusals carry a typed reason, operand path, optional byte span, declaration chain, expected descriptor or operation constraint, and the descriptors successfully derived at the failing operation. Entering an observable body appends operand index 0; its node evidence and declaration failures use that same path and the body's span. Uncertainty inputs establish declared value, optional second-value and reference symbol identities. Literal coefficients and declared typed ratio constants preserve the relationship under supported scaling; scaling by another ratio quantity requires propagation. Sums carrying the same companion evidence and supported powers preserve identity, while multiplying standard uncertainties gives a variance only for the same original uncertainty symbol. A derived alias carries its body's existing companion and evidence; the alias name does not establish a new companion value identity. Equal dimensions alone establish no identity. Relative quotients must use the declared reference symbol as the divisor. For ratio-valued uncertainties, division by a ratio is scaling unless the divisor is the bound value or the companion explicitly names it as its reference. Relative inputs require the relationship-bearing declaration; ordinary inputs, constants, derived declarations and fixed function results cannot introduce an unbound relative kind.
//!
//! An uncertainty declaration for a derived value may explicitly propagate from companions of that value or symbols in its resolved dependency closure. A covariance declaration may use companions from either value's closure. The checker verifies the body's declared descriptor, requires every uncertainty operand to carry a checked companion inside those closures, and records `CompanionEvidence::Propagated` with the contributing uncertainty symbols and their relationships. Original operands remain contributors even under zero factors or cancellation. `CompanionEvidence::Established` distinguishes the relationships supplied by inputs and preserved by the supported identity operations. A propagated companion binds its declared target for later variance and relative checks; relative conversion uses a bound absolute companion and the declared compatible nonzero reference. Propagation evidence establishes admissible dependencies and physical kinds, not correctness of a numerical propagation formula, independence assumptions, correlation values or assurance claims. Numerical nonzero proofs remain outside this contract.

use std::collections::BTreeMap;

use crate::exact::budget;
use nrz_cas::proof::DomainContract;
use nrz_cas::request::{CoveredInput, Request, Task};
use nrz_cas::{Cas, Fault, Handle, limits::LimitsOverride, source::OriginalForm};

use crate::{
    Companion, Conversion, Declaration, Descriptor, Location, SnapshotId, UncertaintyMode,
};

/// How a checked uncertainty expression acquires its declared value relationship.
///
/// TODO ´todo:code:carry-indexed-target-and-contraction-evidence´: carry indexed target and contraction evidence separately from numerical covariance validity under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:retain-cross-binding-source-dependencies-in´: retain cross binding source dependencies in reusable propagation evidence under rule:nrz:batch-source-covariance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompanionEvidence {
    /// A declared input relationship preserved by supported identity operations.
    ///
    /// TODO ´todo:code:retain-established-entry-identities-through-supported´: retain established entry identities through supported view and permutation maps under conv:nrz:shape-entry-order and def:nrz:companion-shape.
    Established(Companion),
    /// A kind-checked body whose uncertainty contributors belong to the value closures.
    ///
    /// TODO ´todo:code:retain-paired-contributor-maps-across-covariance´: retain paired contributor maps across covariance contractions without inferring independence under rule:nrz:shape-sensitivity-propagation and rule:nrz:shape-data-time-evidence.
    Propagated {
        /// The declared target relationship, available to later uncertainty operations.
        companion: Companion,
        /// Contributing uncertainty symbols and their checked value relationships.
        contributors: BTreeMap<String, Companion>,
    },
}

impl CompanionEvidence {
    /// The relationship carried by either established or propagated evidence.
    #[must_use]
    pub const fn companion(&self) -> &Companion {
        match self {
            Self::Established(companion) | Self::Propagated { companion, .. } => companion,
        }
    }

    /// Preserve the evidence source while changing a supported uncertainty mode or reference.
    ///
    /// TODO ´todo:code:transform-uncertainty-reference-maps-and-paired´: transform uncertainty reference maps and paired axes together with each supported mode under rule:nrz:correlation-and-relative-shapes.
    pub(super) fn transform(&mut self, mode: UncertaintyMode, reference: Option<String>) {
        let (Self::Established(companion) | Self::Propagated { companion, .. }) = self;
        companion.mode = mode;
        companion.reference = reference;
    }
}

/// The descriptor evidence associated with one original node.
///
/// TODO ´todo:code:record-original-axis-contractions-and-coordinate´: record original axis contractions and coordinate maps beside each checked node under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:map-checked-expression-locations-to-client´: map checked expression locations to client operation and slot identities under def:nrz:plan-binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeEvidence {
    /// Original tree location or expanded declaration location.
    pub location: Location,
    /// Derived descriptor before conversion to coherent result coordinates.
    pub descriptor: Descriptor,
    /// Conversion from this node's declared result coordinates to coherent coordinates.
    pub conversion: Conversion,
    /// Function argument conversions from coherent coordinates to signature coordinates.
    pub arguments: Vec<Conversion>,
}

/// A validated original expression and an optional algebraic view of its coherent coordinates.
///
/// TODO ´todo:code:own-checked-logical-shapes-and-indexed´: own checked logical shapes and indexed companion evidence before coordinate normalization under rule:nrz:shape-data-time-evidence.
///
/// TODO ´todo:code:retain-checked-schema-admission-witnesses-for´: retain checked schema admission witnesses for descriptor publication under rule:nrz:descriptor-publication-validation.
#[derive(Clone, Debug)]
pub struct CheckedExpression {
    /// Exact physical semantic state captured before checking original operations.
    pub(super) snapshot: SnapshotId,
    /// Original source operations, including those that a normal form would erase.
    pub(super) original: OriginalForm,
    /// Root descriptor with its semantic identity and affine role.
    pub(super) descriptor: Descriptor,
    /// Every original node's postorder checking evidence.
    pub(super) nodes: Vec<NodeEvidence>,
    /// Coherent coordinate expression built only after each original operation passes.
    pub(super) coordinates: Handle,
    /// Owned declaration snapshots, including provenance and original derived expressions.
    pub(super) declarations: BTreeMap<String, Declaration>,
    /// Optional conversion from coherent coordinates to the caller's expected measure.
    pub(super) output: Option<Conversion>,
    /// Established or propagated value and reference identity, when supplied by the expression.
    pub(super) companion: Option<CompanionEvidence>,
}

impl CheckedExpression {
    /// Captured semantic identity; validate it before reuse against another owner.
    #[must_use]
    pub const fn snapshot_id(&self) -> &SnapshotId {
        &self.snapshot
    }

    /// Value and reference names carried by established or explicitly propagated evidence.
    ///
    /// TODO ´todo:code:expose-companion-entry-targets-and-versions´: expose companion entry targets and versions without collapsing them to equal descriptors under def:nrz:companion-shape.
    ///
    /// TODO ´todo:code:expose-declaration-relationships-for-explicit-instantiation´: expose declaration relationships for explicit instantiation against bound slot identities under def:nrz:plan-binding.
    #[must_use]
    pub const fn companion(&self) -> Option<&Companion> {
        match &self.companion {
            Some(evidence) => Some(evidence.companion()),
            None => None,
        }
    }

    /// Distinguish preserved input identity from an explicitly declared propagation.
    ///
    /// TODO ´todo:code:expose-matrix-propagation-obligations-separately-from´: expose matrix propagation obligations separately from established descriptor and dependency evidence under rule:nrz:shape-data-time-evidence.
    ///
    /// TODO ´todo:code:expose-reusable-source-dependencies-without-implying´: expose reusable source dependencies without implying point independent uncertainty under rule:nrz:batch-source-covariance.
    #[must_use]
    pub const fn companion_evidence(&self) -> Option<&CompanionEvidence> {
        self.companion.as_ref()
    }

    /// Original operations; these remain authoritative for domains and physical validity.
    #[must_use]
    pub const fn original(&self) -> &OriginalForm {
        &self.original
    }

    /// Root's physical descriptor, never discarded when coordinates normalize to zero.
    ///
    /// TODO ´todo:code:expose-ordered-axis-descriptors-alongside-the´: expose ordered axis descriptors alongside the scalar root descriptor for shaped results under def:nrz:matrix-shape.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    /// Postorder evidence for original nodes and expanded declarations.
    ///
    /// TODO ´todo:code:expose-original-contraction-paths-and-typed´: expose original contraction paths and typed zero evidence with indexed nodes under rule:nrz:shape-data-time-evidence.
    #[must_use]
    pub fn nodes(&self) -> &[NodeEvidence] {
        &self.nodes
    }

    /// The declarations used by this check, retained independently of later context changes.
    ///
    /// TODO ´todo:code:expose-declaration-snapshot-identity-alongside-the´: expose declaration snapshot identity alongside the retained evidence inventory under rule:nrz:computation-cache-validity.
    #[must_use]
    pub const fn declarations(&self) -> &BTreeMap<String, Declaration> {
        &self.declarations
    }

    /// Conversion to an explicit requested output descriptor, if one was checked.
    ///
    /// TODO ´todo:code:expose-paired-covariance-conversion-maps-instead´: expose paired covariance conversion maps instead of one scalar output conversion under def:nrz:covariance-shape.
    ///
    /// TODO ´todo:code:associate-exposed-conversion-evidence-with-its´: associate exposed conversion evidence with its checked binding schema version under rule:nrz:binding-validation.
    #[must_use]
    pub const fn output_conversion(&self) -> Option<&Conversion> {
        self.output.as_ref()
    }

    /// Cover both endpoints before requesting equality from the engine simplifier.
    pub(super) fn equality_request(
        &self,
        cas: &mut Cas,
        context: &Handle,
        target: &OriginalForm,
        limits: LimitsOverride,
    ) -> Result<Request, Fault> {
        let mut ledger = budget()?;
        let root =
            cas.materialize_derived(&self.coordinates, &self.original.syntax, &mut ledger)?;
        let receipt = cas
            .walk_derived(&root, &mut ledger)
            .map_err(|error| error.fault)?
            .receipt;
        let target_receipt = cas
            .walk_original(&target.root, &mut ledger)
            .map_err(|error| error.fault)?
            .receipt;
        Ok(Request {
            task: Task::Equal {
                domain: cas.admissible_region(context)?,
                domain_contract: DomainContract::DefinedAndEqual,
            },
            context: context.clone(),
            inputs: vec![
                CoveredInput { root, receipt },
                CoveredInput {
                    root: target.root.clone(),
                    receipt: target_receipt,
                },
            ],
            premises: Vec::new(),
            limits,
        })
    }
}
