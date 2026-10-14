// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit resource limits and monotone work accounting.

use crate::error::{Fault, InvalidCode, Location};

/// A resource whose next consumption can be refused.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BudgetKind {
    /// Retained UTF-8 source bytes.
    SourceBytes,
    /// Bytes per declared key.
    NameBytes,
    /// Published original occurrences.
    OriginalOccurrences,
    /// Maximum explicit traversal or scope depth.
    Depth,
    /// Published heterogeneous vertices.
    Vertices,
    /// Published typed edges.
    Edges,
    /// Candidate-state vertices.
    StagedVertices,
    /// Candidate-state edges.
    StagedEdges,
    /// Published logical payload bytes.
    PayloadBytes,
    /// Candidate and draft payload bytes.
    StagedPayloadBytes,
    /// Elementary generation work.
    Work,
    /// Arithmetic operation work.
    ArithmeticSteps,
    /// Bits in an admitted rational component.
    ComponentBits,
    /// Bits in a bigint intermediate.
    IntermediateBits,
    /// Bytes in one exact numeric literal.
    LiteralBytes,
    /// Absolute executed decimal exponent.
    DecimalExponent,
    /// Absolute executed integer exponent.
    IntegerExponent,
    /// Total executed polynomial degree.
    PolynomialDegree,
    /// Executed polynomial variables.
    PolynomialVariables,
    /// Live polynomial terms.
    PolynomialTerms,
    /// Explicit matrix entries.
    MatrixEntries,
    /// Predicates in a context or residual set.
    ConditionAtoms,
    /// Condition-checking work.
    ConditionSteps,
    /// Attempted identity matches.
    MatchAttempts,
    /// Attempts in one queue visit.
    MatchBatch,
    /// Live nondominated labels.
    FrontierLabels,
    /// Queue entries and cursors.
    QueueEntries,
    /// Attempted identity applications.
    Transitions,
    /// Cumulative positive expression growth.
    ExpansionCredits,
    /// Peak unfolded expression size.
    PeakExpressionSize,
    /// Expanded proof records.
    ProofNodes,
    /// Portable witness bytes.
    WitnessBytes,
    /// Optional result payload bytes.
    OutputBytes,
    /// Stored diagnostics.
    DiagnosticRecords,
    /// Independent replay work.
    ReplayWork,
    /// Cost queue visits per epoch.
    CostQuota,
    /// Progress queue visits per epoch.
    ProgressQuota,
}

/// Effective finite ceilings for one service call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Limits {
    /// Retained UTF-8 source bytes.
    pub source_bytes: u64,
    /// Bytes per declared key.
    pub name_bytes: u64,
    /// Published original occurrences.
    pub original_occurrences: u64,
    /// Maximum explicit traversal or scope depth.
    pub depth: u64,
    /// Published heterogeneous vertices.
    pub vertices: u64,
    /// Published typed edges.
    pub edges: u64,
    /// Candidate-state vertices.
    pub staged_vertices: u64,
    /// Candidate-state edges.
    pub staged_edges: u64,
    /// Published logical payload bytes.
    pub payload_bytes: u64,
    /// Candidate and draft payload bytes.
    pub staged_payload_bytes: u64,
    /// Elementary generation work.
    pub work: u64,
    /// Arithmetic operation work.
    pub arithmetic_steps: u64,
    /// Bits in an admitted rational component.
    pub component_bits: u64,
    /// Bits in a bigint intermediate.
    pub intermediate_bits: u64,
    /// Bytes in one exact numeric literal.
    pub literal_bytes: u64,
    /// Absolute executed decimal exponent.
    pub decimal_exponent: u64,
    /// Absolute executed integer exponent.
    pub integer_exponent: u64,
    /// Total executed polynomial degree.
    pub polynomial_degree: u64,
    /// Executed polynomial variables.
    pub polynomial_variables: u64,
    /// Live polynomial terms.
    pub polynomial_terms: u64,
    /// Explicit matrix entries.
    pub matrix_entries: u64,
    /// Predicates in a context or residual set.
    pub condition_atoms: u64,
    /// Condition-checking work.
    pub condition_steps: u64,
    /// Attempted identity matches.
    pub match_attempts: u64,
    /// Attempts in one queue visit.
    pub match_batch: u64,
    /// Live nondominated labels.
    pub frontier_labels: u64,
    /// Queue entries and cursors.
    pub queue_entries: u64,
    /// Attempted identity applications.
    pub transitions: u64,
    /// Cumulative positive expression growth.
    pub expansion_credits: u64,
    /// Peak unfolded expression size.
    pub peak_expression_size: u64,
    /// Expanded proof records.
    pub proof_nodes: u64,
    /// Portable witness bytes.
    pub witness_bytes: u64,
    /// Optional result payload bytes.
    pub output_bytes: u64,
    /// Stored diagnostics.
    pub diagnostic_records: u64,
    /// Independent replay work.
    pub replay_work: u64,
    /// Cost queue visits per epoch.
    pub cost_quota: u64,
    /// Progress queue visits per epoch.
    pub progress_quota: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            source_bytes: 262_144,
            name_bytes: 256,
            original_occurrences: 10_000,
            depth: 128,
            vertices: 50_000,
            edges: 200_000,
            staged_vertices: 50_000,
            staged_edges: 200_000,
            payload_bytes: 16_777_216,
            staged_payload_bytes: 16_777_216,
            work: 2_000_000,
            arithmetic_steps: 500_000,
            component_bits: 4096,
            intermediate_bits: 8192,
            literal_bytes: 256,
            decimal_exponent: 128,
            integer_exponent: 256,
            polynomial_degree: 64,
            polynomial_variables: 16,
            polynomial_terms: 4096,
            matrix_entries: 4096,
            condition_atoms: 256,
            condition_steps: 100_000,
            match_attempts: 50_000,
            match_batch: 32,
            frontier_labels: 8192,
            queue_entries: 16_384,
            transitions: 20_000,
            expansion_credits: 20_000,
            peak_expression_size: 50_000,
            proof_nodes: 20_000,
            witness_bytes: 1_048_576,
            output_bytes: 1_048_576,
            diagnostic_records: 128,
            replay_work: 2_000_000,
            cost_quota: 4,
            progress_quota: 1,
        }
    }
}

