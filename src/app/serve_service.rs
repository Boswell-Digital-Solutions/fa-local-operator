//! Orchestration for FA Local's read-only capability-registry serving mode.
//!
//! Authorized by `BDS-FAL-DAEMON-v0.1`'s implementation scoping packet
//! (`docs/plans/active/BDS_FAL_DAEMON_v0.1/02_IMPLEMENTATION_SCOPING_PACKET.md`).
//! Holds the loaded `CapabilityRegistry` in memory and exposes a `lookup`
//! call `adapters::serve::http_server` uses to answer the one read-only
//! route, plus a `SIGHUP`-triggered `reload`. Composes existing domain
//! logic without absorbing policy authority, matching this crate's other
//! `app/` services -- it calls
//! `domain::capabilities::CapabilityRegistryLoader::load_contract_value`
//! (via `domain::shared::load_contract_from_path`) and
//! `domain::capabilities::CapabilityRegistry::capability_for`, both
//! existing and unmodified. It never calls `admit_execution_request`:
//! policy admission is unrelated to this read-only lookup.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::domain::capabilities::{CapabilityRecord, CapabilityRegistry};
use crate::domain::shared::{CapabilityId, SchemaName, load_contract_from_path};
use crate::errors::FaLocalResult;

/// Outcome of one capability lookup against the currently loaded registry.
#[derive(Debug, Clone)]
pub enum ServeLookupResult {
    /// The capability exists in the loaded registry.
    Found(CapabilityRecord),
    /// The registry loaded fine, but no capability with this id exists.
    NotFound,
    /// No good registry is currently loaded to answer from. Only reachable
    /// if a lookup somehow ran before any successful load -- `ServeService::load`
    /// fails the `serve` subcommand's startup before its listener ever
    /// starts, so in practice this state is never observed by a real
    /// request; it exists so `lookup` has a truthful answer instead of a
    /// panic if that invariant is ever violated.
    RegistryUnavailable,
}

/// Holds the loaded capability registry for the `serve` subcommand.
///
/// A failed [`reload`](ServeService::reload) leaves the previously good
/// registry in place -- it never drops already-good state over a bad
/// reload, and it never serves a partially loaded registry.
pub struct ServeService {
    registry_file: PathBuf,
    registry: RwLock<Option<CapabilityRegistry>>,
}

impl ServeService {
    /// Loads and schema-validates the registry file once, at startup.
    ///
    /// Fails loudly if the initial load does not succeed -- `serve` must
    /// not start its listener over an empty or invalid registry (see
    /// `src/bin/fa_local_run.rs`'s `serve` arm, which exits before calling
    /// `adapters::serve::run_server` if this returns `Err`).
    pub fn load(registry_file: impl AsRef<Path>) -> FaLocalResult<Self> {
        let registry_file = registry_file.as_ref().to_path_buf();
        let registry = load_registry_from_file(&registry_file)?;
        Ok(Self {
            registry_file,
            registry: RwLock::new(Some(registry)),
        })
    }

    /// Looks up one capability by id against the currently loaded registry.
    pub fn lookup(&self, capability_id: CapabilityId) -> ServeLookupResult {
        let guard = self
            .registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match guard.as_ref() {
            Some(registry) => match registry.capability_for(capability_id) {
                Some(record) => ServeLookupResult::Found(record.clone()),
                None => ServeLookupResult::NotFound,
            },
            None => ServeLookupResult::RegistryUnavailable,
        }
    }

    /// Re-reads and schema-validates the registry file (the `SIGHUP`/
    /// `--watch` path).
    ///
    /// A failed reload leaves the previously good registry in place and
    /// returns the error for the caller to log -- it never serves a
    /// partially loaded registry, and it never drops already-good state
    /// over a bad reload (a corrupted or missing file on refresh does not
    /// take a previously working `serve` process down).
    pub fn reload(&self) -> FaLocalResult<()> {
        let fresh = load_registry_from_file(&self.registry_file)?;
        let mut guard = self
            .registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(fresh);
        Ok(())
    }
}

fn load_registry_from_file(path: &Path) -> FaLocalResult<CapabilityRegistry> {
    load_contract_from_path(SchemaName::CapabilityRegistry, path)
}
