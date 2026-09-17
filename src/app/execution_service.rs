use crate::adapters::execution_delivery::registry::AdapterRegistry;
use crate::adapters::execution_delivery::{
    AdapterDeliveryRequest, AdapterDeliveryResult, ExternalRouteDeliveryAdapter,
};
use crate::app::routing_service::{RoutePathKind, SelectedExecutionRoute};
use crate::domain::execution::ValidatedExecutionPlan;
use crate::domain::routing::RouteDecision;
use crate::domain::shared::{
    ApprovalPosture, DegradedSubtype, ExecutionState, TimestampUtc, now_utc,
};
use crate::domain::status::{ExecutionStatus, ValidatedExecutionStatus};
use crate::errors::{FaLocalError, FaLocalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoordinationContext {
    pub coordinated_at_utc: TimestampUtc,
    pub started_at_utc: TimestampUtc,
    pub completed_at_utc: TimestampUtc,
}

impl CoordinationContext {
    pub fn new(
        coordinated_at_utc: TimestampUtc,
        started_at_utc: TimestampUtc,
        completed_at_utc: TimestampUtc,
    ) -> Self {
        Self {
            coordinated_at_utc,
            started_at_utc,
            completed_at_utc,
        }
    }
}

impl Default for CoordinationContext {
    fn default() -> Self {
        let now = now_utc();
        Self::new(now, now, now)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinationDirective {
    NoExecution,
    CompleteDeclaredPlan,
    FailAtDeclaredStep {
        step_id: String,
        failure_summary: String,
    },
    CancelInFlight {
        step_id: String,
    },
    UnsupportedRuntimeCondition {
        summary: String,
    },
}

impl CoordinationDirective {
    fn requires_admitted_execution(&self) -> bool {
        !matches!(self, Self::NoExecution)
    }
}

#[derive(Debug, Clone)]
pub struct CoordinationInput {
    pub route_decision: RouteDecision,
    pub validated_plan: Option<ValidatedExecutionPlan>,
    pub directive: CoordinationDirective,
    pub context: CoordinationContext,
}

impl CoordinationInput {
    pub fn new(
        route_decision: RouteDecision,
        validated_plan: Option<ValidatedExecutionPlan>,
        directive: CoordinationDirective,
        context: CoordinationContext,
    ) -> FaLocalResult<Self> {
        validate_route_decision_surface(&route_decision)?;

        match route_decision.resolved_approval_posture {
            ApprovalPosture::Denied | ApprovalPosture::ReviewRequired => {
                if validated_plan.is_some() {
                    return Err(contract_invalid(
                        "pre-execution denied or review routes must not enter coordinator with execution plan",
                    ));
                }

                if directive.requires_admitted_execution() {
                    return Err(contract_invalid(
                        "route posture does not admit execution coordination",
                    ));
                }
            }
            ApprovalPosture::ExplicitOperatorApproval => {
                let plan = validated_plan.as_ref().ok_or_else(|| {
                    contract_invalid(
                        "explicit approval coordination requires validated execution plan",
                    )
                })?;
                validate_plan_matches_route(&route_decision, plan)?;

                if directive.requires_admitted_execution() {
                    return Err(contract_invalid(
                        "explicit approval route cannot enter execution progression without approval",
                    ));
                }
            }
            ApprovalPosture::PolicyPreapproved | ApprovalPosture::ExecuteAllowed => {
                let plan = validated_plan.as_ref().ok_or_else(|| {
                    contract_invalid(
                        "admitted execution coordination requires validated execution plan",
                    )
                })?;
                validate_plan_matches_route(&route_decision, plan)?;
            }
        }

        Ok(Self {
            route_decision,
            validated_plan,
            directive,
            context,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionTrace {
    pub statuses: Vec<ValidatedExecutionStatus>,
}

impl ExecutionTrace {
    pub fn new(statuses: Vec<ValidatedExecutionStatus>) -> FaLocalResult<Self> {
        if statuses.is_empty() {
            return Err(contract_invalid(
                "execution coordinator must emit at least one execution status",
            ));
        }

        Ok(Self { statuses })
    }

    pub fn final_status(&self) -> &ValidatedExecutionStatus {
        self.statuses
            .last()
            .expect("execution trace always has at least one status")
    }
}

#[derive(Debug, Default)]
pub struct ExecutionService;

impl ExecutionService {
    pub fn coordinate(&self, input: CoordinationInput) -> FaLocalResult<ExecutionTrace> {
        let CoordinationInput {
            route_decision,
            validated_plan,
            directive,
            context,
        } = input;

        match route_decision.resolved_approval_posture {
            ApprovalPosture::Denied => ExecutionTrace::new(vec![build_denied_status(
                &route_decision,
                context.coordinated_at_utc,
            )?]),
            ApprovalPosture::ReviewRequired => {
                ExecutionTrace::new(vec![build_review_required_status(
                    &route_decision,
                    context.coordinated_at_utc,
                )?])
            }
            ApprovalPosture::ExplicitOperatorApproval => {
                ExecutionTrace::new(vec![build_waiting_approval_status(
                    &route_decision,
                    context.coordinated_at_utc,
                )?])
            }
            ApprovalPosture::PolicyPreapproved | ApprovalPosture::ExecuteAllowed => {
                let validated_plan = validated_plan.as_ref().expect(
                    "validated plan required for admitted coordination after input validation",
                );
                self.coordinate_admitted_route(&route_decision, validated_plan, directive, context)
            }
        }
    }

    fn coordinate_admitted_route(
        &self,
        route_decision: &RouteDecision,
        validated_plan: &ValidatedExecutionPlan,
        directive: CoordinationDirective,
        context: CoordinationContext,
    ) -> FaLocalResult<ExecutionTrace> {
        let mut statuses = vec![build_admitted_not_started_status(
            route_decision,
            validated_plan,
            context.coordinated_at_utc,
        )?];

        match directive {
            CoordinationDirective::NoExecution => ExecutionTrace::new(statuses),
            CoordinationDirective::CompleteDeclaredPlan => {
                statuses.extend(build_in_progress_statuses(
                    route_decision,
                    validated_plan,
                    &validated_plan
                        .plan
                        .steps
                        .iter()
                        .map(|step| step.step_id.clone())
                        .collect::<Vec<_>>(),
                    context.started_at_utc,
                )?);
                statuses.push(build_completed_status(
                    route_decision,
                    validated_plan,
                    context.started_at_utc,
                    context.completed_at_utc,
                )?);
                ExecutionTrace::new(statuses)
            }
            CoordinationDirective::FailAtDeclaredStep {
                step_id,
                failure_summary,
            } => {
                validate_required_summary(
                    &failure_summary,
                    "execution coordinator failure_summary",
                )?;
                let step_ids = declared_steps_through(
                    validated_plan,
                    &step_id,
                    "execution coordinator step is not declared in execution plan",
                )?;
                statuses.extend(build_in_progress_statuses(
                    route_decision,
                    validated_plan,
                    &step_ids,
                    context.started_at_utc,
                )?);
                statuses.push(build_failed_status(
                    route_decision,
                    validated_plan,
                    context.started_at_utc,
                    context.completed_at_utc,
                    failure_summary,
                )?);
                ExecutionTrace::new(statuses)
            }
            CoordinationDirective::CancelInFlight { step_id } => {
                let step_ids = declared_steps_through(
                    validated_plan,
                    &step_id,
                    "execution coordinator step is not declared in execution plan",
                )?;
                statuses.extend(build_in_progress_statuses(
                    route_decision,
                    validated_plan,
                    &step_ids,
                    context.started_at_utc,
                )?);
                statuses.push(build_canceled_status(
                    route_decision,
                    validated_plan,
                    context.started_at_utc,
                    context.completed_at_utc,
                    &step_id,
                )?);
                ExecutionTrace::new(statuses)
            }
            CoordinationDirective::UnsupportedRuntimeCondition { summary } => {
                validate_required_summary(
                    &summary,
                    "execution coordinator unsupported runtime summary",
                )?;
                Err(contract_invalid(format!(
                    "unsupported runtime condition: {summary}"
                )))
            }
        }
    }

    pub fn deliver_selected_route<A: ExternalRouteDeliveryAdapter>(
        &self,
        route: &SelectedExecutionRoute,
        adapter: &A,
        context: CoordinationContext,
    ) -> FaLocalResult<ExecutionTrace> {
        self.deliver_via_adapter(route, adapter, context)
    }

    /// Resolves the adapter for `route.requested_capability_id` at runtime
    /// from `registry` instead of requiring the caller to already know which
    /// concrete adapter to invoke. A capability with no registered adapter is
    /// a truthful degraded outcome (no delivery mechanism is currently
    /// available for admitted work), not an error: it is reported the same
    /// way a single adapter's own `DependencyUnavailable` result is.
    pub fn deliver_selected_route_via_registry(
        &self,
        route: &SelectedExecutionRoute,
        registry: &AdapterRegistry,
        context: CoordinationContext,
    ) -> FaLocalResult<ExecutionTrace> {
        validate_selected_route_for_delivery(route)?;

        match registry.resolve(route.requested_capability_id) {
            Some(adapter) => self.deliver_via_adapter(route, adapter, context),
            None => {
                let mut statuses = vec![build_admitted_not_started_status_from_route(
                    route,
                    context.coordinated_at_utc,
                )?];
                statuses.push(build_unavailable_dependency_status_from_route(
                    route,
                    context.completed_at_utc,
                    format!(
                        "no delivery adapter registered for capability {}",
                        route.requested_capability_id
                    ),
                )?);
                ExecutionTrace::new(statuses)
            }
        }
    }

    fn deliver_via_adapter(
        &self,
        route: &SelectedExecutionRoute,
        adapter: &dyn ExternalRouteDeliveryAdapter,
        context: CoordinationContext,
    ) -> FaLocalResult<ExecutionTrace> {
        validate_selected_route_for_delivery(route)?;

        let request = adapter_request_for(route)?;
        let mut statuses = vec![build_admitted_not_started_status_from_route(
            route,
            context.coordinated_at_utc,
        )?];

        match adapter.deliver_route(&request) {
            AdapterDeliveryResult::DeliveredAllSteps => {
                statuses.extend(build_in_progress_statuses_from_route(
                    route,
                    &route.declared_step_ids,
                    context.started_at_utc,
                )?);
                statuses.push(build_completed_status_from_route(
                    route,
                    context.started_at_utc,
                    context.completed_at_utc,
                )?);
                ExecutionTrace::new(statuses)
            }
            AdapterDeliveryResult::CompletedWithDeclaredFallback {
                step_id,
                fallback_step_id,
                degraded_subtype,
            } => {
                validate_fallback_result(route, &step_id, &fallback_step_id, degraded_subtype)?;
                let step_ids = declared_steps_through_route(
                    route,
                    &fallback_step_id,
                    "adapter reported fallback step that is not declared in execution route",
                )?;
                statuses.extend(build_in_progress_statuses_from_route(
                    route,
                    &step_ids,
                    context.started_at_utc,
                )?);
                statuses.push(build_completed_with_constraints_status_from_route(
                    route,
                    context.started_at_utc,
                    context.completed_at_utc,
                    degraded_subtype,
                )?);
                ExecutionTrace::new(statuses)
            }
            AdapterDeliveryResult::FailedAtDeclaredStep {
                step_id,
                failure_summary,
            } => {
                validate_required_summary(&failure_summary, "adapter delivery failure_summary")?;
                let step_ids = declared_steps_through_route(
                    route,
                    &step_id,
                    "adapter reported step that is not declared in execution route",
                )?;
                statuses.extend(build_in_progress_statuses_from_route(
                    route,
                    &step_ids,
                    context.started_at_utc,
                )?);
                statuses.push(build_failed_status_from_route(
                    route,
                    context.started_at_utc,
                    context.completed_at_utc,
                    failure_summary,
                )?);
                ExecutionTrace::new(statuses)
            }
            AdapterDeliveryResult::CanceledAtDeclaredStep { step_id } => {
                let step_ids = declared_steps_through_route(
                    route,
                    &step_id,
                    "adapter reported step that is not declared in execution route",
                )?;
                statuses.extend(build_in_progress_statuses_from_route(
                    route,
                    &step_ids,
                    context.started_at_utc,
                )?);
                statuses.push(build_canceled_status_from_route(
                    route,
                    context.started_at_utc,
                    context.completed_at_utc,
                    &step_id,
                )?);
                ExecutionTrace::new(statuses)
            }
            AdapterDeliveryResult::DependencyUnavailable { summary } => {
                validate_required_summary(&summary, "adapter delivery dependency summary")?;
                statuses.push(build_unavailable_dependency_status_from_route(
                    route,
                    context.completed_at_utc,
                    summary,
                )?);
                ExecutionTrace::new(statuses)
            }
            AdapterDeliveryResult::Unsupported { summary } => {
                validate_required_summary(&summary, "adapter delivery unsupported summary")?;
                Err(contract_invalid(format!(
                    "unsupported adapter condition from {}: {summary}",
                    adapter.adapter_id()
                )))
            }
        }
    }
}

fn build_denied_status(
    route_decision: &RouteDecision,
    updated_at_utc: TimestampUtc,
) -> FaLocalResult<ValidatedExecutionStatus> {
    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route_decision.request_id,
        route_decision.correlation_id,
        None,
        None,
        ApprovalPosture::Denied,
        ExecutionState::Denied,
        None,
        None,
        updated_at_utc,
        None,
        None,
        None,
        None,
        route_decision.operator_visible_summary.clone(),
    )?)
}

fn build_review_required_status(
    route_decision: &RouteDecision,
    updated_at_utc: TimestampUtc,
) -> FaLocalResult<ValidatedExecutionStatus> {
    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route_decision.request_id,
        route_decision.correlation_id,
        None,
        None,
        ApprovalPosture::ReviewRequired,
        ExecutionState::ReviewRequired,
        None,
        None,
        updated_at_utc,
        None,
        None,
        None,
        None,
        route_decision.operator_visible_summary.clone(),
    )?)
}

fn build_waiting_approval_status(
    route_decision: &RouteDecision,
    updated_at_utc: TimestampUtc,
) -> FaLocalResult<ValidatedExecutionStatus> {
    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route_decision.request_id,
        route_decision.correlation_id,
        None,
        None,
        ApprovalPosture::ExplicitOperatorApproval,
        ExecutionState::WaitingExplicitApproval,
        None,
        None,
        updated_at_utc,
        None,
        None,
        None,
        None,
        "waiting for explicit operator approval".to_owned(),
    )?)
}

fn build_admitted_not_started_status(
    route_decision: &RouteDecision,
    validated_plan: &ValidatedExecutionPlan,
    updated_at_utc: TimestampUtc,
) -> FaLocalResult<ValidatedExecutionStatus> {
    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route_decision.request_id,
        route_decision.correlation_id,
        Some(validated_plan.plan.execution_plan_id),
        Some(validated_plan.stable_plan_hash.clone()),
        route_decision.resolved_approval_posture,
        ExecutionState::AdmittedNotStarted,
        None,
        None,
        updated_at_utc,
        None,
        None,
        None,
        None,
        "execution admitted and bounded plan is ready to start".to_owned(),
    )?)
}