/// Optional overrides resolved before a request starts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LimitsOverride {
    /// Override for `source_bytes`.
    pub source_bytes: Option<u64>,
    /// Override for `name_bytes`.
    pub name_bytes: Option<u64>,
    /// Override for `original_occurrences`.
    pub original_occurrences: Option<u64>,
    /// Override for depth.
    pub depth: Option<u64>,
    /// Override for vertices.
    pub vertices: Option<u64>,
    /// Override for edges.
    pub edges: Option<u64>,
    /// Override for `staged_vertices`.
    pub staged_vertices: Option<u64>,
    /// Override for `staged_edges`.
    pub staged_edges: Option<u64>,
    /// Override for `payload_bytes`.
    pub payload_bytes: Option<u64>,
    /// Override for `staged_payload_bytes`.
    pub staged_payload_bytes: Option<u64>,
    /// Override for work.
    pub work: Option<u64>,
    /// Override for `arithmetic_steps`.
    pub arithmetic_steps: Option<u64>,
    /// Override for `component_bits`.
    pub component_bits: Option<u64>,
    /// Override for `intermediate_bits`.
    pub intermediate_bits: Option<u64>,
    /// Override for `literal_bytes`.
    pub literal_bytes: Option<u64>,
    /// Override for `decimal_exponent`.
    pub decimal_exponent: Option<u64>,
    /// Override for `integer_exponent`.
    pub integer_exponent: Option<u64>,
    /// Override for `polynomial_degree`.
    pub polynomial_degree: Option<u64>,
    /// Override for `polynomial_variables`.
    pub polynomial_variables: Option<u64>,
    /// Override for `polynomial_terms`.
    pub polynomial_terms: Option<u64>,
    /// Override for `matrix_entries`.
    pub matrix_entries: Option<u64>,
    /// Override for `condition_atoms`.
    pub condition_atoms: Option<u64>,
    /// Override for `condition_steps`.
    pub condition_steps: Option<u64>,
    /// Override for `match_attempts`.
    pub match_attempts: Option<u64>,
    /// Override for `match_batch`.
    pub match_batch: Option<u64>,
    /// Override for `frontier_labels`.
    pub frontier_labels: Option<u64>,
    /// Override for `queue_entries`.
    pub queue_entries: Option<u64>,
    /// Override for transitions.
    pub transitions: Option<u64>,
    /// Override for `expansion_credits`.
    pub expansion_credits: Option<u64>,
    /// Override for `peak_expression_size`.
    pub peak_expression_size: Option<u64>,
    /// Override for `proof_nodes`.
    pub proof_nodes: Option<u64>,
    /// Override for `witness_bytes`.
    pub witness_bytes: Option<u64>,
    /// Override for `output_bytes`.
    pub output_bytes: Option<u64>,
    /// Override for `diagnostic_records`.
    pub diagnostic_records: Option<u64>,
    /// Override for `replay_work`.
    pub replay_work: Option<u64>,
    /// Override for `cost_quota`.
    pub cost_quota: Option<u64>,
    /// Override for `progress_quota`.
    pub progress_quota: Option<u64>,
}

