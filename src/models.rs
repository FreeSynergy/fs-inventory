//! Domain models for the inventory — what is installed and running.

use crate::{entity, error::InventoryError};
use fs_types::{ResourceType, Role, ValidationStatus};
use serde::{Deserialize, Serialize};
use std::fmt;

// ── ReleaseChannel ────────────────────────────────────────────────────────────

/// Which store release channel a resource was installed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    #[default]
    Stable,
    Testing,
    Nightly,
}

impl fmt::Display for ReleaseChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stable => write!(f, "Stable"),
            Self::Testing => write!(f, "Testing"),
            Self::Nightly => write!(f, "Nightly"),
        }
    }
}

// ── ResourceStatus ────────────────────────────────────────────────────────────

/// Runtime state of an installed resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum ResourceStatus {
    Active,
    Stopped,
    Error(String),
    Updating,
    Installing,
    /// Installed but the first-time setup wizard has not completed yet.
    SetupRequired,
}

impl ResourceStatus {
    #[must_use]
    pub fn needs_attention(&self) -> bool {
        matches!(self, Self::Error(_) | Self::SetupRequired)
    }
}

impl fmt::Display for ResourceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Stopped => write!(f, "Stopped"),
            Self::Error(msg) => write!(f, "Error: {msg}"),
            Self::Updating => write!(f, "Updating"),
            Self::Installing => write!(f, "Installing"),
            Self::SetupRequired => write!(f, "Setup required"),
        }
    }
}

// ── InstalledResource ─────────────────────────────────────────────────────────

/// A resource that has been downloaded and installed on this node.
///
/// One row per installed resource — all resource types share this table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledResource {
    /// Resource slug, e.g. `"kanidm"`. Primary key.
    pub id: String,
    pub resource_type: ResourceType,
    /// Installed version string, e.g. `"1.5.0"`.
    pub version: String,
    pub channel: ReleaseChannel,
    /// ISO-8601 installation timestamp.
    pub installed_at: String,
    pub status: ResourceStatus,
    /// Path to the resource's configuration file.
    pub config_path: String,
    /// Path to the resource's data directory.
    pub data_path: String,
    pub validation: ValidationStatus,
    /// Optional user-assigned display name for this resource.
    ///
    /// Used when multiple instances of the same program are running
    /// (e.g. `"wiki.team-a"`, `"wiki.team-b"`).  Falls back to `id` when absent.
    pub caption: Option<String>,
}

impl TryFrom<entity::installed_resource::Model> for InstalledResource {
    type Error = InventoryError;

    fn try_from(m: entity::installed_resource::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: m.id,
            resource_type: serde_json::from_str(&m.resource_type)?,
            version: m.version,
            channel: serde_json::from_str(&m.channel)?,
            installed_at: m.installed_at,
            status: serde_json::from_str(&m.status)?,
            config_path: m.config_path,
            data_path: m.data_path,
            validation: serde_json::from_str(&m.validation)?,
            caption: m.caption,
        })
    }
}

// ── ProgramGroup ──────────────────────────────────────────────────────────────

/// A logical group of installed resources that represent multiple instances
/// of the same program (e.g. two Kanidm instances or three wiki instances).
///
/// The desktop shell renders the group as a single parent icon with sub-icons
/// for each instance.  The `group_icon_key` is a namespaced icon reference
/// in the format `"namespace:path"` (e.g. `"fs:apps/wiki"`) — the same value
/// that `fs_render::IconRef::key` would carry.
///
/// # Design
///
/// `ProgramGroup` is a pure domain struct.  It deliberately uses `String`
/// for `group_icon_key` so that this crate does not depend on `fs-render`.
/// The rendering layer converts the key to an `IconRef` when building the menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramGroup {
    /// The resource id of the primary / representative instance.
    pub parent_id: String,
    /// All instances that belong to this group.
    pub instances: Vec<InstalledResource>,
    /// Namespaced icon key (equivalent to `fs_render::IconRef::key`).
    pub group_icon_key: String,
}

// ── ServiceStatus ─────────────────────────────────────────────────────────────

/// Runtime state of a service instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum ServiceStatus {
    Running,
    Stopped,
    Starting,
    Error(String),
}

impl ServiceStatus {
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Stopped => write!(f, "Stopped"),
            Self::Starting => write!(f, "Starting"),
            Self::Error(msg) => write!(f, "Error: {msg}"),
        }
    }
}

// ── ConfiguredVar ─────────────────────────────────────────────────────────────

/// A configuration variable with its resolved value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredVar {
    pub name: String,
    /// Resolved value — secrets are stored encrypted separately.
    pub value: Option<String>,
}

// ── ServiceInstance ───────────────────────────────────────────────────────────

/// A running (or stopped) container service instance.
///
/// Multiple instances of the same resource can exist (e.g. two Kanidm instances).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstance {
    /// Unique instance identifier (UUID).
    pub id: String,
    /// The installed resource this instance is derived from.
    pub resource_id: String,
    /// User-assigned name, e.g. `"main-iam"`.
    pub instance_name: String,
    /// Roles this instance provides, e.g. `["iam"]`.
    pub roles_provided: Vec<Role>,
    /// Roles this instance requires from other services.
    pub roles_required: Vec<Role>,
    /// Configured environment variables.
    pub variables: Vec<ConfiguredVar>,
    /// Docker network name.
    pub network: String,
    pub status: ServiceStatus,
    /// Host port exposed (if any).
    pub port: Option<u16>,
    /// S3 paths used by this instance for data storage.
    pub s3_paths: Vec<String>,
}

impl TryFrom<entity::service_instance::Model> for ServiceInstance {
    type Error = InventoryError;

    fn try_from(m: entity::service_instance::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: m.id,
            resource_id: m.resource_id,
            instance_name: m.instance_name,
            roles_provided: serde_json::from_str(&m.roles_provided)?,
            roles_required: serde_json::from_str(&m.roles_required)?,
            variables: serde_json::from_str(&m.variables)?,
            network: m.network,
            status: serde_json::from_str(&m.status)?,
            port: m.port.map(|p| u16::try_from(p).unwrap_or(0)),
            s3_paths: serde_json::from_str(&m.s3_paths)?,
        })
    }
}