fn build_in_progress_statuses(
    route_decision: &RouteDecision,
    validated_plan: &ValidatedExecutionPlan,
    step_ids: &[String],
    started_at_utc: TimestampUtc,
) -> FaLocalResult<Vec<ValidatedExecutionStatus>> {
    let mut statuses = Vec::with_capacity(step_ids.len());
    for step_id in step_ids {
        statuses.push(ValidatedExecutionStatus::new(ExecutionStatus::new(
            route_decision.request_id,
            route_decision.correlation_id,
            Some(validated_plan.plan.execution_plan_id),
            Some(validated_plan.stable_plan_hash.clone()),
            route_decision.resolved_approval_posture,
            ExecutionState::InProgress,
            None,
            Some(started_at_utc),
            started_at_utc,
            None,
            Some(step_id.clone()),
            None,
            None,
            format!("executing declared step {step_id}"),
        )?)?);
    }
    Ok(statuses)
}

fn build_completed_status(
    route_decision: &RouteDecision,
    validated_plan: &ValidatedExecutionPlan,
    started_at_utc: TimestampUtc,
    completed_at_utc: TimestampUtc,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let completion_summary = "execution completed for all declared plan steps".to_owned();
    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route_decision.request_id,
        route_decision.correlation_id,
        Some(validated_plan.plan.execution_plan_id),
        Some(validated_plan.stable_plan_hash.clone()),
        route_decision.resolved_approval_posture,
        ExecutionState::Completed,
        None,
        Some(started_at_utc),
        completed_at_utc,
        Some(completed_at_utc),
        None,
        Some(completion_summary.clone()),
        None,
        completion_summary,
    )?)
}

