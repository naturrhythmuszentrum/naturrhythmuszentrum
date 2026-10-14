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
    /// Associations in one case before sorting.
    CaseArms,
    /// New region records retained by this phase.
    RegionNodes,
    /// Cells in one exact partition, including singleton cuts.
    RegionCells,
    /// Attempted arm meets, including empty and repeated pairs.
    RefinementPairs,
    /// Local frames retained by this phase.
    LocalContexts,
    /// Maximum parent-frame depth.
    HypothesisDepth,
    /// Distinct locations in one claimed decomposition.
    BoundaryNodes,
    /// Highest requested trace order.
    JetOrder,
    /// Generated or replayed trace entries.
    JetEntries,
    /// Highest delta derivative order.
    DeltaOrder,
    /// Test or germ nodes visited by an action.
    TestNodes,
    /// Cumulative checked action expansions.
    PairingSteps,
    /// Reserved bound for proposed zero certificates.
    PullbackZeros,
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
    /// Associations in one case before sorting.
    pub case_arms: u64,
    /// New region records retained by this phase.
    pub region_nodes: u64,
    /// Cells in one exact partition, including singleton cuts.
    pub region_cells: u64,
    /// Attempted arm meets, including empty and repeated pairs.
    pub refinement_pairs: u64,
    /// Local frames retained by this phase.
    pub local_contexts: u64,
    /// Maximum parent-frame depth.
    pub hypothesis_depth: u64,
    /// Distinct locations in one claimed decomposition.
    pub boundary_nodes: u64,
    /// Highest requested trace order.
    pub jet_order: u64,
    /// Generated or replayed trace entries.
    pub jet_entries: u64,
    /// Highest delta derivative order.
    pub delta_order: u64,
    /// Test or germ nodes visited by an action.
    pub test_nodes: u64,
    /// Cumulative checked action expansions.
    pub pairing_steps: u64,
    /// Reserved bound for proposed zero certificates.
    pub pullback_zeros: u64,
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
            case_arms: 256,
            region_nodes: 2048,
            region_cells: 256,
            refinement_pairs: 4096,
            local_contexts: 128,
            hypothesis_depth: 32,
            boundary_nodes: 128,
            jet_order: 8,
            jet_entries: 4096,
            delta_order: 8,
            test_nodes: 256,
            pairing_steps: 4096,
            pullback_zeros: 64,
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
    /// Override for `case_arms`.
    pub case_arms: Option<u64>,
    /// Override for `region_nodes`.
    pub region_nodes: Option<u64>,
    /// Override for `region_cells`.
    pub region_cells: Option<u64>,
    /// Override for `refinement_pairs`.
    pub refinement_pairs: Option<u64>,
    /// Override for `local_contexts`.
    pub local_contexts: Option<u64>,
    /// Override for `hypothesis_depth`.
    pub hypothesis_depth: Option<u64>,
    /// Override for `boundary_nodes`.
    pub boundary_nodes: Option<u64>,
    /// Override for `jet_order`.
    pub jet_order: Option<u64>,
    /// Override for `jet_entries`.
    pub jet_entries: Option<u64>,
    /// Override for `delta_order`.
    pub delta_order: Option<u64>,
    /// Override for `test_nodes`.
    pub test_nodes: Option<u64>,
    /// Override for `pairing_steps`.
    pub pairing_steps: Option<u64>,
    /// Override for `pullback_zeros`.
    pub pullback_zeros: Option<u64>,
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
            BudgetKind::CaseArms => self.case_arms,
            BudgetKind::RegionNodes => self.region_nodes,
            BudgetKind::RegionCells => self.region_cells,
            BudgetKind::RefinementPairs => self.refinement_pairs,
            BudgetKind::LocalContexts => self.local_contexts,
            BudgetKind::HypothesisDepth => self.hypothesis_depth,
            BudgetKind::BoundaryNodes => self.boundary_nodes,
            BudgetKind::JetOrder => self.jet_order,
            BudgetKind::JetEntries => self.jet_entries,
            BudgetKind::DeltaOrder => self.delta_order,
            BudgetKind::TestNodes => self.test_nodes,
            BudgetKind::PairingSteps => self.pairing_steps,
            BudgetKind::PullbackZeros => self.pullback_zeros,
        }
    }

    /// Resolve every supplied override and validate the result.
    ///
    /// # Errors
    /// Returns an invalid-policy fault for inconsistent resolved values.
    pub fn resolve(&self, overrides: &LimitsOverride) -> Result<Self, Fault> {
        let mut resolved = Self::default();
        resolved.source_bytes = overrides.source_bytes.unwrap_or(resolved.source_bytes);
        resolved.name_bytes = overrides.name_bytes.unwrap_or(resolved.name_bytes);
        resolved.original_occurrences = overrides
            .original_occurrences
            .unwrap_or(resolved.original_occurrences);
        resolved.depth = overrides.depth.unwrap_or(resolved.depth);
        resolved.vertices = overrides.vertices.unwrap_or(resolved.vertices);
        resolved.edges = overrides.edges.unwrap_or(resolved.edges);
        resolved.staged_vertices = overrides
            .staged_vertices
            .unwrap_or(resolved.staged_vertices);
        resolved.staged_edges = overrides.staged_edges.unwrap_or(resolved.staged_edges);
        resolved.payload_bytes = overrides.payload_bytes.unwrap_or(resolved.payload_bytes);
        resolved.staged_payload_bytes = overrides
            .staged_payload_bytes
            .unwrap_or(resolved.staged_payload_bytes);
        resolved.work = overrides.work.unwrap_or(resolved.work);
        resolved.arithmetic_steps = overrides
            .arithmetic_steps
            .unwrap_or(resolved.arithmetic_steps);
        resolved.component_bits = overrides.component_bits.unwrap_or(resolved.component_bits);
        resolved.intermediate_bits = overrides
            .intermediate_bits
            .unwrap_or(resolved.intermediate_bits);
        resolved.literal_bytes = overrides.literal_bytes.unwrap_or(resolved.literal_bytes);
        resolved.decimal_exponent = overrides
            .decimal_exponent
            .unwrap_or(resolved.decimal_exponent);
        resolved.integer_exponent = overrides
            .integer_exponent
            .unwrap_or(resolved.integer_exponent);
        resolved.polynomial_degree = overrides
            .polynomial_degree
            .unwrap_or(resolved.polynomial_degree);
        resolved.polynomial_variables = overrides
            .polynomial_variables
            .unwrap_or(resolved.polynomial_variables);
        resolved.polynomial_terms = overrides
            .polynomial_terms
            .unwrap_or(resolved.polynomial_terms);
        resolved.matrix_entries = overrides.matrix_entries.unwrap_or(resolved.matrix_entries);
        resolved.condition_atoms = overrides
            .condition_atoms
            .unwrap_or(resolved.condition_atoms);
        resolved.condition_steps = overrides
            .condition_steps
            .unwrap_or(resolved.condition_steps);
        resolved.match_attempts = overrides.match_attempts.unwrap_or(resolved.match_attempts);
        resolved.match_batch = overrides.match_batch.unwrap_or(resolved.match_batch);
        resolved.frontier_labels = overrides
            .frontier_labels
            .unwrap_or(resolved.frontier_labels);
        resolved.queue_entries = overrides.queue_entries.unwrap_or(resolved.queue_entries);
        resolved.transitions = overrides.transitions.unwrap_or(resolved.transitions);
        resolved.expansion_credits = overrides
            .expansion_credits
            .unwrap_or(resolved.expansion_credits);
        resolved.peak_expression_size = overrides
            .peak_expression_size
            .unwrap_or(resolved.peak_expression_size);
        resolved.proof_nodes = overrides.proof_nodes.unwrap_or(resolved.proof_nodes);
        resolved.witness_bytes = overrides.witness_bytes.unwrap_or(resolved.witness_bytes);
        resolved.output_bytes = overrides.output_bytes.unwrap_or(resolved.output_bytes);
        resolved.diagnostic_records = overrides
            .diagnostic_records
            .unwrap_or(resolved.diagnostic_records);
        resolved.replay_work = overrides.replay_work.unwrap_or(resolved.replay_work);
        resolved.cost_quota = overrides.cost_quota.unwrap_or(resolved.cost_quota);
        resolved.progress_quota = overrides.progress_quota.unwrap_or(resolved.progress_quota);
        resolved.case_arms = overrides.case_arms.unwrap_or(resolved.case_arms);
        resolved.region_nodes = overrides.region_nodes.unwrap_or(resolved.region_nodes);
        resolved.region_cells = overrides.region_cells.unwrap_or(resolved.region_cells);
        resolved.refinement_pairs = overrides
            .refinement_pairs
            .unwrap_or(resolved.refinement_pairs);
        resolved.local_contexts = overrides.local_contexts.unwrap_or(resolved.local_contexts);
        resolved.hypothesis_depth = overrides
            .hypothesis_depth
            .unwrap_or(resolved.hypothesis_depth);
        resolved.boundary_nodes = overrides.boundary_nodes.unwrap_or(resolved.boundary_nodes);
        resolved.jet_order = overrides.jet_order.unwrap_or(resolved.jet_order);
        resolved.jet_entries = overrides.jet_entries.unwrap_or(resolved.jet_entries);
        resolved.delta_order = overrides.delta_order.unwrap_or(resolved.delta_order);
        resolved.test_nodes = overrides.test_nodes.unwrap_or(resolved.test_nodes);
        resolved.pairing_steps = overrides.pairing_steps.unwrap_or(resolved.pairing_steps);
        resolved.pullback_zeros = overrides.pullback_zeros.unwrap_or(resolved.pullback_zeros);
        resolved.intersect(self);
        resolved.validate()?;
        Ok(resolved)
    }

    /// Intersect every component with the permanent store ceiling.
    pub(crate) fn intersect(&mut self, store: &Self) {
        self.source_bytes = self.source_bytes.min(store.source_bytes);
        self.name_bytes = self.name_bytes.min(store.name_bytes);
        self.original_occurrences = self.original_occurrences.min(store.original_occurrences);
        self.depth = self.depth.min(store.depth);
        self.vertices = self.vertices.min(store.vertices);
        self.edges = self.edges.min(store.edges);
        self.staged_vertices = self.staged_vertices.min(store.staged_vertices);
        self.staged_edges = self.staged_edges.min(store.staged_edges);
        self.payload_bytes = self.payload_bytes.min(store.payload_bytes);
        self.staged_payload_bytes = self.staged_payload_bytes.min(store.staged_payload_bytes);
        self.work = self.work.min(store.work);
        self.arithmetic_steps = self.arithmetic_steps.min(store.arithmetic_steps);
        self.component_bits = self.component_bits.min(store.component_bits);
        self.intermediate_bits = self.intermediate_bits.min(store.intermediate_bits);
        self.literal_bytes = self.literal_bytes.min(store.literal_bytes);
        self.decimal_exponent = self.decimal_exponent.min(store.decimal_exponent);
        self.integer_exponent = self.integer_exponent.min(store.integer_exponent);
        self.polynomial_degree = self.polynomial_degree.min(store.polynomial_degree);
        self.polynomial_variables = self.polynomial_variables.min(store.polynomial_variables);
        self.polynomial_terms = self.polynomial_terms.min(store.polynomial_terms);
        self.matrix_entries = self.matrix_entries.min(store.matrix_entries);
        self.condition_atoms = self.condition_atoms.min(store.condition_atoms);
        self.condition_steps = self.condition_steps.min(store.condition_steps);
        self.match_attempts = self.match_attempts.min(store.match_attempts);
        self.match_batch = self.match_batch.min(store.match_batch);
        self.frontier_labels = self.frontier_labels.min(store.frontier_labels);
        self.queue_entries = self.queue_entries.min(store.queue_entries);
        self.transitions = self.transitions.min(store.transitions);
        self.expansion_credits = self.expansion_credits.min(store.expansion_credits);
        self.peak_expression_size = self.peak_expression_size.min(store.peak_expression_size);
        self.proof_nodes = self.proof_nodes.min(store.proof_nodes);
        self.witness_bytes = self.witness_bytes.min(store.witness_bytes);
        self.output_bytes = self.output_bytes.min(store.output_bytes);
        self.diagnostic_records = self.diagnostic_records.min(store.diagnostic_records);
        self.replay_work = self.replay_work.min(store.replay_work);
        self.cost_quota = self.cost_quota.min(store.cost_quota);
        self.progress_quota = self.progress_quota.min(store.progress_quota);
        self.case_arms = self.case_arms.min(store.case_arms);
        self.region_nodes = self.region_nodes.min(store.region_nodes);
        self.region_cells = self.region_cells.min(store.region_cells);
        self.refinement_pairs = self.refinement_pairs.min(store.refinement_pairs);
        self.local_contexts = self.local_contexts.min(store.local_contexts);
        self.hypothesis_depth = self.hypothesis_depth.min(store.hypothesis_depth);
        self.boundary_nodes = self.boundary_nodes.min(store.boundary_nodes);
        self.jet_order = self.jet_order.min(store.jet_order);
        self.jet_entries = self.jet_entries.min(store.jet_entries);
        self.delta_order = self.delta_order.min(store.delta_order);
        self.test_nodes = self.test_nodes.min(store.test_nodes);
        self.pairing_steps = self.pairing_steps.min(store.pairing_steps);
        self.pullback_zeros = self.pullback_zeros.min(store.pullback_zeros);
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
    /// Associations in one case before sorting.
    pub case_arms: u64,
    /// New region records retained by this phase.
    pub region_nodes: u64,
    /// Cells in one exact partition, including singleton cuts.
    pub region_cells: u64,
    /// Attempted arm meets, including empty and repeated pairs.
    pub refinement_pairs: u64,
    /// Local frames retained by this phase.
    pub local_contexts: u64,
    /// Maximum parent-frame depth.
    pub hypothesis_depth: u64,
    /// Distinct locations in one claimed decomposition.
    pub boundary_nodes: u64,
    /// Highest requested trace order.
    pub jet_order: u64,
    /// Generated or replayed trace entries.
    pub jet_entries: u64,
    /// Highest delta derivative order.
    pub delta_order: u64,
    /// Test or germ nodes visited by an action.
    pub test_nodes: u64,
    /// Cumulative checked action expansions.
    pub pairing_steps: u64,
    /// Reserved bound for proposed zero certificates.
    pub pullback_zeros: u64,
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

    /// Temporarily intersect every permanent ceiling with the active call policy.
    pub(crate) fn restrict_storage(&mut self, store: &Limits) -> Limits {
        let previous = self.limits.clone();
        self.limits.intersect(store);
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

    /// Prepay a specialized resource, counting cumulative work or observing a peak.
    pub(crate) fn specialized(
        &mut self,
        kind: BudgetKind,
        amount: u64,
        cumulative: bool,
        at: Location,
    ) -> Result<(), Fault> {
        let prior = match kind {
            BudgetKind::CaseArms => self.usage.case_arms,
            BudgetKind::RegionNodes => self.usage.region_nodes,
            BudgetKind::RegionCells => self.usage.region_cells,
            BudgetKind::RefinementPairs => self.usage.refinement_pairs,
            BudgetKind::LocalContexts => self.usage.local_contexts,
            BudgetKind::HypothesisDepth => self.usage.hypothesis_depth,
            BudgetKind::BoundaryNodes => self.usage.boundary_nodes,
            BudgetKind::JetOrder => self.usage.jet_order,
            BudgetKind::JetEntries => self.usage.jet_entries,
            BudgetKind::DeltaOrder => self.usage.delta_order,
            BudgetKind::TestNodes => self.usage.test_nodes,
            BudgetKind::PairingSteps => self.usage.pairing_steps,
            BudgetKind::PullbackZeros => self.usage.pullback_zeros,
            _ => return Err(Fault::invalid(InvalidCode::Policy, at)),
        };
        let next = if cumulative {
            prior
                .checked_add(amount)
                .ok_or_else(|| Fault::capacity(kind, at))?
        } else {
            prior.max(amount)
        };
        self.check(kind, next, at)?;
        self.work(1, at)?;
        match kind {
            BudgetKind::CaseArms => self.usage.case_arms = next,
            BudgetKind::RegionNodes => self.usage.region_nodes = next,
            BudgetKind::RegionCells => self.usage.region_cells = next,
            BudgetKind::RefinementPairs => self.usage.refinement_pairs = next,
            BudgetKind::LocalContexts => self.usage.local_contexts = next,
            BudgetKind::HypothesisDepth => self.usage.hypothesis_depth = next,
            BudgetKind::BoundaryNodes => self.usage.boundary_nodes = next,
            BudgetKind::JetOrder => self.usage.jet_order = next,
            BudgetKind::JetEntries => self.usage.jet_entries = next,
            BudgetKind::DeltaOrder => self.usage.delta_order = next,
            BudgetKind::TestNodes => self.usage.test_nodes = next,
            BudgetKind::PairingSteps => self.usage.pairing_steps = next,
            BudgetKind::PullbackZeros => self.usage.pullback_zeros = next,
            _ => return Err(Fault::invalid(InvalidCode::Policy, at)),
        }
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
pub(crate) const ALL_KINDS: [BudgetKind; 50] = [
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
    BudgetKind::CaseArms,
    BudgetKind::RegionNodes,
    BudgetKind::RegionCells,
    BudgetKind::RefinementPairs,
    BudgetKind::LocalContexts,
    BudgetKind::HypothesisDepth,
    BudgetKind::BoundaryNodes,
    BudgetKind::JetOrder,
    BudgetKind::JetEntries,
    BudgetKind::DeltaOrder,
    BudgetKind::TestNodes,
    BudgetKind::PairingSteps,
    BudgetKind::PullbackZeros,
];
