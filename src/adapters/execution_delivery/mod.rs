pub mod local_file_write;
pub mod nmap_preflight;

use crate::domain::execution::FallbackReference;
use crate::domain::shared::{
    ApprovalPosture, CapabilityId, CorrelationId, DegradedSubtype, ExecutionPlanId, RequestId,
    RouteDecisionId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterDeliveryRequest {
    pub route_decision_id: RouteDecisionId,
    pub correlation_id: CorrelationId,
    pub request_id: RequestId,
    pub resolved_approval_posture: ApprovalPosture,
    pub requested_capability_id: CapabilityId,
    pub execution_plan_id: ExecutionPlanId,
    pub stable_plan_hash: String,
    pub declared_step_ids: Vec<String>,
    pub declared_capability_ids: Vec<CapabilityId>,
    pub declared_fallback_references: Vec<FallbackReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterDeliveryResult {
    DeliveredAllSteps,
    CompletedWithDeclaredFallback {
        step_id: String,
        fallback_step_id: String,
        degraded_subtype: DegradedSubtype,
    },
    FailedAtDeclaredStep {
        step_id: String,
        failure_summary: String,
    },
    CanceledAtDeclaredStep {
        step_id: String,
    },
    DependencyUnavailable {
        summary: String,
    },
    Unsupported {
        summary: String,
    },
}

pub trait ExternalRouteDeliveryAdapter {
    fn adapter_id(&self) -> &'static str;

    fn deliver_route(&self, request: &AdapterDeliveryRequest) -> AdapterDeliveryResult;
}
