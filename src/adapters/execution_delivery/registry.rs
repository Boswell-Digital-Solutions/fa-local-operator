use std::collections::BTreeMap;

use crate::adapters::execution_delivery::ExternalRouteDeliveryAdapter;
use crate::domain::shared::CapabilityId;
use crate::errors::{FaLocalError, FaLocalResult};

/// Runtime capability-to-adapter dispatch table. Each capability may resolve
/// to exactly one adapter; registering a second adapter for an
/// already-registered capability fails closed instead of silently shadowing
/// the first (this is a bounded worker: ambiguous dispatch is a
/// misconfiguration, not a pick-one situation).
#[derive(Default)]
pub struct AdapterRegistry {
    by_capability: BTreeMap<CapabilityId, Box<dyn ExternalRouteDeliveryAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        supported_capability_id: CapabilityId,
        adapter: Box<dyn ExternalRouteDeliveryAdapter>,
    ) -> FaLocalResult<()> {
        if self.by_capability.contains_key(&supported_capability_id) {
            return Err(FaLocalError::ContractInvalid(format!(
                "adapter already registered for capability {supported_capability_id}"
            )));
        }

        self.by_capability.insert(supported_capability_id, adapter);
        Ok(())
    }

    pub fn resolve(
        &self,
        capability_id: CapabilityId,
    ) -> Option<&dyn ExternalRouteDeliveryAdapter> {
        self.by_capability
            .get(&capability_id)
            .map(|adapter| adapter.as_ref())
    }

    pub fn registered_capability_ids(&self) -> impl Iterator<Item = CapabilityId> + '_ {
        self.by_capability.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::execution_delivery::{AdapterDeliveryRequest, AdapterDeliveryResult};

    struct StubAdapter(&'static str);

    impl ExternalRouteDeliveryAdapter for StubAdapter {
        fn adapter_id(&self) -> &'static str {
            self.0
        }

        fn deliver_route(&self, _request: &AdapterDeliveryRequest) -> AdapterDeliveryResult {
            AdapterDeliveryResult::DeliveredAllSteps
        }
    }

    #[test]
    fn resolves_registered_adapter_by_capability() {
        let capability_id = CapabilityId::new();
        let mut registry = AdapterRegistry::new();
        registry
            .register(capability_id, Box::new(StubAdapter("a")))
            .unwrap();

        let resolved = registry.resolve(capability_id).unwrap();
        assert_eq!(resolved.adapter_id(), "a");
    }

    #[test]
    fn distinct_capabilities_resolve_to_distinct_adapters() {
        let capability_a = CapabilityId::new();
        let capability_b = CapabilityId::new();
        let mut registry = AdapterRegistry::new();
        registry
            .register(capability_a, Box::new(StubAdapter("a")))
            .unwrap();
        registry
            .register(capability_b, Box::new(StubAdapter("b")))
            .unwrap();

        assert_eq!(registry.resolve(capability_a).unwrap().adapter_id(), "a");
        assert_eq!(registry.resolve(capability_b).unwrap().adapter_id(), "b");
    }

    #[test]
    fn unknown_capability_resolves_to_none() {
        let registry = AdapterRegistry::new();
        assert!(registry.resolve(CapabilityId::new()).is_none());
    }

    #[test]
    fn duplicate_capability_registration_fails_closed() {
        let capability_id = CapabilityId::new();
        let mut registry = AdapterRegistry::new();
        registry
            .register(capability_id, Box::new(StubAdapter("a")))
            .unwrap();

        let error = registry
            .register(capability_id, Box::new(StubAdapter("b")))
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("contract invalid: adapter already registered for capability {capability_id}")
        );
    }
}