fn build_admitted_not_started_status_from_route(
    route: &SelectedExecutionRoute,
    updated_at_utc: TimestampUtc,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let execution_plan_id = route.execution_plan_id.ok_or_else(|| {
        contract_invalid("external delivery route must include execution_plan_id")
    })?;
    let stable_plan_hash = route
        .stable_plan_hash
        .clone()
        .ok_or_else(|| contract_invalid("external delivery route must include stable_plan_hash"))?;

    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route.request_id,
        route.correlation_id,
        Some(execution_plan_id),
        Some(stable_plan_hash),
        route.resolved_approval_posture,
        ExecutionState::AdmittedNotStarted,
        None,
        None,
        updated_at_utc,
        None,
        None,
        None,
        None,
        "execution admitted for external bounded delivery".to_owned(),
    )?)
}

fn build_in_progress_statuses_from_route(
    route: &SelectedExecutionRoute,
    step_ids: &[String],
    started_at_utc: TimestampUtc,
) -> FaLocalResult<Vec<ValidatedExecutionStatus>> {
    let execution_plan_id = route.execution_plan_id.ok_or_else(|| {
        contract_invalid("external delivery route must include execution_plan_id")
    })?;
    let stable_plan_hash = route
        .stable_plan_hash
        .clone()
        .ok_or_else(|| contract_invalid("external delivery route must include stable_plan_hash"))?;

    let mut statuses = Vec::with_capacity(step_ids.len());
    for step_id in step_ids {
        statuses.push(ValidatedExecutionStatus::new(ExecutionStatus::new(
            route.request_id,
            route.correlation_id,
            Some(execution_plan_id),
            Some(stable_plan_hash.clone()),
            route.resolved_approval_posture,
            ExecutionState::InProgress,
            None,
            Some(started_at_utc),
            started_at_utc,
            None,
            Some(step_id.clone()),
            None,
            None,
            format!("executing externally delivered step {step_id}"),
        )?)?);
    }
    Ok(statuses)
}

