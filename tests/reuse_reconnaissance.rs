mod support;

use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use serde_json::Value;
use uuid::Uuid;

use fa_local::adapters::execution_delivery::sealed_corpus_read::{
    SealedCorpusReadAdapter, SealedCorpusReadRequest,
};
use fa_local::app::execution_service::CoordinationContext;
use fa_local::app::reuse_reconnaissance_service::ReuseReconnaissanceService;
use fa_local::domain::capabilities::{
    CapabilityRegistry, CapabilityRegistryLoader, CapabilityType,
};
use fa_local::domain::execution::{
    CancellationPolicy, CompletionPolicy, ExecutionPlan, ExecutionPlanStep,
    ExecutionPlanValidator, ExecutionRequest, RequestIntent,
};
use fa_local::domain::policy::PolicyArtifactLoader;
use fa_local::domain::posture::{
    ApprovalPostureResolver, RouteResolutionContext, RouteResolutionInput,
};
use fa_local::domain::requester_trust::RequesterTrustEngine;
use fa_local::domain::reuse_reconnaissance::{
    DonorDisposition, FRAA_CP1_STEP_IDS, ReuseMode, ReuseReconnaissanceInput,
    ReuseReconnaissanceResult,
};
use fa_local::{
    ApprovalPosture, CapabilityId, CorrelationId, EnvironmentMode, ExecutionPlanId,
    ExecutionState, RequestId, RequesterId, RouteDecisionId, SideEffectClass,
};

const CAPABILITY_ID: &str = "77777777-7777-4777-8777-777777777777";
const REQUEST_ID: &str = "55555555-5555-4555-8555-555555555555";
const CORRELATION_ID: &str = "66666666-6666-4666-8666-666666666666";
const PLAN_ID: &str = "99999999-9999-4999-8999-999999999999";
const ROUTE_ID: &str = "88888888-8888-4888-8888-888888888888";

fn parse_uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}

fn fraa_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fraa/tarcie_cp0")
}

fn candidate_visible_root() -> PathBuf {
    fraa_fixture_root().join("candidate-visible")
}

fn load_input() -> ReuseReconnaissanceInput {
    let raw = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("schemas/examples/fraa/valid-input.json"),
    )
    .unwrap();
    let value: Value = serde_json::from_str(&raw).unwrap();
    ReuseReconnaissanceInput::load_contract_value(&value).unwrap()
}

fn load_registry() -> CapabilityRegistry {
    let value = support::load_fixture_json(
        "valid",
        "capability-registry-local-file-read.json",
    );
    CapabilityRegistryLoader::load_contract_value(&value).unwrap()
}

fn admitted_route(input: &ReuseReconnaissanceInput, registry: &CapabilityRegistry) -> fa_local::domain::routing::RouteDecision {
    let mut requester_value =
        support::load_fixture_json("valid", "requester-trust-basic.json");
    requester_value["requester_id"] = Value::String(
        "11111111-1111-4111-8111-111111111111".to_owned(),
    );
    requester_value["requester_class"] =
        Value::String("development_test_surface".to_owned());
    requester_value["environment_mode"] = Value::String("test_harness".to_owned());
    let requester = RequesterTrustEngine::load_contract_value(&requester_value).unwrap();

    let mut policy_value = support::load_fixture_json("valid", "policy-artifact-basic.json");
    policy_value["scope"]["environment_modes"] =
        serde_json::json!(["test_harness"]);
    policy_value["capability_rules"][0]["capability_id"] =
        Value::String(CAPABILITY_ID.to_owned());
    policy_value["capability_rules"][0]["allowed_requester_classes"] =
        serde_json::json!(["development_test_surface"]);
    policy_value["capability_rules"][0]["allowed_side_effect_classes"] =
        serde_json::json!(["none"]);
    policy_value["side_effect_rules"] =
        serde_json::json!([{"side_effect_class": "none", "allowed": true}]);
    policy_value["approval_rules"] = serde_json::json!([{
        "requester_class": "development_test_surface",
        "max_posture": "execute_allowed"
    }]);
    policy_value["environment_conditions"] = serde_json::json!(["test_harness"]);
    let policy = PolicyArtifactLoader::load_contract_value(&policy_value).unwrap();

    let request = ExecutionRequest {
        request_id: RequestId::from_uuid(parse_uuid(REQUEST_ID)),
        correlation_id: CorrelationId::from_uuid(parse_uuid(CORRELATION_ID)),
        requester_id: RequesterId::from_uuid(
            parse_uuid("11111111-1111-4111-8111-111111111111"),
        ),
        environment_mode: EnvironmentMode::TestHarness,
        requested_capability_id: input.capability_id,
        requested_side_effect_class: SideEffectClass::None,
        intent: RequestIntent::ExecuteCapability,
        intent_summary: "analyze sealed Tarcie CP0 candidate-visible evidence".to_owned(),
        requested_at: Utc.with_ymd_and_hms(2026, 8, 28, 6, 40, 0).unwrap(),
    };

    let admitted = CapabilityRegistryLoader::admit_execution_request(
        registry,
        &policy,
        &requester,
        &request,
    );
    assert!(admitted.is_ok());

    ApprovalPostureResolver::resolve(
        RouteResolutionInput {
            request,
            requester_trust_outcome: Ok(requester),
            policy_outcome: Ok(policy),
            capability_admission_outcome: admitted,
        },
        RouteResolutionContext::new(
            RouteDecisionId::from_uuid(parse_uuid(ROUTE_ID)),
            Utc.with_ymd_and_hms(2026, 8, 28, 6, 40, 1).unwrap(),
        ),
    )
}

