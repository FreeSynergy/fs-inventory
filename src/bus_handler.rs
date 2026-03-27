// bus_handler.rs — InventoryBusHandler: bridges fs-bus installer::* events to
// the Inventory database.
//
// Topic patterns handled:
//   installer.package.installed  → upsert_resource(resource)
//   installer.package.removed    → uninstall(id)
//
// The handler is intentionally lenient: unknown topics and malformed payloads
// are logged as warnings and not propagated as errors so a bad message cannot
// take down the whole bus.

use std::sync::Arc;

use async_trait::async_trait;
use fs_bus::{BusError, Event, TopicHandler};
use serde::{Deserialize, Serialize};
use tracing::{instrument, warn};

use crate::{
    models::{InstalledResource, ReleaseChannel, ResourceStatus},
    repo::Inventory,
};
use fs_types::{ResourceType, ValidationStatus};

// ── Payload types ─────────────────────────────────────────────────────────────

/// Payload of `installer.package.installed`.
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageInstalledPayload {
    /// Package id, e.g. `"kanidm"`.
    pub id: String,
    /// Version string, e.g. `"1.4.2"`.
    pub version: String,
    /// Resource type string, e.g. `"app"`, `"container"`, `"theme"`.
    #[serde(default = "default_resource_type")]
    pub resource_type: ResourceType,
    /// Optional config path.
    #[serde(default)]
    pub config_path: String,
    /// Optional data path.
    #[serde(default)]
    pub data_path: String,
}

fn default_resource_type() -> ResourceType {
    ResourceType::App
}

/// Payload of `installer.package.removed`.
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageRemovedPayload {
    pub id: String,
}

// ── InventoryBusHandler ───────────────────────────────────────────────────────

/// Subscribes to `installer.#` bus events and keeps `fs-inventory.db` in sync.
pub struct InventoryBusHandler {
    inventory: Arc<Inventory>,
}

impl InventoryBusHandler {
    /// Wrap an existing `Inventory` in a bus handler.
    #[must_use]
    pub fn new(inventory: Arc<Inventory>) -> Self {
        Self { inventory }
    }
}

#[async_trait]
impl TopicHandler for InventoryBusHandler {
    #[allow(clippy::unnecessary_literal_bound)]
    fn topic_pattern(&self) -> &str {
        "installer.#"
    }

    #[instrument(name = "inventory.bus_handler", skip(self, event), fields(topic = event.topic()))]
    async fn handle(&self, event: &Event) -> Result<(), BusError> {
        match event.topic() {
            "installer.package.installed" => {
                let payload: PackageInstalledPayload = match event.parse_payload() {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("installer.package.installed: bad payload: {e}");
                        return Ok(());
                    }
                };
                let resource = InstalledResource {
                    id: payload.id,
                    resource_type: payload.resource_type,
                    version: payload.version,
                    channel: ReleaseChannel::Stable,
                    installed_at: chrono::Utc::now().to_rfc3339(),
                    status: ResourceStatus::Active,
                    config_path: payload.config_path,
                    data_path: payload.data_path,
                    validation: ValidationStatus::Ok,
                };
                if let Err(e) = self.inventory.upsert_resource(&resource).await {
                    warn!("inventory upsert failed: {e}");
                }
            }
            "installer.package.removed" => {
                let payload: PackageRemovedPayload = match event.parse_payload() {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("installer.package.removed: bad payload: {e}");
                        return Ok(());
                    }
                };
                if let Err(e) = self.inventory.uninstall(&payload.id).await {
                    warn!("inventory uninstall failed: {e}");
                }
            }
            other => {
                warn!("InventoryBusHandler: unhandled topic '{other}'");
            }
        }
        Ok(())
    }
}