fn build_completed_status_from_route(
    route: &SelectedExecutionRoute,
    started_at_utc: TimestampUtc,
    completed_at_utc: TimestampUtc,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let execution_plan_id = route.execution_plan_id.ok_or_else(|| {
        contract_invalid("external delivery route must include execution_plan_id")
    })?;
    let stable_plan_hash = route
        .stable_plan_hash
        .clone()
        .ok_or_else(|| contract_invalid("external delivery route must include stable_plan_hash"))?;
    let completion_summary = "external delivery completed for all declared plan steps".to_owned();

    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route.request_id,
        route.correlation_id,
        Some(execution_plan_id),
        Some(stable_plan_hash),
        route.resolved_approval_posture,
        ExecutionState::Completed,
        None,
        Some(started_at_utc),
        completed_at_utc,
        Some(completed_at_utc),
        None,
        Some(completion_summary.clone()),
        None,
        completion_summary,
    )?)
}

fn build_completed_with_constraints_status_from_route(
    route: &SelectedExecutionRoute,
    started_at_utc: TimestampUtc,
    completed_at_utc: TimestampUtc,
    degraded_subtype: DegradedSubtype,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let execution_plan_id = route.execution_plan_id.ok_or_else(|| {
        contract_invalid("external delivery route must include execution_plan_id")
    })?;
    let stable_plan_hash = route
        .stable_plan_hash
        .clone()
        .ok_or_else(|| contract_invalid("external delivery route must include stable_plan_hash"))?;
    let completion_summary = "external delivery completed with declared fallback path".to_owned();

    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route.request_id,
        route.correlation_id,
        Some(execution_plan_id),
        Some(stable_plan_hash),
        route.resolved_approval_posture,
        ExecutionState::CompletedWithConstraints,
        Some(degraded_subtype),
        Some(started_at_utc),
        completed_at_utc,
        Some(completed_at_utc),
        None,
        Some(completion_summary.clone()),
        None,
        completion_summary,
    )?)
}

