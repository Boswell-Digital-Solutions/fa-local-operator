use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use jsonschema::draft202012;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::domain::shared::CapabilityId;
use crate::errors::{FaLocalError, FaLocalResult};

pub const FRAA_CP1_STEP_IDS: [&str; 6] = [
    "validate_sealed_manifest",
    "read_candidate_visible",
    "normalize_evidence",
    "classify_donor_capabilities",
    "select_topology",
    "freeze_reconnaissance_result",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetRepositoryPosture {
    NewPrivateApplication,
    ExistingRepositoryRefactor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReuseMode {
    DirectDependency,
    Vendoring,
    BoundedExtraction,
    InterfaceReuse,
    TestVectorReuse,
    PatternReimplementation,
    ServiceConsumption,
    Hold,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DonorDisposition {
    QualifiedCandidate,
    Held,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass {
    PinnedCiAndSourceReview,
    SourceReviewAndTests,
    SourceReviewOnly,
    Unproven,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconnaissanceAuthority {
    pub execution_owner: String,
    pub network_allowed: bool,
    pub model_allowed: bool,
    pub repository_mutation_allowed: bool,
    pub hidden_oracle_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReuseReconnaissanceInput {
    pub schema_version: String,
    pub run_id: String,
    pub capability_id: CapabilityId,
    pub corpus_manifest_path: String,
    pub expected_corpus_aggregate_sha256: String,
    pub target_application: String,
    pub target_repository_posture: TargetRepositoryPosture,
    pub required_capabilities: Vec<String>,
    pub prohibited_effects: Vec<String>,
    pub authority: ReconnaissanceAuthority,
}

impl ReuseReconnaissanceInput {
    pub fn load_contract_value(value: &Value) -> FaLocalResult<Self> {
        validate_fraa_contract(FraaSchemaName::ReconnaissanceInput, value)?;
        Ok(serde_json::from_value(value.clone())?)
    }

    pub fn validate_authority_boundary(&self) -> FaLocalResult<()> {
        if self.authority.execution_owner != "fa-local-operator" {
            return Err(contract_invalid(
                "FRAA CP1 execution owner must be fa-local-operator",
            ));
        }
        if self.authority.network_allowed
            || self.authority.model_allowed
            || self.authority.repository_mutation_allowed
            || self.authority.hidden_oracle_allowed
        {
            return Err(contract_invalid(
                "FRAA CP1 authority boundary must deny network, model, mutation, and oracle access",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusDocument {
    pub relative_path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DonorCandidateDecision {
    pub capability_id: String,
    pub source_repo: String,
    pub source_commit: String,
    pub source_paths: Vec<String>,
    pub evidence_class: EvidenceClass,
    pub reuse_mode: ReuseMode,
    pub disposition: DonorDisposition,
    pub blockers: Vec<String>,
    pub preserved_tests: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyAssessment {
    pub topology_id: String,
    pub repository_posture: String,
    pub hard_blockers: Vec<String>,
    pub complexity_rank: u32,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityBoundaryResult {
    pub topology_owner: String,
    pub operator_surface: String,
    pub execution_owner: String,
    pub mutation_owner: String,
    pub final_decision_owner: String,
    pub network_allowed: bool,
    pub model_allowed: bool,
    pub repository_mutation_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftAssemblyBlueprint {
    pub repository_posture: String,
    pub deployables: Vec<String>,
    pub donor_capability_ids: Vec<String>,
    pub verification_obligations: Vec<String>,
    pub prohibited_effects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReuseReconnaissanceResult {
    pub schema_version: String,
    pub run_id: String,
    pub corpus_aggregate_sha256: String,
    pub donor_candidates: Vec<DonorCandidateDecision>,
    pub critical_blockers: Vec<String>,
    pub topology_alternatives: Vec<TopologyAssessment>,
    pub selected_topology: String,
    pub authority_boundary: AuthorityBoundaryResult,
    pub draft_blueprint: DraftAssemblyBlueprint,
    pub result_hash: String,
}

impl ReuseReconnaissanceResult {
    pub fn load_contract_value(value: &Value) -> FaLocalResult<Self> {
        validate_fraa_contract(FraaSchemaName::ReconnaissanceResult, value)?;
        let result: Self = serde_json::from_value(value.clone())?;
        result.verify_hash()?;
        Ok(result)
    }

    pub fn compute_hash(&self) -> String {
        let mut canonical = self.clone();
        canonical.result_hash.clear();
        let bytes = serde_json::to_vec(&canonical)
            .expect("FRAA reconnaissance result serialization must succeed");
        sha256_hex(&bytes)
    }

    pub fn verify_hash(&self) -> FaLocalResult<()> {
        let expected = self.compute_hash();
        if self.result_hash != expected {
            return Err(contract_invalid(
                "FRAA reconnaissance result hash does not match canonical result content",
            ));
        }
        Ok(())
    }

    fn freeze(mut self) -> FaLocalResult<Self> {
        self.result_hash = self.compute_hash();
        let value = serde_json::to_value(&self)?;
        validate_fraa_contract(FraaSchemaName::ReconnaissanceResult, &value)?;
        self.verify_hash()?;
        Ok(self)
    }
}

#[derive(Debug, Default)]
pub struct DeterministicReuseReconnaissanceEngine;

impl DeterministicReuseReconnaissanceEngine {
    pub fn analyze(
        &self,
        input: &ReuseReconnaissanceInput,
        corpus_aggregate_sha256: &str,
        documents: &[CorpusDocument],
    ) -> FaLocalResult<ReuseReconnaissanceResult> {
        input.validate_authority_boundary()?;
        if corpus_aggregate_sha256 != input.expected_corpus_aggregate_sha256 {
            return Err(contract_invalid(
                "sealed corpus aggregate does not match FRAA input binding",
            ));
        }

        let mut target_brief = None;
        let mut donor_evidence = None;
        let mut topology_options = None;
        let mut constraints = None;

        for document in documents {
            let value: Value = serde_json::from_str(&document.content)?;
            let kind = value.get("kind").and_then(Value::as_str).ok_or_else(|| {
                contract_invalid(format!(
                    "candidate-visible document {} has no kind",
                    document.relative_path
                ))
            })?;
            match kind {
                "target_brief" => set_once(
                    &mut target_brief,
                    serde_json::from_value(value)?,
                    "target_brief",
                )?,
                "donor_evidence" => set_once(
                    &mut donor_evidence,
                    serde_json::from_value(value)?,
                    "donor_evidence",
                )?,
                "topology_options" => set_once(
                    &mut topology_options,
                    serde_json::from_value(value)?,
                    "topology_options",
                )?,
                "constraints" => set_once(
                    &mut constraints,
                    serde_json::from_value(value)?,
                    "constraints",
                )?,
                other => {
                    return Err(contract_invalid(format!(
                        "unsupported candidate-visible document kind {other}"
                    )))
                }
            }
        }

        let target = target_brief.ok_or_else(|| contract_invalid("missing target_brief document"))?;
        let donors = donor_evidence
            .ok_or_else(|| contract_invalid("missing donor_evidence document"))?;
        let topologies = topology_options
            .ok_or_else(|| contract_invalid("missing topology_options document"))?;
        let constraints = constraints
            .ok_or_else(|| contract_invalid("missing constraints document"))?;

        if target.application != input.target_application
            || target.repository_posture != repository_posture_label(input.target_repository_posture)
        {
            return Err(contract_invalid(
                "target brief does not match FRAA input target binding",
            ));
        }

        let mut donor_candidates = donors
            .donors
            .into_iter()
            .map(classify_donor)
            .collect::<FaLocalResult<Vec<_>>>()?;
        donor_candidates.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));

        let selected = select_topology(&target, &topologies.options)?;
        let mut topology_alternatives = topologies
            .options
            .into_iter()
            .map(|option| TopologyAssessment {
                selected: option.topology_id == selected.topology_id,
                topology_id: option.topology_id,
                repository_posture: option.repository_posture,
                hard_blockers: sorted_unique(option.hard_blockers),
                complexity_rank: option.complexity_rank,
            })
            .collect::<Vec<_>>();
        topology_alternatives.sort_by(|left, right| left.topology_id.cmp(&right.topology_id));

        let mut critical_blockers = constraints.critical_blockers;
        for donor in &donor_candidates {
            critical_blockers.extend(donor.blockers.iter().cloned());
        }
        critical_blockers = sorted_unique(critical_blockers);

        let donor_capability_ids = donor_candidates
            .iter()
            .filter(|candidate| candidate.disposition != DonorDisposition::Rejected)
            .map(|candidate| candidate.capability_id.clone())
            .collect::<Vec<_>>();

        ReuseReconnaissanceResult {
            schema_version: "FraaReconnaissanceResult.v0".to_owned(),
            run_id: input.run_id.clone(),
            corpus_aggregate_sha256: corpus_aggregate_sha256.to_owned(),
            donor_candidates,
            critical_blockers,
            topology_alternatives,
            selected_topology: selected.topology_id,
            authority_boundary: AuthorityBoundaryResult {
                topology_owner: target.authority.topology_owner,
                operator_surface: target.authority.operator_surface,
                execution_owner: target.authority.execution_owner,
                mutation_owner: target.authority.mutation_owner,
                final_decision_owner: target.authority.final_decision_owner,
                network_allowed: false,
                model_allowed: false,
                repository_mutation_allowed: false,
            },
            draft_blueprint: DraftAssemblyBlueprint {
                repository_posture: selected.repository_posture,
                deployables: selected.deployables,
                donor_capability_ids,
                verification_obligations: sorted_unique(constraints.verification_obligations),
                prohibited_effects: sorted_unique(input.prohibited_effects.clone()),
            },
            result_hash: String::new(),
        }
        .freeze()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TargetBriefDocument {
    application: String,
    repository_posture: String,
    separate_trust_actors_required: bool,
    #[allow(dead_code)]
    required_capabilities: Vec<String>,
    authority: TargetAuthorityDocument,
}

#[derive(Debug, Clone, Deserialize)]
struct TargetAuthorityDocument {
    topology_owner: String,
    operator_surface: String,
    execution_owner: String,
    mutation_owner: String,
    final_decision_owner: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DonorEvidenceDocument {
    donors: Vec<DonorEvidenceRecord>,
}

#[derive(Debug, Clone, Deserialize)]
struct DonorEvidenceRecord {
    capability_id: String,
    source_repo: String,
    source_commit: String,
    source_paths: Vec<String>,
    behavior_proven: bool,
    evidence_class: EvidenceClass,
    allowed_modes: Vec<ReuseMode>,
    prohibited_modes: Vec<ReuseMode>,
    blockers: Vec<String>,
    preserved_tests: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TopologyOptionsDocument {
    options: Vec<TopologyOptionRecord>,
}

#[derive(Debug, Clone, Deserialize)]
struct TopologyOptionRecord {
    topology_id: String,
    repository_posture: String,
    supports_new_application: bool,
    separate_trust_actors: bool,
    complexity_rank: u32,
    deployables: Vec<String>,
    hard_blockers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ConstraintsDocument {
    critical_blockers: Vec<String>,
    verification_obligations: Vec<String>,
}

fn classify_donor(donor: DonorEvidenceRecord) -> FaLocalResult<DonorCandidateDecision> {
    let reuse_mode = select_reuse_mode(&donor);
    let blockers = sorted_unique(donor.blockers);
    let disposition = if reuse_mode == ReuseMode::Reject {
        DonorDisposition::Rejected
    } else if blockers.is_empty() {
        DonorDisposition::QualifiedCandidate
    } else {
        DonorDisposition::Held
    };

    let rationale = match disposition {
        DonorDisposition::Rejected => {
            "No admitted reuse mode remains after evidence and prohibition checks.".to_owned()
        }
        DonorDisposition::Held => format!(
            "Behavior is proven, but {} blocker(s) require resolution before {}.",
            blockers.len(),
            reuse_mode_label(reuse_mode)
        ),
        DonorDisposition::QualifiedCandidate => format!(
            "Behavior is proven and {} is the highest-priority compatible reuse mode.",
            reuse_mode_label(reuse_mode)
        ),
    };

    Ok(DonorCandidateDecision {
        capability_id: donor.capability_id,
        source_repo: donor.source_repo,
        source_commit: donor.source_commit,
        source_paths: sorted_unique(donor.source_paths),
        evidence_class: donor.evidence_class,
        reuse_mode,
        disposition,
        blockers,
        preserved_tests: sorted_unique(donor.preserved_tests),
        rationale,
    })
}

fn select_reuse_mode(donor: &DonorEvidenceRecord) -> ReuseMode {
    if !donor.behavior_proven {
        return ReuseMode::Reject;
    }

    let allowed = donor
        .allowed_modes
        .iter()
        .copied()
        .filter(|mode| !donor.prohibited_modes.contains(mode))
        .collect::<BTreeSet<_>>();

    [
        ReuseMode::ServiceConsumption,
        ReuseMode::InterfaceReuse,
        ReuseMode::BoundedExtraction,
        ReuseMode::TestVectorReuse,
        ReuseMode::PatternReimplementation,
        ReuseMode::DirectDependency,
        ReuseMode::Vendoring,
    ]
    .into_iter()
    .find(|mode| allowed.contains(mode))
    .unwrap_or(ReuseMode::Reject)
}

fn select_topology(
    target: &TargetBriefDocument,
    options: &[TopologyOptionRecord],
) -> FaLocalResult<TopologyOptionRecord> {
    let mut eligible = options
        .iter()
        .filter(|option| option.hard_blockers.is_empty())
        .filter(|option| {
            target.repository_posture != "new_private_application"
                || option.supports_new_application
        })
        .filter(|option| {
            !target.separate_trust_actors_required || option.separate_trust_actors
        })
        .cloned()
        .collect::<Vec<_>>();
    eligible.sort_by(|left, right| {
        left.complexity_rank
            .cmp(&right.complexity_rank)
            .then(left.topology_id.cmp(&right.topology_id))
    });
    eligible
        .into_iter()
        .next()
        .ok_or_else(|| contract_invalid("no topology option satisfies target constraints"))
}

fn set_once<T>(slot: &mut Option<T>, value: T, kind: &str) -> FaLocalResult<()> {
    if slot.replace(value).is_some() {
        return Err(contract_invalid(format!(
            "duplicate candidate-visible document kind {kind}"
        )));
    }
    Ok(())
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values.into_iter().collect::<BTreeSet<_>>().into_iter().collect()
}

fn repository_posture_label(posture: TargetRepositoryPosture) -> &'static str {
    match posture {
        TargetRepositoryPosture::NewPrivateApplication => "new_private_application",
        TargetRepositoryPosture::ExistingRepositoryRefactor => "existing_repository_refactor",
    }
}

fn reuse_mode_label(mode: ReuseMode) -> &'static str {
    match mode {
        ReuseMode::DirectDependency => "direct_dependency",
        ReuseMode::Vendoring => "vendoring",
        ReuseMode::BoundedExtraction => "bounded_extraction",
        ReuseMode::InterfaceReuse => "interface_reuse",
        ReuseMode::TestVectorReuse => "test_vector_reuse",
        ReuseMode::PatternReimplementation => "pattern_reimplementation",
        ReuseMode::ServiceConsumption => "service_consumption",
        ReuseMode::Hold => "hold",
        ReuseMode::Reject => "reject",
    }
}

#[derive(Debug, Clone, Copy)]
enum FraaSchemaName {
    ReconnaissanceInput,
    ReconnaissanceResult,
}

impl FraaSchemaName {
    fn file_name(self) -> &'static str {
        match self {
            Self::ReconnaissanceInput => "fraa-reconnaissance-input.v0.schema.json",
            Self::ReconnaissanceResult => "fraa-reconnaissance-result.v0.schema.json",
        }
    }

    fn path(self) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("schemas")
            .join(self.file_name())
    }
}

fn validate_fraa_contract(schema_name: FraaSchemaName, value: &Value) -> FaLocalResult<()> {
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(schema_name.path())?)?;
    let validator = draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| FaLocalError::SchemaCompile {
            schema: schema_name.file_name().to_owned(),
            message: error.to_string(),
        })?;
    let errors = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(FaLocalError::SchemaValidation {
            schema: schema_name.file_name().to_owned(),
            errors,
        })
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        write!(&mut out, "{byte:02x}").expect("writing SHA-256 digest must succeed");
    }
    out
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}