impl Limits {
    /// Validate relationships needed by all service operations.
    ///
    /// # Errors
    /// Returns an invalid-policy fault for inconsistent ceilings or zero quotas.
    pub const fn validate(&self) -> Result<(), Fault> {
        if self.component_bits == 0
            || self.intermediate_bits < self.component_bits
            || self.match_batch == 0
            || self.cost_quota == 0
            || self.progress_quota == 0
        {
            return Err(Fault::invalid(InvalidCode::Policy, Location::new("limits")));
        }
        Ok(())
    }

    /// Return one named ceiling.
    #[must_use]
    pub const fn get(&self, kind: BudgetKind) -> u64 {
        match kind {
            BudgetKind::SourceBytes => self.source_bytes,
            BudgetKind::NameBytes => self.name_bytes,
            BudgetKind::OriginalOccurrences => self.original_occurrences,
            BudgetKind::Depth => self.depth,
            BudgetKind::Vertices => self.vertices,
            BudgetKind::Edges => self.edges,
            BudgetKind::StagedVertices => self.staged_vertices,
            BudgetKind::StagedEdges => self.staged_edges,
            BudgetKind::PayloadBytes => self.payload_bytes,
            BudgetKind::StagedPayloadBytes => self.staged_payload_bytes,
            BudgetKind::Work => self.work,
            BudgetKind::ArithmeticSteps => self.arithmetic_steps,
            BudgetKind::ComponentBits => self.component_bits,
            BudgetKind::IntermediateBits => self.intermediate_bits,
            BudgetKind::LiteralBytes => self.literal_bytes,
            BudgetKind::DecimalExponent => self.decimal_exponent,
            BudgetKind::IntegerExponent => self.integer_exponent,
            BudgetKind::PolynomialDegree => self.polynomial_degree,
            BudgetKind::PolynomialVariables => self.polynomial_variables,
            BudgetKind::PolynomialTerms => self.polynomial_terms,
            BudgetKind::MatrixEntries => self.matrix_entries,
            BudgetKind::ConditionAtoms => self.condition_atoms,
            BudgetKind::ConditionSteps => self.condition_steps,
            BudgetKind::MatchAttempts => self.match_attempts,
            BudgetKind::MatchBatch => self.match_batch,
            BudgetKind::FrontierLabels => self.frontier_labels,
            BudgetKind::QueueEntries => self.queue_entries,
            BudgetKind::Transitions => self.transitions,
            BudgetKind::ExpansionCredits => self.expansion_credits,
            BudgetKind::PeakExpressionSize => self.peak_expression_size,
            BudgetKind::ProofNodes => self.proof_nodes,
            BudgetKind::WitnessBytes => self.witness_bytes,
            BudgetKind::OutputBytes => self.output_bytes,
            BudgetKind::DiagnosticRecords => self.diagnostic_records,
            BudgetKind::ReplayWork => self.replay_work,
            BudgetKind::CostQuota => self.cost_quota,
            BudgetKind::ProgressQuota => self.progress_quota,
        }
    }

