//! Integration tests for fs-inventory.
//!
//! Uses an in-memory SQLite database so no files are left on disk.

use fs_inventory::{
    InstalledResource, Inventory, ReleaseChannel, ResourceStatus, ServiceInstance, ServiceStatus,
};
use fs_types::{ResourceType, ValidationStatus};

fn test_resource() -> InstalledResource {
    InstalledResource {
        id: "kanidm".to_string(),
        resource_type: ResourceType::Container,
        version: "1.5.0".to_string(),
        channel: ReleaseChannel::Stable,
        installed_at: "2026-03-25T12:00:00Z".to_string(),
        status: ResourceStatus::Active,
        config_path: "/etc/kanidm/config.toml".to_string(),
        data_path: "/var/lib/kanidm".to_string(),
        validation: ValidationStatus::Ok,
    }
}

fn test_service(resource_id: &str) -> ServiceInstance {
    ServiceInstance {
        id: "kanidm-main".to_string(),
        resource_id: resource_id.to_string(),
        instance_name: "main-iam".to_string(),
        roles_provided: vec![fs_types::Role::new("iam")],
        roles_required: vec![],
        variables: vec![],
        network: "fs-net".to_string(),
        status: ServiceStatus::Running,
        port: Some(8443),
        s3_paths: vec![],
    }
}

async fn open_memory_db() -> Inventory {
    Inventory::open(":memory:")
        .await
        .expect("failed to open in-memory inventory")
}

#[tokio::test]
async fn install_and_query_resource() {
    let inv = open_memory_db().await;
    let resource = test_resource();

    inv.install(&resource).await.expect("install failed");

    let found = inv.resource("kanidm").await.expect("query failed");
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, "kanidm");
    assert_eq!(found.version, "1.5.0");
    assert_eq!(found.channel, ReleaseChannel::Stable);
}

#[tokio::test]
async fn install_duplicate_returns_error() {
    let inv = open_memory_db().await;
    let resource = test_resource();

    inv.install(&resource).await.expect("first install failed");
    let result = inv.install(&resource).await;

    assert!(matches!(
        result,
        Err(fs_inventory::InventoryError::AlreadyInstalled { .. })
    ));
}

#[tokio::test]
async fn upsert_resource_is_idempotent() {
    let inv = open_memory_db().await;
    let resource = test_resource();

    inv.upsert_resource(&resource)
        .await
        .expect("first upsert failed");
    inv.upsert_resource(&resource)
        .await
        .expect("second upsert failed");

    let all = inv.all_resources().await.expect("all_resources failed");
    assert_eq!(all.len(), 1);
}

#[tokio::test]
async fn update_resource_status() {
    let inv = open_memory_db().await;
    inv.install(&test_resource()).await.unwrap();

    inv.set_resource_status("kanidm", &ResourceStatus::Stopped)
        .await
        .expect("status update failed");

    let found = inv.resource("kanidm").await.unwrap().unwrap();
    assert_eq!(found.status, ResourceStatus::Stopped);
}

#[tokio::test]
async fn uninstall_resource() {
    let inv = open_memory_db().await;
    inv.install(&test_resource()).await.unwrap();
    inv.uninstall("kanidm").await.expect("uninstall failed");

    let found = inv.resource("kanidm").await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn service_instance_lifecycle() {
    let inv = open_memory_db().await;
    inv.install(&test_resource()).await.unwrap();
    inv.add_service(&test_service("kanidm"))
        .await
        .expect("add_service failed");

    let by_role = inv
        .services_with_role("iam")
        .await
        .expect("services_with_role failed");
    assert_eq!(by_role.len(), 1);
    assert_eq!(by_role[0].port, Some(8443));

    inv.set_service_status_by_name("main-iam", &ServiceStatus::Stopped)
        .await
        .expect("status update failed");

    let all = inv.all_services().await.unwrap();
    assert_eq!(all[0].status, ServiceStatus::Stopped);
}