fn cp1_plan(input: &ReuseReconnaissanceInput) -> ExecutionPlan {
    let mut plan = ExecutionPlan {
        execution_plan_id: ExecutionPlanId::from_uuid(parse_uuid(PLAN_ID)),
        correlation_id: CorrelationId::from_uuid(parse_uuid(CORRELATION_ID)),
        originating_request_id: RequestId::from_uuid(parse_uuid(REQUEST_ID)),
        steps: FRAA_CP1_STEP_IDS
            .iter()
            .map(|step_id| ExecutionPlanStep {
                step_id: (*step_id).to_owned(),
                capability_id: input.capability_id,
                declared_side_effect_class: SideEffectClass::None,
                timeout_budget_ms: 500,
            })
            .collect(),
        referenced_capabilities: vec![input.capability_id],
        declared_max_step_count: 6,
        declared_side_effect_classes: vec![SideEffectClass::None],
        fallback_references: Vec::new(),
        cancellation_policy: CancellationPolicy::CancelRemainingSteps,
        completion_policy: CompletionPolicy::AllStepsMustSucceed,
        max_duration_budget_ms: 5_000,
        stable_plan_hash: String::new(),
        planned_at_utc: Utc.with_ymd_and_hms(2026, 8, 28, 6, 40, 2).unwrap(),
    };
    plan.stable_plan_hash = ExecutionPlanValidator::compute_stable_plan_hash(&plan);
    plan
}

#[test]
fn local_file_read_is_an_explicit_schema_backed_capability() {
    let registry = load_registry();
    let capability = &registry.capabilities[0];
    assert_eq!(capability.capability_type, CapabilityType::LocalFileRead);
    assert_eq!(capability.side_effect_class, SideEffectClass::None);
    assert_eq!(
        serde_json::to_string(&capability.capability_type).unwrap(),
        "\"local_file_read\""
    );
}

#[test]
fn repo_local_fraa_input_and_result_contracts_are_fail_closed() {
    let input = load_input();
    assert_eq!(
        input.capability_id,
        CapabilityId::from_uuid(parse_uuid(CAPABILITY_ID))
    );

    let mut invalid_input_value: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("schemas/examples/fraa/valid-input.json"),
        )
        .unwrap(),
    )
    .unwrap();
    invalid_input_value["authority"]["hidden_oracle_allowed"] = Value::Bool(true);
    assert!(ReuseReconnaissanceInput::load_contract_value(&invalid_input_value).is_err());

    let oracle_raw =
        std::fs::read_to_string(fraa_fixture_root().join("oracle/expected-result.json"))
            .unwrap();
    let oracle_value: Value = serde_json::from_str(&oracle_raw).unwrap();
    assert!(ReuseReconnaissanceResult::load_contract_value(&oracle_value).is_ok());

    let mut invalid_result_value = oracle_value;
    invalid_result_value["donor_candidates"][0]["reuse_mode"] =
        Value::String("copy_everything".to_owned());
    assert!(ReuseReconnaissanceResult::load_contract_value(&invalid_result_value).is_err());
}

#[test]
fn sealed_reader_verifies_every_member_and_aggregate_without_oracle_access() {
    let input = load_input();
    let receipt = SealedCorpusReadAdapter
        .read_candidate_visible(&SealedCorpusReadRequest {
            allowed_root: candidate_visible_root(),
            manifest_relative_path: input.corpus_manifest_path.clone(),
            max_total_bytes: 1024 * 1024,
        })
        .unwrap();

    assert_eq!(receipt.member_count, 4);
    assert_eq!(
        receipt.aggregate_sha256,
        input.expected_corpus_aggregate_sha256
    );
    assert!(
        receipt
            .documents
            .iter()
            .all(|document| !document.relative_path.contains("oracle"))
    );
}