fn build_failed_status_from_route(
    route: &SelectedExecutionRoute,
    started_at_utc: TimestampUtc,
    completed_at_utc: TimestampUtc,
    failure_summary: String,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let execution_plan_id = route.execution_plan_id.ok_or_else(|| {
        contract_invalid("external delivery route must include execution_plan_id")
    })?;
    let stable_plan_hash = route
        .stable_plan_hash
        .clone()
        .ok_or_else(|| contract_invalid("external delivery route must include stable_plan_hash"))?;
    let truthful_summary = failure_summary.clone();

    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route.request_id,
        route.correlation_id,
        Some(execution_plan_id),
        Some(stable_plan_hash),
        route.resolved_approval_posture,
        ExecutionState::Failed,
        None,
        Some(started_at_utc),
        completed_at_utc,
        Some(completed_at_utc),
        None,
        None,
        Some(failure_summary),
        truthful_summary,
    )?)
}

fn build_canceled_status_from_route(
    route: &SelectedExecutionRoute,
    started_at_utc: TimestampUtc,
    completed_at_utc: TimestampUtc,
    step_id: &str,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let execution_plan_id = route.execution_plan_id.ok_or_else(|| {
        contract_invalid("external delivery route must include execution_plan_id")
    })?;
    let stable_plan_hash = route
        .stable_plan_hash
        .clone()
        .ok_or_else(|| contract_invalid("external delivery route must include stable_plan_hash"))?;

    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route.request_id,
        route.correlation_id,
        Some(execution_plan_id),
        Some(stable_plan_hash),
        route.resolved_approval_posture,
        ExecutionState::Canceled,
        None,
        Some(started_at_utc),
        completed_at_utc,
        Some(completed_at_utc),
        None,
        None,
        None,
        format!("external delivery canceled during declared step {step_id}"),
    )?)
}