    /// Resolve every supplied override and validate the result.
    ///
    /// # Errors
    /// Returns an invalid-policy fault for inconsistent resolved values.
    pub fn resolve(&self, overrides: &LimitsOverride) -> Result<Self, Fault> {
        let resolved = Self {
            source_bytes: overrides.source_bytes.unwrap_or(self.source_bytes),
            name_bytes: overrides.name_bytes.unwrap_or(self.name_bytes),
            original_occurrences: overrides
                .original_occurrences
                .unwrap_or(self.original_occurrences),
            depth: overrides.depth.unwrap_or(self.depth),
            vertices: overrides.vertices.unwrap_or(self.vertices),
            edges: overrides.edges.unwrap_or(self.edges),
            staged_vertices: overrides.staged_vertices.unwrap_or(self.staged_vertices),
            staged_edges: overrides.staged_edges.unwrap_or(self.staged_edges),
            payload_bytes: overrides.payload_bytes.unwrap_or(self.payload_bytes),
            staged_payload_bytes: overrides
                .staged_payload_bytes
                .unwrap_or(self.staged_payload_bytes),
            work: overrides.work.unwrap_or(self.work),
            arithmetic_steps: overrides.arithmetic_steps.unwrap_or(self.arithmetic_steps),
            component_bits: overrides.component_bits.unwrap_or(self.component_bits),
            intermediate_bits: overrides
                .intermediate_bits
                .unwrap_or(self.intermediate_bits),
            literal_bytes: overrides.literal_bytes.unwrap_or(self.literal_bytes),
            decimal_exponent: overrides.decimal_exponent.unwrap_or(self.decimal_exponent),
            integer_exponent: overrides.integer_exponent.unwrap_or(self.integer_exponent),
            polynomial_degree: overrides
                .polynomial_degree
                .unwrap_or(self.polynomial_degree),
            polynomial_variables: overrides
                .polynomial_variables
                .unwrap_or(self.polynomial_variables),
            polynomial_terms: overrides.polynomial_terms.unwrap_or(self.polynomial_terms),
            matrix_entries: overrides.matrix_entries.unwrap_or(self.matrix_entries),
            condition_atoms: overrides.condition_atoms.unwrap_or(self.condition_atoms),
            condition_steps: overrides.condition_steps.unwrap_or(self.condition_steps),
            match_attempts: overrides.match_attempts.unwrap_or(self.match_attempts),
            match_batch: overrides.match_batch.unwrap_or(self.match_batch),
            frontier_labels: overrides.frontier_labels.unwrap_or(self.frontier_labels),
            queue_entries: overrides.queue_entries.unwrap_or(self.queue_entries),
            transitions: overrides.transitions.unwrap_or(self.transitions),
            expansion_credits: overrides
                .expansion_credits
                .unwrap_or(self.expansion_credits),
            peak_expression_size: overrides
                .peak_expression_size
                .unwrap_or(self.peak_expression_size),
            proof_nodes: overrides.proof_nodes.unwrap_or(self.proof_nodes),
            witness_bytes: overrides.witness_bytes.unwrap_or(self.witness_bytes),
            output_bytes: overrides.output_bytes.unwrap_or(self.output_bytes),
            diagnostic_records: overrides
                .diagnostic_records
                .unwrap_or(self.diagnostic_records),
            replay_work: overrides.replay_work.unwrap_or(self.replay_work),
            cost_quota: overrides.cost_quota.unwrap_or(self.cost_quota),
            progress_quota: overrides.progress_quota.unwrap_or(self.progress_quota),
        };
        resolved.validate()?;
        Ok(resolved)
    }
}

/// Monotone counters for one generation or checking phase.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BudgetUsage {
    /// Elementary work already consumed.
    pub work: u64,
    /// Big-integer and coefficient operations consumed.
    pub arithmetic_steps: u64,
    /// Condition visits already consumed.
    pub condition_steps: u64,
}

/// A finite work ledger, shared by all operations in a call.
#[derive(Debug)]
pub struct Budget {
    /// Resolved ceilings.
    limits: Limits,
    /// Monotone measured consumption.
    usage: BudgetUsage,
}

impl Budget {
    /// Start a checked ledger.
    ///
    /// # Errors
    /// Returns an invalid-policy fault when limits are inconsistent.
    pub fn new(limits: Limits) -> Result<Self, Fault> {
        limits.validate()?;
        Ok(Self {
            limits,
            usage: BudgetUsage::default(),
        })
    }

    /// Temporarily intersect permanent storage ceilings with the active call policy.
    pub(crate) fn restrict_storage(&mut self, store: &Limits) -> Limits {
        let previous = self.limits.clone();
        self.limits.source_bytes = self.limits.source_bytes.min(store.source_bytes);
        self.limits.name_bytes = self.limits.name_bytes.min(store.name_bytes);
        self.limits.original_occurrences = self
            .limits
            .original_occurrences
            .min(store.original_occurrences);
        self.limits.depth = self.limits.depth.min(store.depth);
        self.limits.vertices = self.limits.vertices.min(store.vertices);
        self.limits.edges = self.limits.edges.min(store.edges);
        self.limits.staged_vertices = self.limits.staged_vertices.min(store.staged_vertices);
        self.limits.staged_edges = self.limits.staged_edges.min(store.staged_edges);
        self.limits.payload_bytes = self.limits.payload_bytes.min(store.payload_bytes);
        self.limits.staged_payload_bytes = self
            .limits
            .staged_payload_bytes
            .min(store.staged_payload_bytes);
        previous
    }

