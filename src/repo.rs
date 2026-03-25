//! `Inventory` — the primary interface to `fs-inventory.db`.

use crate::{
    entity::{installed_resource, service_instance},
    error::InventoryError,
    models::{InstalledResource, ResourceStatus, ServiceInstance, ServiceStatus},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, Database, DatabaseConnection,
    EntityTrait, QueryFilter,
};
use tracing::instrument;

// ── Schema ────────────────────────────────────────────────────────────────────

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS installed_resources (
    id            TEXT PRIMARY KEY NOT NULL,
    resource_type TEXT NOT NULL,
    version       TEXT NOT NULL,
    channel       TEXT NOT NULL DEFAULT 'stable',
    installed_at  TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT '{\"state\":\"active\"}',
    config_path   TEXT NOT NULL DEFAULT '',
    data_path     TEXT NOT NULL DEFAULT '',
    validation    TEXT NOT NULL DEFAULT 'incomplete'
);

CREATE TABLE IF NOT EXISTS service_instances (
    id             TEXT PRIMARY KEY NOT NULL,
    resource_id    TEXT NOT NULL REFERENCES installed_resources(id),
    instance_name  TEXT NOT NULL,
    roles_provided TEXT NOT NULL DEFAULT '[]',
    roles_required TEXT NOT NULL DEFAULT '[]',
    variables      TEXT NOT NULL DEFAULT '[]',
    network        TEXT NOT NULL DEFAULT '',
    status         TEXT NOT NULL DEFAULT '{\"state\":\"stopped\"}',
    port           INTEGER,
    s3_paths       TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS idx_si_resource_id ON service_instances(resource_id);
";

// ── Inventory ─────────────────────────────────────────────────────────────────

/// The local inventory — the single source of truth for what is installed on this node.
pub struct Inventory {
    db: DatabaseConnection,
}

impl Inventory {
    /// Open (or create) the inventory database at the given path.
    ///
    /// Use `"sqlite::memory:"` in tests.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] if the database connection fails or the schema cannot be applied.
    #[instrument(name = "inventory.open")]
    pub async fn open(path: &str) -> Result<Self, InventoryError> {
        let url = if path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite://{path}?mode=rwc")
        };
        let db = Database::connect(&url).await?;
        db.execute_unprepared(SCHEMA).await?;
        Ok(Self { db })
    }

    // ── InstalledResource ─────────────────────────────────────────────────────

    /// Install a new resource. Returns `AlreadyInstalled` if the id exists.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::AlreadyInstalled`] if the id already exists, or a database error.
    #[instrument(name = "inventory.install", skip(self, resource))]
    pub async fn install(&self, resource: &InstalledResource) -> Result<(), InventoryError> {
        if installed_resource::Entity::find_by_id(&resource.id)
            .one(&self.db)
            .await?
            .is_some()
        {
            return Err(InventoryError::AlreadyInstalled {
                id: resource.id.clone(),
            });
        }
        installed_resource::ActiveModel {
            id: Set(resource.id.clone()),
            resource_type: Set(serde_json::to_string(&resource.resource_type)?),
            version: Set(resource.version.clone()),
            channel: Set(serde_json::to_string(&resource.channel)?),
            installed_at: Set(resource.installed_at.clone()),
            status: Set(serde_json::to_string(&resource.status)?),
            config_path: Set(resource.config_path.clone()),
            data_path: Set(resource.data_path.clone()),
            validation: Set(serde_json::to_string(&resource.validation)?),
        }
        .insert(&self.db)
        .await?;
        Ok(())
    }

    /// Remove an installed resource by id.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::NotFound`] if the id does not exist, or a database error.
    #[instrument(name = "inventory.uninstall", skip(self))]
    pub async fn uninstall(&self, id: &str) -> Result<(), InventoryError> {
        let model = installed_resource::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| InventoryError::NotFound { id: id.to_owned() })?;
        let active: installed_resource::ActiveModel = model.into();
        active.delete(&self.db).await?;
        Ok(())
    }

    /// All installed resources.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or deserialization failure.
    pub async fn all_resources(&self) -> Result<Vec<InstalledResource>, InventoryError> {
        installed_resource::Entity::find()
            .all(&self.db)
            .await?
            .into_iter()
            .map(InstalledResource::try_from)
            .collect()
    }

    /// Find an installed resource by id.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or deserialization failure.
    pub async fn resource(&self, id: &str) -> Result<Option<InstalledResource>, InventoryError> {
        installed_resource::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .map(InstalledResource::try_from)
            .transpose()
    }

    /// Insert a resource, or update its status if already installed.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or serialization failure.
    #[instrument(name = "inventory.upsert_resource", skip(self, resource))]
    pub async fn upsert_resource(
        &self,
        resource: &InstalledResource,
    ) -> Result<(), InventoryError> {
        match self.install(resource).await {
            Ok(()) => Ok(()),
            Err(InventoryError::AlreadyInstalled { .. }) => {
                self.set_resource_status(&resource.id, &resource.status)
                    .await
            }
            Err(e) => Err(e),
        }
    }

    // ── ServiceInstance ───────────────────────────────────────────────────────

    /// Register a service instance.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or serialization failure.
    #[instrument(name = "inventory.add_service", skip(self, svc))]
    pub async fn add_service(&self, svc: &ServiceInstance) -> Result<(), InventoryError> {
        service_instance::ActiveModel {
            id: Set(svc.id.clone()),
            resource_id: Set(svc.resource_id.clone()),
            instance_name: Set(svc.instance_name.clone()),
            roles_provided: Set(serde_json::to_string(&svc.roles_provided)?),
            roles_required: Set(serde_json::to_string(&svc.roles_required)?),
            variables: Set(serde_json::to_string(&svc.variables)?),
            network: Set(svc.network.clone()),
            status: Set(serde_json::to_string(&svc.status)?),
            port: Set(svc.port.map(i32::from)),
            s3_paths: Set(serde_json::to_string(&svc.s3_paths)?),
        }
        .insert(&self.db)
        .await?;
        Ok(())
    }

    /// All service instances providing a specific role.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or deserialization failure.
    #[instrument(name = "inventory.services_with_role", skip(self))]
    pub async fn services_with_role(
        &self,
        role: &str,
    ) -> Result<Vec<ServiceInstance>, InventoryError> {
        let pattern = format!("%\"{role}\"%");
        service_instance::Entity::find()
            .filter(service_instance::Column::RolesProvided.like(pattern))
            .all(&self.db)
            .await?
            .into_iter()
            .map(ServiceInstance::try_from)
            .collect()
    }

    /// All service instances.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or deserialization failure.
    pub async fn all_services(&self) -> Result<Vec<ServiceInstance>, InventoryError> {
        service_instance::Entity::find()
            .all(&self.db)
            .await?
            .into_iter()
            .map(ServiceInstance::try_from)
            .collect()
    }

    /// Register a service instance or update an existing one with the same name.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError`] on database or serialization failure.
    #[instrument(name = "inventory.upsert_service", skip(self, svc))]
    pub async fn upsert_service(&self, svc: &ServiceInstance) -> Result<(), InventoryError> {
        let existing = service_instance::Entity::find()
            .filter(service_instance::Column::InstanceName.eq(&svc.instance_name))
            .one(&self.db)
            .await?;

        if let Some(model) = existing {
            let mut active: service_instance::ActiveModel = model.into();
            active.status = Set(serde_json::to_string(&svc.status)?);
            active.roles_provided = Set(serde_json::to_string(&svc.roles_provided)?);
            active.roles_required = Set(serde_json::to_string(&svc.roles_required)?);
            active.network = Set(svc.network.clone());
            active.port = Set(svc.port.map(i32::from));
            active.update(&self.db).await?;
        } else {
            self.add_service(svc).await?;
        }
        Ok(())
    }

    // ── Status updates ────────────────────────────────────────────────────────

    /// Update the runtime status of an installed resource.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::NotFound`] if the id does not exist, or a database error.
    #[instrument(name = "inventory.set_resource_status", skip(self, status))]
    pub async fn set_resource_status(
        &self,
        id: &str,
        status: &ResourceStatus,
    ) -> Result<(), InventoryError> {
        let model = installed_resource::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| InventoryError::NotFound { id: id.to_owned() })?;
        let mut active: installed_resource::ActiveModel = model.into();
        active.status = Set(serde_json::to_string(status)?);
        active.update(&self.db).await?;
        Ok(())
    }

    /// Update the runtime status of a service instance by name.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::NotFound`] if the instance is not found, or a database error.
    #[instrument(name = "inventory.set_service_status_by_name", skip(self, status))]
    pub async fn set_service_status_by_name(
        &self,
        instance_name: &str,
        status: &ServiceStatus,
    ) -> Result<(), InventoryError> {
        let model = service_instance::Entity::find()
            .filter(service_instance::Column::InstanceName.eq(instance_name))
            .one(&self.db)
            .await?
            .ok_or_else(|| InventoryError::NotFound {
                id: instance_name.to_owned(),
            })?;
        let mut active: service_instance::ActiveModel = model.into();
        active.status = Set(serde_json::to_string(status)?);
        active.update(&self.db).await?;
        Ok(())
    }
}