fn build_unavailable_dependency_status_from_route(
    route: &SelectedExecutionRoute,
    updated_at_utc: TimestampUtc,
    summary: String,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let execution_plan_id = route.execution_plan_id.ok_or_else(|| {
        contract_invalid("external delivery route must include execution_plan_id")
    })?;
    let stable_plan_hash = route
        .stable_plan_hash
        .clone()
        .ok_or_else(|| contract_invalid("external delivery route must include stable_plan_hash"))?;

    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route.request_id,
        route.correlation_id,
        Some(execution_plan_id),
        Some(stable_plan_hash),
        route.resolved_approval_posture,
        ExecutionState::Degraded,
        Some(DegradedSubtype::UnavailableDependencyBlock),
        None,
        updated_at_utc,
        None,
        None,
        None,
        None,
        summary,
    )?)
}

fn build_failed_status(
    route_decision: &RouteDecision,
    validated_plan: &ValidatedExecutionPlan,
    started_at_utc: TimestampUtc,
    completed_at_utc: TimestampUtc,
    failure_summary: String,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let truthful_summary = failure_summary.clone();
    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route_decision.request_id,
        route_decision.correlation_id,
        Some(validated_plan.plan.execution_plan_id),
        Some(validated_plan.stable_plan_hash.clone()),
        route_decision.resolved_approval_posture,
        ExecutionState::Failed,
        None,
        Some(started_at_utc),
        completed_at_utc,
        Some(completed_at_utc),
        None,
        None,
        Some(failure_summary),
        truthful_summary,
    )?)
}

