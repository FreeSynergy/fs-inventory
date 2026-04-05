//! gRPC service implementation for `fs-inventory`.
//!
//! Wraps a shared [`crate::repo::Inventory`] via the [`crate::store::InventoryStore`] trait.

use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::{
    models::{InstalledResource, ReleaseChannel, ResourceStatus, ServiceInstance, ServiceStatus},
    store::InventoryStore,
};
use fs_types::{ResourceType, Role, ValidationStatus};

// Include the generated tonic code.
pub mod proto {
    #![allow(clippy::all, clippy::pedantic, warnings)]
    tonic::include_proto!("inventory");
}

pub use proto::inventory_service_server::{InventoryService, InventoryServiceServer};
pub use proto::{
    GetResourceRequest, GetResourceResponse, GetStatusRequest, GetStatusResponse,
    ListResourcesRequest, ListResourcesResponse, ListServicesRequest, ListServicesResponse,
    ResourceRecord, ServiceRecord, UninstallRequest, UninstallResponse, UpsertResourceRequest,
    UpsertResourceResponse, UpsertServiceRequest, UpsertServiceResponse,
};

// ── GrpcInventory ─────────────────────────────────────────────────────────────

/// gRPC service that wraps a shared inventory store.
pub struct GrpcInventory {
    store: Arc<dyn InventoryStore>,
}

impl GrpcInventory {
    /// Create a new gRPC service backed by the given store.
    #[must_use]
    pub fn new(store: Arc<dyn InventoryStore>) -> Self {
        Self { store }
    }
}

// ── Conversions ───────────────────────────────────────────────────────────────

fn resource_to_proto(r: &InstalledResource) -> ResourceRecord {
    ResourceRecord {
        id: r.id.clone(),
        resource_type: serde_json::to_string(&r.resource_type).unwrap_or_default(),
        version: r.version.clone(),
        channel: serde_json::to_string(&r.channel).unwrap_or_default(),
        installed_at: r.installed_at.clone(),
        status: serde_json::to_string(&r.status).unwrap_or_default(),
        config_path: r.config_path.clone(),
        data_path: r.data_path.clone(),
        validation: serde_json::to_string(&r.validation).unwrap_or_default(),
        caption: r.caption.clone().unwrap_or_default(),
    }
}

fn proto_to_resource(r: ResourceRecord) -> InstalledResource {
    InstalledResource {
        id: r.id,
        resource_type: serde_json::from_str(&r.resource_type).unwrap_or(ResourceType::App),
        version: r.version,
        channel: serde_json::from_str(&r.channel).unwrap_or(ReleaseChannel::Stable),
        installed_at: r.installed_at,
        status: serde_json::from_str(&r.status).unwrap_or(ResourceStatus::Active),
        config_path: r.config_path,
        data_path: r.data_path,
        validation: serde_json::from_str(&r.validation).unwrap_or(ValidationStatus::Incomplete),
        caption: if r.caption.is_empty() {
            None
        } else {
            Some(r.caption)
        },
    }
}

fn service_to_proto(s: &ServiceInstance) -> ServiceRecord {
    ServiceRecord {
        id: s.id.clone(),
        resource_id: s.resource_id.clone(),
        instance_name: s.instance_name.clone(),
        roles_provided: s
            .roles_provided
            .iter()
            .map(|r| r.as_str().to_owned())
            .collect(),
        roles_required: s
            .roles_required
            .iter()
            .map(|r| r.as_str().to_owned())
            .collect(),
        variables_json: serde_json::to_string(&s.variables).unwrap_or_default(),
        network: s.network.clone(),
        status: serde_json::to_string(&s.status).unwrap_or_default(),
        port: u32::from(s.port.unwrap_or(0)),
        s3_paths: s.s3_paths.clone(),
    }
}