    /// Reserve retained temporary maps while nested arithmetic accounts for its own scratch.
    pub(crate) fn reserve_temporary(&mut self, bytes: u64, at: Location) -> Result<Limits, Fault> {
        self.check(BudgetKind::StagedPayloadBytes, bytes, at)?;
        let previous = self.limits.clone();
        self.limits.staged_payload_bytes = self
            .limits
            .staged_payload_bytes
            .checked_sub(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        Ok(previous)
    }

    /// Reserve an independent replay ceiling inside the remaining aggregate work ledger.
    pub(crate) fn restrict_replay(&mut self) -> Result<Limits, Fault> {
        let previous = self.limits.clone();
        let ceiling = self
            .usage
            .work
            .checked_add(self.limits.replay_work)
            .ok_or_else(|| {
                Fault::capacity(BudgetKind::ReplayWork, Location::new("replay allowance"))
            })?;
        self.limits.work = self.limits.work.min(ceiling);
        Ok(previous)
    }

    /// Restore the caller policy while retaining every consumed work unit.
    pub(crate) const fn restore_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    /// Read the effective ceilings.
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Snapshot counters without resetting them.
    #[must_use]
    pub const fn usage(&self) -> BudgetUsage {
        self.usage
    }

    /// Check a size without consuming a counter.
    ///
    /// # Errors
    /// Returns exhaustion if the measured size exceeds the named ceiling.
    pub const fn check(&self, kind: BudgetKind, needed: u64, at: Location) -> Result<(), Fault> {
        let limit = self.limits.get(kind);
        if needed > limit {
            return Err(Fault::exhausted(kind, limit, needed, at));
        }
        Ok(())
    }

    /// Consume elementary work before performing it.
    ///
    /// # Errors
    /// Returns exhaustion before exceeding the work ceiling.
    pub fn work(&mut self, amount: u64, at: Location) -> Result<(), Fault> {
        let next =
            self.usage.work.checked_add(amount).ok_or_else(|| {
                Fault::exhausted(BudgetKind::Work, self.limits.work, u64::MAX, at)
            })?;
        self.check(BudgetKind::Work, next, at)?;
        self.usage.work = next;
        Ok(())
    }

    /// Charge a bigint operation and its conservative elementary cost.
    ///
    /// # Errors
    /// Returns exhaustion before performing any unbudgeted arithmetic.
    pub fn arithmetic(&mut self, work: u64, at: Location) -> Result<(), Fault> {
        let next = self.usage.arithmetic_steps.checked_add(1).ok_or_else(|| {
            Fault::exhausted(
                BudgetKind::ArithmeticSteps,
                self.limits.arithmetic_steps,
                u64::MAX,
                at,
            )
        })?;
        self.check(BudgetKind::ArithmeticSteps, next, at)?;
        self.work(work, at)?;
        self.usage.arithmetic_steps = next;
        Ok(())
    }

    /// Prepay one syntactic condition visit and its elementary cost.
    pub(crate) fn condition(&mut self, work: u64, at: Location) -> Result<(), Fault> {
        let next = self
            .usage
            .condition_steps
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::ConditionSteps, at))?;
        self.check(BudgetKind::ConditionSteps, next, at)?;
        self.work(work, at)?;
        self.usage.condition_steps = next;
        Ok(())
    }
}

/// Complete stable order for effective-limit records.
pub(crate) const ALL_KINDS: [BudgetKind; 37] = [
    BudgetKind::SourceBytes,
    BudgetKind::NameBytes,
    BudgetKind::OriginalOccurrences,
    BudgetKind::Depth,
    BudgetKind::Vertices,
    BudgetKind::Edges,
    BudgetKind::StagedVertices,
    BudgetKind::StagedEdges,
    BudgetKind::PayloadBytes,
    BudgetKind::StagedPayloadBytes,
    BudgetKind::Work,
    BudgetKind::ArithmeticSteps,
    BudgetKind::ComponentBits,
    BudgetKind::IntermediateBits,
    BudgetKind::LiteralBytes,
    BudgetKind::DecimalExponent,
    BudgetKind::IntegerExponent,
    BudgetKind::PolynomialDegree,
    BudgetKind::PolynomialVariables,
    BudgetKind::PolynomialTerms,
    BudgetKind::MatrixEntries,
    BudgetKind::ConditionAtoms,
    BudgetKind::ConditionSteps,
    BudgetKind::MatchAttempts,
    BudgetKind::MatchBatch,
    BudgetKind::FrontierLabels,
    BudgetKind::QueueEntries,
    BudgetKind::Transitions,
    BudgetKind::ExpansionCredits,
    BudgetKind::PeakExpressionSize,
    BudgetKind::ProofNodes,
    BudgetKind::WitnessBytes,
    BudgetKind::OutputBytes,
    BudgetKind::DiagnosticRecords,
    BudgetKind::ReplayWork,
    BudgetKind::CostQuota,
    BudgetKind::ProgressQuota,
];
