//! `InventoryStore` — repository pattern trait for the inventory database.
//!
//! All code that reads or writes inventory data must go through this trait.
//! The concrete implementation is [`crate::repo::Inventory`].

use async_trait::async_trait;

use crate::{
    error::InventoryError,
    models::{InstalledResource, ResourceStatus, ServiceInstance, ServiceStatus},
};

// ── InventoryStore ────────────────────────────────────────────────────────────

/// Abstract repository for the local resource inventory.
///
/// Implement this trait to provide a custom backend (mock, remote proxy, …).
/// The default implementation is [`crate::repo::Inventory`] backed by `SQLite`.
///
/// # Design
///
/// - **Upsert semantics**: `upsert_resource` / `upsert_service` are always idempotent.
/// - **Typed errors**: every method returns [`InventoryError`] so callers can
///   match on `NotFound`, `AlreadyInstalled`, etc. without parsing strings.
#[async_trait]
pub trait InventoryStore: Send + Sync {
    // ── Resources ─────────────────────────────────────────────────────────────

    /// Install a resource or update its status if already present.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or serialisation failure.
    async fn upsert_resource(&self, resource: &InstalledResource) -> Result<(), InventoryError>;

    /// Remove an installed resource by id.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::NotFound`] if the resource is not installed.
    async fn uninstall(&self, id: &str) -> Result<(), InventoryError>;

    /// All installed resources.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or deserialisation failure.
    async fn list_resources(&self) -> Result<Vec<InstalledResource>, InventoryError>;

    /// Find an installed resource by id.
    ///
    /// Returns `Ok(None)` when not found — use this for existence checks.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or deserialisation failure.
    async fn get_resource(&self, id: &str) -> Result<Option<InstalledResource>, InventoryError>;

    /// Update the runtime status of an installed resource.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::NotFound`] if the resource is not installed.
    async fn set_resource_status(
        &self,
        id: &str,
        status: &ResourceStatus,
    ) -> Result<(), InventoryError>;

    // ── Services ──────────────────────────────────────────────────────────────

    /// Register a service instance or update an existing one with the same name.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or serialisation failure.
    async fn upsert_service(&self, svc: &ServiceInstance) -> Result<(), InventoryError>;

    /// All registered service instances.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or deserialisation failure.
    async fn list_services(&self) -> Result<Vec<ServiceInstance>, InventoryError>;

    /// Service instances that provide a specific role (e.g. `"iam"`).
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or deserialisation failure.
    async fn services_with_role(&self, role: &str) -> Result<Vec<ServiceInstance>, InventoryError>;

    /// Update the runtime status of a service instance identified by its name.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::NotFound`] if the instance is not registered.
    async fn set_service_status_by_name(
        &self,
        instance_name: &str,
        status: &ServiceStatus,
    ) -> Result<(), InventoryError>;
}