fn proto_to_service(s: ServiceRecord) -> ServiceInstance {
    ServiceInstance {
        id: s.id,
        resource_id: s.resource_id,
        instance_name: s.instance_name,
        roles_provided: s
            .roles_provided
            .into_iter()
            .map(|r| Role::new(&r))
            .collect(),
        roles_required: s
            .roles_required
            .into_iter()
            .map(|r| Role::new(&r))
            .collect(),
        variables: serde_json::from_str(&s.variables_json).unwrap_or_default(),
        network: s.network,
        status: serde_json::from_str(&s.status).unwrap_or(ServiceStatus::Stopped),
        port: if s.port == 0 {
            None
        } else {
            u16::try_from(s.port).ok()
        },
        s3_paths: s.s3_paths,
    }
}

fn map_err(e: crate::error::InventoryError) -> Status {
    use crate::error::InventoryError;
    match e {
        InventoryError::NotFound { id } => Status::not_found(id),
        InventoryError::AlreadyInstalled { id } => Status::already_exists(id),
        other => Status::internal(other.to_string()),
    }
}

// ── Service impl ──────────────────────────────────────────────────────────────

#[tonic::async_trait]
impl InventoryService for GrpcInventory {
    async fn upsert_resource(
        &self,
        request: Request<UpsertResourceRequest>,
    ) -> Result<Response<UpsertResourceResponse>, Status> {
        let resource = proto_to_resource(
            request
                .into_inner()
                .resource
                .ok_or_else(|| Status::invalid_argument("resource is required"))?,
        );
        self.store
            .upsert_resource(&resource)
            .await
            .map_err(map_err)?;
        Ok(Response::new(UpsertResourceResponse {
            ok: true,
            message: String::new(),
        }))
    }

    async fn uninstall(
        &self,
        request: Request<UninstallRequest>,
    ) -> Result<Response<UninstallResponse>, Status> {
        self.store
            .uninstall(&request.into_inner().id)
            .await
            .map_err(map_err)?;
        Ok(Response::new(UninstallResponse {
            ok: true,
            message: String::new(),
        }))
    }

    async fn list_resources(
        &self,
        _request: Request<ListResourcesRequest>,
    ) -> Result<Response<ListResourcesResponse>, Status> {
        let resources = self.store.list_resources().await.map_err(map_err)?;
        Ok(Response::new(ListResourcesResponse {
            resources: resources.iter().map(resource_to_proto).collect(),
        }))
    }

    async fn get_resource(
        &self,
        request: Request<GetResourceRequest>,
    ) -> Result<Response<GetResourceResponse>, Status> {
        let maybe = self
            .store
            .get_resource(&request.into_inner().id)
            .await
            .map_err(map_err)?;
        Ok(Response::new(GetResourceResponse {
            found: maybe.is_some(),
            resource: maybe.as_ref().map(resource_to_proto),
        }))
    }

    async fn upsert_service(
        &self,
        request: Request<UpsertServiceRequest>,
    ) -> Result<Response<UpsertServiceResponse>, Status> {
        let svc = proto_to_service(
            request
                .into_inner()
                .service
                .ok_or_else(|| Status::invalid_argument("service is required"))?,
        );
        self.store.upsert_service(&svc).await.map_err(map_err)?;
        Ok(Response::new(UpsertServiceResponse {
            ok: true,
            message: String::new(),
        }))
    }

    async fn list_services(
        &self,
        request: Request<ListServicesRequest>,
    ) -> Result<Response<ListServicesResponse>, Status> {
        let role = request.into_inner().role_filter;
        let services = if role.is_empty() {
            self.store.list_services().await.map_err(map_err)?
        } else {
            self.store
                .services_with_role(&role)
                .await
                .map_err(map_err)?
        };
        Ok(Response::new(ListServicesResponse {
            services: services.iter().map(service_to_proto).collect(),
        }))
    }

    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        let resource_count = self.store.list_resources().await.map_err(map_err)?.len();
        let service_count = self.store.list_services().await.map_err(map_err)?.len();
        Ok(Response::new(GetStatusResponse {
            ok: true,
            resource_count: u32::try_from(resource_count).unwrap_or(u32::MAX),
            service_count: u32::try_from(service_count).unwrap_or(u32::MAX),
        }))
    }
}
