use std::collections::HashMap;

use crate::adapters::execution_delivery::registry::AdapterRegistry;
use crate::adapters::execution_delivery::{
    AdapterDeliveryRequest, AdapterDeliveryResult, ExternalRouteDeliveryAdapter,
};
use crate::app::routing_service::{RoutePathKind, SelectedExecutionRoute};
use crate::domain::execution::{CancellationPolicy, ExecutionPlanStep, ValidatedExecutionPlan};
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

    /// Dispatches a validated plan one declared step at a time, resolving a
    /// (possibly different) adapter per step from `registry` by that step's
    /// own `capability_id` — beyond the single whole-route call in
    /// [`Self::deliver_selected_route_via_registry`], which only ever
    /// resolves one adapter for the route's top-level requested capability.
    /// Each step's [`AdapterDeliveryRequest`] is scoped to that one step
    /// (`declared_step_ids` of length one, no fallback references), so the
    /// existing concrete adapters need no changes to be dispatched this way.
    /// An adapter itself reporting a fallback completion for a per-step call
    /// is still an unsupported condition here (it was never told about any
    /// declared fallback), not a silent mismatch.
    ///
    /// Declared fallback references (`validated_plan.plan.fallback_references`)
    /// *are* coordinated here, but as a coordinator-level retry across
    /// (possibly different) adapters, not as an in-band adapter signal:
    /// when a step's own attempt is `Failed`, `Unavailable`, or `Canceled`
    /// and the plan declares a fallback for it, the coordinator immediately
    /// dispatches the declared fallback step out of its normal order,
    /// resolving *its own* adapter from `registry` by the fallback step's
    /// own capability. Plan validation already guarantees the fallback step
    /// is declared later in the plan, targets a different step, and that
    /// `fallback_references` is only non-empty under a fallback-aware
    /// completion policy, so this never dispatches a step out of causal
    /// order or double-declares a step as its own fallback. If the fallback
    /// delivery completes, the *primary* step's outcome becomes
    /// [`StepDeliveryOutcome::CompletedViaFallback`] (`degraded_subtype`
    /// [`DegradedSubtype::DegradedFallbackLimited`] — never
    /// `DegradedFallbackEquivalent`, since the coordinator has no way to
    /// know two different capabilities are truly equivalent, only that a
    /// plan author declared one a fallback for the other); the fallback
    /// step's own later turn in the loop then reuses that same real result
    /// instead of dispatching it a second time. If the fallback delivery
    /// does not complete either, the primary step keeps its own original
    /// outcome, and the fallback step's own later turn reuses *that*
    /// attempt's result. A fallback step already consumed by an earlier
    /// step's failed primary attempt is never dispatched a second time as
    /// a fallback target for some other step (`fallback_results` below
    /// tracks consumption), so a plan where two steps declare the same
    /// fallback never double-delivers it.
    pub fn deliver_plan_per_step_via_registry(
        &self,
        route: &SelectedExecutionRoute,
        validated_plan: &ValidatedExecutionPlan,
        registry: &AdapterRegistry,
        context: CoordinationContext,
    ) -> FaLocalResult<ExecutionTrace> {
        validate_selected_route_for_delivery(route)?;

        if Some(validated_plan.plan.execution_plan_id) != route.execution_plan_id
            || Some(validated_plan.stable_plan_hash.clone()) != route.stable_plan_hash
        {
            return Err(contract_invalid(
                "per-step delivery plan does not match the selected route's plan",
            ));
        }

        let mut statuses = vec![build_admitted_not_started_status_from_route(
            route,
            context.coordinated_at_utc,
        )?];

        let mut outcomes: Vec<(String, StepDeliveryOutcome)> = Vec::new();
        let mut stop_dispatching = false;
        let mut fallback_results: HashMap<String, StepDeliveryOutcome> = HashMap::new();

        for step in &validated_plan.plan.steps {
            if let Some(outcome) = fallback_results.remove(&step.step_id) {
                // Already dispatched out of order as another step's
                // declared fallback target -- reuse that real result rather
                // than delivering this step a second time.
                outcomes.push((step.step_id.clone(), outcome));
                continue;
            }

            if stop_dispatching {
                outcomes.push((step.step_id.clone(), StepDeliveryOutcome::Skipped));
                continue;
            }

            statuses.extend(build_in_progress_statuses_from_route(
                route,
                std::slice::from_ref(&step.step_id),
                context.started_at_utc,
            )?);

            let mut outcome = dispatch_one_step(route, validated_plan, registry, step)?;

            if matches!(
                outcome,
                StepDeliveryOutcome::Failed { .. }
                    | StepDeliveryOutcome::Unavailable { .. }
                    | StepDeliveryOutcome::Canceled
            ) && let Some(fallback_step) =
                declared_fallback_step(validated_plan, &step.step_id, &fallback_results)
            {
                statuses.extend(build_in_progress_statuses_from_route(
                    route,
                    std::slice::from_ref(&fallback_step.step_id),
                    context.started_at_utc,
                )?);

                let fallback_outcome =
                    dispatch_one_step(route, validated_plan, registry, fallback_step)?;
                if matches!(fallback_outcome, StepDeliveryOutcome::Completed) {
                    outcome = StepDeliveryOutcome::CompletedViaFallback {
                        fallback_step_id: fallback_step.step_id.clone(),
                    };
                }
                fallback_results.insert(fallback_step.step_id.clone(), fallback_outcome);
            }

            if matches!(
                outcome,
                StepDeliveryOutcome::Failed { .. }
                    | StepDeliveryOutcome::Unavailable { .. }
                    | StepDeliveryOutcome::Canceled
            ) && validated_plan.plan.cancellation_policy
                == CancellationPolicy::CancelRemainingSteps
            {
                stop_dispatching = true;
            }

            outcomes.push((step.step_id.clone(), outcome));
        }

        statuses.push(build_plan_outcome_status_from_route(
            route,
            &outcomes,
            context.started_at_utc,
            context.completed_at_utc,
        )?);

        ExecutionTrace::new(statuses)
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

#[derive(Debug, Clone)]
enum StepDeliveryOutcome {
    Completed,
    /// The step's own attempt did not complete, but its declared fallback
    /// step did, dispatched by the coordinator to the fallback step's own
    /// registry-resolved adapter (see
    /// [`ExecutionService::deliver_plan_per_step_via_registry`]).
    CompletedViaFallback {
        fallback_step_id: String,
    },
    Failed {
        summary: String,
    },
    /// The adapter itself reported a cancellation for this step.
    Canceled,
    Unavailable {
        summary: String,
    },
    /// Never dispatched: an earlier step already stopped the run under
    /// `cancel_remaining_steps`. Distinct from `Canceled` (which means an
    /// adapter *attempted and canceled* a step) so this never gets counted
    /// as its own root cause when deciding the plan's final status below.
    Skipped,
}

/// Aggregates every declared step's own delivery outcome into one final,
/// truthful status for the whole plan: all-completed (whether directly or
/// via a declared fallback) stays `Completed`, unless at least one step
/// needed its fallback, in which case the whole plan reports
/// `CompletedWithConstraints`/`DegradedFallbackLimited` rather than
/// silently claiming an unconstrained success; a mix of completed and
/// not-completed steps becomes `PartialSuccess` (`degraded_partial`) rather
/// than silently reporting either extreme; zero completed steps falls back
/// to whichever single-outcome status already exists for that failure mode
/// (`Failed`, `Canceled`, or `Degraded`/`UnavailableDependencyBlock` when
/// every attempted step had no adapter at all) — judged only from steps
/// that were actually attempted, since a step skipped after an earlier
/// failure is a consequence, not a cause.
fn build_plan_outcome_status_from_route(
    route: &SelectedExecutionRoute,
    outcomes: &[(String, StepDeliveryOutcome)],
    started_at_utc: TimestampUtc,
    completed_at_utc: TimestampUtc,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let completed_count = outcomes
        .iter()
        .filter(|(_, outcome)| {
            matches!(
                outcome,
                StepDeliveryOutcome::Completed | StepDeliveryOutcome::CompletedViaFallback { .. }
            )
        })
        .count();

    if completed_count == outcomes.len() {
        let any_step_used_a_fallback = outcomes.iter().any(|(_, outcome)| {
            matches!(outcome, StepDeliveryOutcome::CompletedViaFallback { .. })
        });

        return if any_step_used_a_fallback {
            build_plan_completed_via_fallback_status_from_route(
                route,
                outcomes,
                started_at_utc,
                completed_at_utc,
            )
        } else {
            build_completed_status_from_route(route, started_at_utc, completed_at_utc)
        };
    }

    if completed_count > 0 {
        let (failed_step_id, failure_summary) = outcomes
            .iter()
            .find_map(|(step_id, outcome)| match outcome {
                StepDeliveryOutcome::Failed { summary } => Some((step_id.clone(), summary.clone())),
                StepDeliveryOutcome::Unavailable { summary } => {
                    Some((step_id.clone(), summary.clone()))
                }
                StepDeliveryOutcome::Canceled => {
                    Some((step_id.clone(), "step was canceled".to_owned()))
                }
                StepDeliveryOutcome::Skipped => Some((
                    step_id.clone(),
                    "step was skipped after an earlier step did not complete".to_owned(),
                )),
                StepDeliveryOutcome::Completed
                | StepDeliveryOutcome::CompletedViaFallback { .. } => None,
            })
            .expect("mixed outcome always has at least one non-completed step");
        return build_partial_success_status_from_route(
            route,
            started_at_utc,
            completed_at_utc,
            completed_count,
            outcomes.len(),
            failed_step_id,
            failure_summary,
        );
    }

    let attempted = outcomes
        .iter()
        .filter(|(_, outcome)| !matches!(outcome, StepDeliveryOutcome::Skipped));

    if let Some((_, summary)) = attempted
        .clone()
        .find_map(|(step_id, outcome)| match outcome {
            StepDeliveryOutcome::Failed { summary } => Some((step_id, summary)),
            _ => None,
        })
    {
        return build_failed_status_from_route(
            route,
            started_at_utc,
            completed_at_utc,
            summary.clone(),
        );
    }

    if let Some((step_id, _)) = attempted
        .clone()
        .find(|(_, outcome)| matches!(outcome, StepDeliveryOutcome::Canceled))
    {
        return build_canceled_status_from_route(route, started_at_utc, completed_at_utc, step_id);
    }

    build_unavailable_dependency_status_from_route(
        route,
        completed_at_utc,
        "no delivery adapter registered for any declared step".to_owned(),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_partial_success_status_from_route(
    route: &SelectedExecutionRoute,
    started_at_utc: TimestampUtc,
    completed_at_utc: TimestampUtc,
    completed_count: usize,
    total_count: usize,
    failed_step_id: String,
    failure_detail: String,
) -> FaLocalResult<ValidatedExecutionStatus> {
    let execution_plan_id = route.execution_plan_id.ok_or_else(|| {
        contract_invalid("external delivery route must include execution_plan_id")
    })?;
    let stable_plan_hash = route
        .stable_plan_hash
        .clone()
        .ok_or_else(|| contract_invalid("external delivery route must include stable_plan_hash"))?;
    let completion_summary =
        format!("{completed_count} of {total_count} declared plan steps completed");
    let failure_summary = format!("step {failed_step_id} did not complete: {failure_detail}");

    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route.request_id,
        route.correlation_id,
        Some(execution_plan_id),
        Some(stable_plan_hash),
        route.resolved_approval_posture,
        ExecutionState::PartialSuccess,
        Some(DegradedSubtype::DegradedPartial),
        Some(started_at_utc),
        completed_at_utc,
        Some(completed_at_utc),
        None,
        Some(completion_summary),
        Some(failure_summary),
        format!("{completed_count} of {total_count} declared plan steps completed"),
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

/// Like [`build_completed_with_constraints_status_from_route`], but for the
/// per-step delivery path: every declared step completed, but at least one
/// only via its own declared fallback, so the message names exactly which
/// primary step fell back to which fallback step rather than reporting a
/// generic "used a fallback" claim.
fn build_plan_completed_via_fallback_status_from_route(
    route: &SelectedExecutionRoute,
    outcomes: &[(String, StepDeliveryOutcome)],
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

    let fallback_pairs = outcomes
        .iter()
        .filter_map(|(step_id, outcome)| match outcome {
            StepDeliveryOutcome::CompletedViaFallback { fallback_step_id } => {
                Some(format!("{step_id} -> {fallback_step_id}"))
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(", ");
    let completion_summary =
        format!("all declared plan steps completed; used declared fallback for: {fallback_pairs}");

    ValidatedExecutionStatus::new(ExecutionStatus::new(
        route.request_id,
        route.correlation_id,
        Some(execution_plan_id),
        Some(stable_plan_hash),
        route.resolved_approval_posture,
        ExecutionState::CompletedWithConstraints,
        Some(DegradedSubtype::DegradedFallbackLimited),
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

/// Delivers exactly one declared plan step to its own registry-resolved
/// adapter, scoped the same way [`ExecutionService::deliver_plan_per_step_via_registry`]
/// always scopes a per-step request (`declared_step_ids` of length one, no
/// fallback references) -- shared by both a step's own ordinary turn and a
/// coordinator-triggered fallback attempt for a different step, so both go
/// through the exact same adapter contract.
fn dispatch_one_step(
    route: &SelectedExecutionRoute,
    validated_plan: &ValidatedExecutionPlan,
    registry: &AdapterRegistry,
    step: &ExecutionPlanStep,
) -> FaLocalResult<StepDeliveryOutcome> {
    let Some(adapter) = registry.resolve(step.capability_id) else {
        return Ok(StepDeliveryOutcome::Unavailable {
            summary: format!(
                "no delivery adapter registered for capability {}",
                step.capability_id
            ),
        });
    };

    let request = AdapterDeliveryRequest {
        route_decision_id: route.route_decision_id,
        correlation_id: route.correlation_id,
        request_id: route.request_id,
        resolved_approval_posture: route.resolved_approval_posture,
        requested_capability_id: step.capability_id,
        execution_plan_id: validated_plan.plan.execution_plan_id,
        stable_plan_hash: validated_plan.stable_plan_hash.clone(),
        declared_step_ids: vec![step.step_id.clone()],
        declared_capability_ids: vec![step.capability_id],
        declared_fallback_references: Vec::new(),
    };

    match adapter.deliver_route(&request) {
        AdapterDeliveryResult::DeliveredAllSteps => Ok(StepDeliveryOutcome::Completed),
        AdapterDeliveryResult::FailedAtDeclaredStep {
            failure_summary, ..
        } => {
            validate_required_summary(&failure_summary, "adapter delivery failure_summary")?;
            Ok(StepDeliveryOutcome::Failed {
                summary: failure_summary,
            })
        }
        AdapterDeliveryResult::CanceledAtDeclaredStep { .. } => Ok(StepDeliveryOutcome::Canceled),
        AdapterDeliveryResult::DependencyUnavailable { summary } => {
            validate_required_summary(&summary, "adapter delivery dependency summary")?;
            Ok(StepDeliveryOutcome::Unavailable { summary })
        }
        AdapterDeliveryResult::CompletedWithDeclaredFallback { .. } => Err(contract_invalid(
            "declared fallback completion is not supported in per-step delivery",
        )),
        AdapterDeliveryResult::Unsupported { summary } => Err(contract_invalid(format!(
            "unsupported adapter condition from {}: {summary}",
            adapter.adapter_id()
        ))),
    }
}

/// Looks up the plan's own declared fallback step for `step_id`, if any --
/// `None` both when no fallback is declared for this step and when the
/// declared fallback step was already dispatched (successfully or not) as
/// some other step's fallback target, so a plan where two steps declare the
/// same fallback never delivers it twice.
fn declared_fallback_step<'a>(
    validated_plan: &'a ValidatedExecutionPlan,
    step_id: &str,
    already_consumed: &HashMap<String, StepDeliveryOutcome>,
) -> Option<&'a ExecutionPlanStep> {
    let fallback_reference = validated_plan
        .plan
        .fallback_references
        .iter()
        .find(|reference| reference.step_id == step_id)?;

    if already_consumed.contains_key(&fallback_reference.fallback_step_id) {
        return None;
    }

    validated_plan
        .plan
        .steps
        .iter()
        .find(|step| step.step_id == fallback_reference.fallback_step_id)
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
