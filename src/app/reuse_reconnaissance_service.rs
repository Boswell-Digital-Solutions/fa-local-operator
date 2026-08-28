use std::path::Path;

use crate::adapters::execution_delivery::sealed_corpus_read::{
    SealedCorpusReadAdapter, SealedCorpusReadReceipt, SealedCorpusReadRequest,
};
use crate::app::execution_service::{
    CoordinationContext, CoordinationDirective, CoordinationInput, ExecutionService, ExecutionTrace,
};
use crate::app::routing_service::{RoutingInput, RoutingService};
use crate::domain::capabilities::{CapabilityRegistry, CapabilityType};
use crate::domain::execution::{CompletionPolicy, ExecutionPlan, ExecutionPlanValidator};
use crate::domain::reuse_reconnaissance::{
    DeterministicReuseReconnaissanceEngine, FRAA_CP1_STEP_IDS, ReuseReconnaissanceInput,
    ReuseReconnaissanceResult,
};
use crate::domain::routing::RouteDecision;
use crate::domain::shared::{ApprovalPosture, SideEffectClass};
use crate::errors::{FaLocalError, FaLocalResult};

#[derive(Debug, Clone)]
pub struct ReuseReconnaissanceRun {
    pub corpus_receipt: SealedCorpusReadReceipt,
    pub result: ReuseReconnaissanceResult,
    pub execution_trace: ExecutionTrace,
}

#[derive(Debug, Default)]
pub struct ReuseReconnaissanceService;

impl ReuseReconnaissanceService {
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        input: &ReuseReconnaissanceInput,
        route_decision: RouteDecision,
        plan: &ExecutionPlan,
        registry: &CapabilityRegistry,
        corpus_root: &Path,
        adapter: &SealedCorpusReadAdapter,
        context: CoordinationContext,
    ) -> FaLocalResult<ReuseReconnaissanceRun> {
        input.validate_authority_boundary()?;
        let capability = registry.capability_for(input.capability_id).ok_or_else(|| {
            contract_invalid("FRAA input references an unregistered read capability")
        })?;
        if capability.capability_type != CapabilityType::LocalFileRead
            || capability.side_effect_class != SideEffectClass::None
        {
            return Err(contract_invalid(
                "FRAA CP1 requires an explicit local_file_read capability with side_effect_class none",
            ));
        }

        let validated_plan = ExecutionPlanValidator::validate(plan, registry)?;
        validate_cp1_plan_shape(plan, input)?;

        if route_decision.resolved_approval_posture != ApprovalPosture::PolicyPreapproved
            || !route_decision.execution_allowed
        {
            return Err(contract_invalid(
                "FRAA CP1 route must be policy_preapproved and executable",
            ));
        }
        let selected_route = RoutingService.select_route(RoutingInput::new(
            route_decision.clone(),
            Some(validated_plan.clone()),
        )?)?;
        if !selected_route.executable {
            return Err(contract_invalid(
                "FRAA CP1 routing did not select an executable bounded route",
            ));
        }

        let corpus_receipt = adapter.read_candidate_visible(&SealedCorpusReadRequest {
            allowed_root: corpus_root.to_path_buf(),
            manifest_relative_path: input.corpus_manifest_path.clone(),
            max_total_bytes: 1024 * 1024,
        })?;

        let result = DeterministicReuseReconnaissanceEngine.analyze(
            input,
            &corpus_receipt.aggregate_sha256,
            &corpus_receipt.documents,
        )?;

        let execution_trace = ExecutionService.coordinate(CoordinationInput::new(
            route_decision,
            Some(validated_plan),
            CoordinationDirective::CompleteDeclaredPlan,
            context,
        )?)?;

        Ok(ReuseReconnaissanceRun {
            corpus_receipt,
            result,
            execution_trace,
        })
    }
}

fn validate_cp1_plan_shape(
    plan: &ExecutionPlan,
    input: &ReuseReconnaissanceInput,
) -> FaLocalResult<()> {
    let observed_steps = plan
        .steps
        .iter()
        .map(|step| step.step_id.as_str())
        .collect::<Vec<_>>();
    if observed_steps.as_slice() != FRAA_CP1_STEP_IDS.as_slice() {
        return Err(contract_invalid(
            "FRAA CP1 execution plan does not match the authorized six-step sequence",
        ));
    }
    if plan.completion_policy != CompletionPolicy::AllStepsMustSucceed
        || !plan.fallback_references.is_empty()
        || plan.referenced_capabilities != vec![input.capability_id]
        || plan.declared_side_effect_classes != vec![SideEffectClass::None]
        || plan.steps.iter().any(|step| {
            step.capability_id != input.capability_id
                || step.declared_side_effect_class != SideEffectClass::None
        })
    {
        return Err(contract_invalid(
            "FRAA CP1 execution plan broadens capability, side effect, or fallback scope",
        ));
    }
    Ok(())
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}
