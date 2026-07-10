use std::path::{Path, PathBuf};

use crate::adapters::execution_delivery::{
    AdapterDeliveryRequest, AdapterDeliveryResult, ExternalRouteDeliveryAdapter,
};
use crate::domain::shared::{ApprovalPosture, CapabilityId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmapScanProfile {
    LoopbackTcpConnectV1,
    AuthorizedPrivateSubnetTcpConnectV1,
}

impl NmapScanProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoopbackTcpConnectV1 => "loopback_tcp_connect_v1",
            Self::AuthorizedPrivateSubnetTcpConnectV1 => "authorized_private_subnet_tcp_connect_v1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NmapPreflightAdapterConfig {
    pub supported_capability_id: CapabilityId,
    pub nmap_binary: PathBuf,
    pub scan_profile: NmapScanProfile,
}

impl NmapPreflightAdapterConfig {
    pub fn new(
        supported_capability_id: CapabilityId,
        nmap_binary: PathBuf,
        scan_profile: NmapScanProfile,
    ) -> Self {
        Self {
            supported_capability_id,
            nmap_binary,
            scan_profile,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NmapPreflightDeliveryAdapter {
    config: NmapPreflightAdapterConfig,
}

impl NmapPreflightDeliveryAdapter {
    pub fn new(config: NmapPreflightAdapterConfig) -> Self {
        Self { config }
    }

    pub fn nmap_binary(&self) -> &Path {
        &self.config.nmap_binary
    }

    pub fn scan_profile(&self) -> NmapScanProfile {
        self.config.scan_profile
    }

    fn validate_request(&self, request: &AdapterDeliveryRequest) -> Result<(), &'static str> {
        if !matches!(
            request.resolved_approval_posture,
            ApprovalPosture::PolicyPreapproved | ApprovalPosture::ExecuteAllowed
        ) {
            return Err("nmap preflight adapter requires admitted posture");
        }

        if request.requested_capability_id != self.config.supported_capability_id {
            return Err("nmap preflight adapter capability mismatch");
        }

        if request.declared_capability_ids.len() != 1
            || request.declared_capability_ids[0] != self.config.supported_capability_id
        {
            return Err("nmap preflight adapter requires one declared capability");
        }

        if request.declared_step_ids.len() != 1 {
            return Err("nmap preflight adapter requires one declared step");
        }

        if !request.declared_fallback_references.is_empty() {
            return Err("nmap preflight adapter does not support fallbacks");
        }

        Ok(())
    }
}

impl ExternalRouteDeliveryAdapter for NmapPreflightDeliveryAdapter {
    fn adapter_id(&self) -> &'static str {
        "nmap-preflight-delivery"
    }

    fn deliver_route(&self, request: &AdapterDeliveryRequest) -> AdapterDeliveryResult {
        if let Err(summary) = self.validate_request(request) {
            return AdapterDeliveryResult::Unsupported {
                summary: summary.to_owned(),
            };
        }

        if !self.nmap_binary().is_file() {
            return AdapterDeliveryResult::DependencyUnavailable {
                summary: "nmap runtime is unavailable for declared scan profile".to_owned(),
            };
        }

        AdapterDeliveryResult::DeliveredAllSteps
    }
}