#[test]
fn sealed_reader_rejects_parent_traversal_and_digest_tampering() {
    let scratch = ScratchDir::new("rejections");
    std::fs::write(scratch.path.join("visible.json"), b"{}\n").unwrap();
    std::fs::create_dir_all(scratch.path.join("oracle")).unwrap();
    std::fs::write(scratch.path.join("oracle/answer.json"), b"{}\n").unwrap();

    let traversal_manifest = serde_json::json!({
        "schema_version": "FraaSealedCorpusManifest.v0",
        "corpus_id": "bad-traversal",
        "exposure_class": "candidate_visible",
        "aggregate_profile": "path-sha256-size-v1",
        "members": [{"path": "../oracle/answer.json", "sha256": "00".repeat(32), "size_bytes": 3}]
    });
    std::fs::write(
        scratch.path.join("manifest.json"),
        serde_json::to_vec_pretty(&traversal_manifest).unwrap(),
    )
    .unwrap();
    assert!(
        SealedCorpusReadAdapter
            .read_candidate_visible(&SealedCorpusReadRequest {
                allowed_root: scratch.path.clone(),
                manifest_relative_path: "manifest.json".to_owned(),
                max_total_bytes: 1024,
            })
            .is_err()
    );

    let tampered_manifest = serde_json::json!({
        "schema_version": "FraaSealedCorpusManifest.v0",
        "corpus_id": "bad-digest",
        "exposure_class": "candidate_visible",
        "aggregate_profile": "path-sha256-size-v1",
        "members": [{"path": "visible.json", "sha256": "00".repeat(32), "size_bytes": 3}]
    });
    std::fs::write(
        scratch.path.join("manifest.json"),
        serde_json::to_vec_pretty(&tampered_manifest).unwrap(),
    )
    .unwrap();
    assert!(
        SealedCorpusReadAdapter
            .read_candidate_visible(&SealedCorpusReadRequest {
                allowed_root: scratch.path.clone(),
                manifest_relative_path: "manifest.json".to_owned(),
                max_total_bytes: 1024,
            })
            .is_err()
    );
}

#[test]
fn fa_local_runs_the_authorized_six_step_plan_and_matches_hidden_oracle() {
    let input = load_input();
    let registry = load_registry();
    let route = admitted_route(&input, &registry);
    assert_eq!(
        route.resolved_approval_posture,
        ApprovalPosture::PolicyPreapproved
    );
    let plan = cp1_plan(&input);
    let validated = ExecutionPlanValidator::validate(&plan, &registry).unwrap();
    assert_eq!(validated.plan.steps.len(), 6);

    let run = ReuseReconnaissanceService
        .run(
            &input,
            route,
            &plan,
            &registry,
            &candidate_visible_root(),
            &SealedCorpusReadAdapter,
            CoordinationContext::new(
                Utc.with_ymd_and_hms(2026, 8, 28, 6, 40, 3).unwrap(),
                Utc.with_ymd_and_hms(2026, 8, 28, 6, 40, 4).unwrap(),
                Utc.with_ymd_and_hms(2026, 8, 28, 6, 40, 5).unwrap(),
            ),
        )
        .unwrap();

    assert_eq!(
        run.execution_trace.final_status().status.state,
        ExecutionState::Completed
    );
    assert_eq!(
        run.result.selected_topology,
        "new_private_monorepo_separate_deployables"
    );
    assert_eq!(
        run.result
            .donor_candidates
            .iter()
            .find(|candidate| candidate.capability_id == "legacy-tarcie-outbox")
            .unwrap()
            .reuse_mode,
        ReuseMode::BoundedExtraction
    );
    assert_eq!(
        run.result
            .donor_candidates
            .iter()
            .filter(|candidate| candidate.disposition == DonorDisposition::Rejected)
            .count(),
        0
    );

    // The candidate runtime receives only candidate-visible documents. The test
    // harness reads the sibling oracle after the result is frozen.
    let oracle_raw =
        std::fs::read_to_string(fraa_fixture_root().join("oracle/expected-result.json"))
            .unwrap();
    let oracle_value: Value = serde_json::from_str(&oracle_raw).unwrap();
    let oracle = ReuseReconnaissanceResult::load_contract_value(&oracle_value).unwrap();
    assert_eq!(run.result, oracle);
}

#[test]
fn deterministic_replay_produces_identical_result_hash() {
    let input = load_input();
    let registry = load_registry();
    let plan = cp1_plan(&input);

    let first = ReuseReconnaissanceService
        .run(
            &input,
            admitted_route(&input, &registry),
            &plan,
            &registry,
            &candidate_visible_root(),
            &SealedCorpusReadAdapter,
            CoordinationContext::default(),
        )
        .unwrap();
    let second = ReuseReconnaissanceService
        .run(
            &input,
            admitted_route(&input, &registry),
            &plan,
            &registry,
            &candidate_visible_root(),
            &SealedCorpusReadAdapter,
            CoordinationContext::default(),
        )
        .unwrap();

    assert_eq!(first.result.result_hash, second.result.result_hash);
    assert_eq!(first.result, second.result);
}

struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "fa-local-fraa-{label}-{}",
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