fn build_canceled_status(
    route_decision: &RouteDecision,
    validated_plan: &ValidatedExecutionPlan,
    started_at_utc: TimestampUtc,
    completed_at_utc: TimestampUtc,
    step_id: &str,
) -> FaLocalResult<ValidatedExecutionStatus> {
    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route_decision.request_id,
        route_decision.correlation_id,
        Some(validated_plan.plan.execution_plan_id),
        Some(validated_plan.stable_plan_hash.clone()),
        route_decision.resolved_approval_posture,
        ExecutionState::Canceled,
        None,
        Some(started_at_utc),
        completed_at_utc,
        Some(completed_at_utc),
        None,
        None,
        None,
        format!("execution canceled during declared step {step_id}"),
    )?)
}

fn validate_route_decision_surface(route_decision: &RouteDecision) -> FaLocalResult<()> {
    match route_decision.resolved_approval_posture {
        ApprovalPosture::Denied => {
            if route_decision.execution_allowed
                || route_decision.review_required
                || route_decision.explicit_approval_required
            {
                return Err(contract_invalid(
                    "denied route decision is inconsistent with coordinator expectations",
                ));
            }
        }
        ApprovalPosture::ReviewRequired => {
            if route_decision.execution_allowed
                || !route_decision.review_required
                || route_decision.explicit_approval_required
            {
                return Err(contract_invalid(
                    "review_required route decision is inconsistent with coordinator expectations",
                ));
            }
        }
        ApprovalPosture::ExplicitOperatorApproval => {
            if route_decision.execution_allowed
                || route_decision.review_required
                || !route_decision.explicit_approval_required
            {
                return Err(contract_invalid(
                    "explicit_operator_approval route decision is inconsistent with coordinator expectations",
                ));
            }
        }
        ApprovalPosture::PolicyPreapproved | ApprovalPosture::ExecuteAllowed => {
            if !route_decision.execution_allowed
                || route_decision.review_required
                || route_decision.explicit_approval_required
            {
                return Err(contract_invalid(
                    "admitted route decision is inconsistent with coordinator expectations",
                ));
            }
        }
    }

    Ok(())
}

fn validate_plan_matches_route(
    route_decision: &RouteDecision,
    validated_plan: &ValidatedExecutionPlan,
) -> FaLocalResult<()> {
    if validated_plan.plan.correlation_id != route_decision.correlation_id {
        return Err(contract_invalid(
            "execution coordinator plan correlation_id does not match route decision",
        ));
    }

    if validated_plan.plan.originating_request_id != route_decision.request_id {
        return Err(contract_invalid(
            "execution coordinator plan request_id does not match route decision",
        ));
    }

    Ok(())
}

fn declared_steps_through(
    validated_plan: &ValidatedExecutionPlan,
    target_step_id: &str,
    error_message: &'static str,
) -> FaLocalResult<Vec<String>> {
    let Some(target_index) = validated_plan
        .plan
        .steps
        .iter()
        .position(|step| step.step_id == target_step_id)
    else {
        return Err(contract_invalid(error_message));
    };

    Ok(validated_plan
        .plan
        .steps
        .iter()
        .take(target_index + 1)
        .map(|step| step.step_id.clone())
        .collect())
}

fn validate_required_summary(summary: &str, field_name: &str) -> FaLocalResult<()> {
    if summary.is_empty() || summary.len() > 160 {
        return Err(contract_invalid(format!(
            "{field_name} must be between 1 and 160 characters",
        )));
    }
    Ok(())
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}

