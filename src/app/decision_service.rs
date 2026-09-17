use serde_json::Value;

use crate::app::intake_service::IntakeService;
use crate::domain::capabilities::CapabilityRegistryLoader;
use crate::domain::guards::{DenialGuard, deny};
use crate::domain::policy::PolicyArtifactLoader;
use crate::domain::posture::{
    ApprovalPostureResolver, RouteResolutionContext, RouteResolutionInput,
};
use crate::domain::requester_trust::{RequesterTrustEngine, TrustEvaluationContext};
use crate::domain::routing::RouteDecision;
use crate::domain::shared::{DenialBasis, DenialReasonClass, DenialScope};
use crate::errors::FaLocalResult;

/// Wires intake, requester-trust evaluation, policy loading, and capability
/// admission into a single resolved route decision.
///
/// A malformed execution request or capability registry is an operator/config
/// error and returns `Err` — those are not per-request nuances. A failing
/// requester-trust or policy evaluation instead folds into the resolved
/// [`RouteDecision`] as a truthful `Denied` posture, matching how those two
/// domains already report their own failures ([`RequesterTrustEngine::load_and_evaluate`],
/// [`PolicyArtifactLoader::load_required_value`]).
#[derive(Debug, Default)]
pub struct DecisionService;

impl DecisionService {
    pub fn resolve_route_decision(
        &self,
        request: &Value,
        requester_trust: &Value,
        policy: &Value,
        capability_registry: &Value,
        context: RouteResolutionContext,
    ) -> FaLocalResult<RouteDecision> {
        let capability_registry =
            CapabilityRegistryLoader::load_contract_value(capability_registry)?;
        let request = IntakeService.validate_request(request)?.request;

        let trust_context = TrustEvaluationContext {
            expected_environment: request.environment_mode,
            now: context.decided_at_utc,
        };
        let requester_trust_outcome =
            RequesterTrustEngine::load_and_evaluate(requester_trust, &trust_context);
        let policy_outcome = PolicyArtifactLoader::load_required_value(Some(policy));

        let capability_admission_outcome = match (&requester_trust_outcome, &policy_outcome) {
            (Ok(requester), Ok(policy)) => CapabilityRegistryLoader::admit_execution_request(
                &capability_registry,
                policy,
                requester,
                &request,
            ),
            _ => Err(prerequisite_unavailable_denial()),
        };

        Ok(ApprovalPostureResolver::resolve(
            RouteResolutionInput {
                request,
                requester_trust_outcome,
                policy_outcome,
                capability_admission_outcome,
            },
            context,
        ))
    }
}

fn prerequisite_unavailable_denial() -> DenialGuard {
    deny(
        DenialReasonClass::CapabilityNotAdmitted,
        DenialScope::Capability,
        DenialBasis::Contract,
        "capability admission requires successful requester trust and policy evaluation",
    )
}