fn validate_selected_route_for_delivery(route: &SelectedExecutionRoute) -> FaLocalResult<()> {
    match route.route_path_kind {
        RoutePathKind::NonExecutableDenied | RoutePathKind::NonExecutableReviewRequired => Err(
            contract_invalid("non-executable route must not reach adapter delivery"),
        ),
        RoutePathKind::AwaitExplicitApproval => Err(contract_invalid(
            "explicit approval route must not reach adapter delivery",
        )),
        RoutePathKind::ExternalAdapterBoundedExecution => {
            if !route.executable || route.explicit_approval_required {
                return Err(contract_invalid(
                    "external adapter route is inconsistent with delivery expectations",
                ));
            }

            if !matches!(
                route.resolved_approval_posture,
                ApprovalPosture::PolicyPreapproved | ApprovalPosture::ExecuteAllowed
            ) {
                return Err(contract_invalid(
                    "external adapter delivery requires admitted posture",
                ));
            }

            if route.execution_plan_id.is_none() || route.stable_plan_hash.is_none() {
                return Err(contract_invalid(
                    "external adapter delivery route must include execution_plan_id and stable_plan_hash",
                ));
            }

            if route.declared_step_ids.is_empty() || route.declared_capability_ids.is_empty() {
                return Err(contract_invalid(
                    "external adapter delivery route must include declared steps and capabilities",
                ));
            }

            if !route
                .declared_capability_ids
                .contains(&route.requested_capability_id)
            {
                return Err(contract_invalid(
                    "external adapter delivery route must remain capability-scoped to declared capability set",
                ));
            }

            Ok(())
        }
    }
}

fn adapter_request_for(route: &SelectedExecutionRoute) -> FaLocalResult<AdapterDeliveryRequest> {
    validate_selected_route_for_delivery(route)?;

    Ok(AdapterDeliveryRequest {
        route_decision_id: route.route_decision_id,
        correlation_id: route.correlation_id,
        request_id: route.request_id,
        resolved_approval_posture: route.resolved_approval_posture,
        requested_capability_id: route.requested_capability_id,
        execution_plan_id: route.execution_plan_id.ok_or_else(|| {
            contract_invalid("external adapter delivery route must include execution_plan_id")
        })?,
        stable_plan_hash: route.stable_plan_hash.clone().ok_or_else(|| {
            contract_invalid("external adapter delivery route must include stable_plan_hash")
        })?,
        declared_step_ids: route.declared_step_ids.clone(),
        declared_capability_ids: route.declared_capability_ids.clone(),
        declared_fallback_references: route.declared_fallback_references.clone(),
    })
}

fn declared_steps_through_route(
    route: &SelectedExecutionRoute,
    target_step_id: &str,
    error_message: &'static str,
) -> FaLocalResult<Vec<String>> {
    let Some(target_index) = route
        .declared_step_ids
        .iter()
        .position(|step_id| step_id == target_step_id)
    else {
        return Err(contract_invalid(error_message));
    };

    Ok(route
        .declared_step_ids
        .iter()
        .take(target_index + 1)
        .cloned()
        .collect())
}

fn validate_fallback_result(
    route: &SelectedExecutionRoute,
    step_id: &str,
    fallback_step_id: &str,
    degraded_subtype: DegradedSubtype,
) -> FaLocalResult<()> {
    if !matches!(
        degraded_subtype,
        DegradedSubtype::DegradedFallbackEquivalent | DegradedSubtype::DegradedFallbackLimited
    ) {
        return Err(contract_invalid(
            "adapter fallback completion must use an explicit fallback degraded_subtype",
        ));
    }

    if !route
        .declared_step_ids
        .iter()
        .any(|candidate| candidate == step_id)
    {
        return Err(contract_invalid(
            "adapter reported primary step that is not declared in execution route",
        ));
    }

    if !route.declared_fallback_references.iter().any(|reference| {
        reference.step_id == step_id && reference.fallback_step_id == fallback_step_id
    }) {
        return Err(contract_invalid(
            "adapter reported fallback that is not declared in execution route",
        ));
    }

    Ok(())
}
