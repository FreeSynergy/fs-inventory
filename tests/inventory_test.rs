//! Integration tests for fs-inventory.
//!
//! Uses an in-memory `SQLite` database so no files are left on disk.

use std::sync::Arc;

use fs_bus::topics::{
    INVENTORY_PACKAGE_INSTALLED, INVENTORY_PACKAGE_REMOVED, INVENTORY_PACKAGE_UPDATED,
};
use fs_bus::{Event, TopicHandler};
use fs_inventory::{
    DbConfig, InstalledResource, Inventory, InventoryBusHandler, PackageInstalledPayload,
    PackageRemovedPayload, ReleaseChannel, ResourceStatus, ServiceInstance, ServiceStatus,
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
    Inventory::open(DbConfig::sqlite(":memory:"))
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

// ── Bus handler tests ─────────────────────────────────────────────────────────

async fn inventory_in_memory() -> Arc<Inventory> {
    let inv = Inventory::open(DbConfig::sqlite(":memory:"))
        .await
        .expect("open failed");
    Arc::new(inv)
}

#[tokio::test]
async fn bus_handler_installs_package_on_event() {
    let inv = inventory_in_memory().await;
    let handler = InventoryBusHandler::new(Arc::clone(&inv));

    let payload = PackageInstalledPayload {
        id: "stalwart".into(),
        version: "0.9.1".into(),
        resource_type: fs_types::ResourceType::Container,
        config_path: String::new(),
        data_path: String::new(),
    };
    let event = Event::new(INVENTORY_PACKAGE_INSTALLED, "test", payload).unwrap();
    handler.handle(&event).await.unwrap();

    let resource = inv.resource("stalwart").await.unwrap();
    assert!(resource.is_some());
    assert_eq!(resource.unwrap().version, "0.9.1");
}

#[tokio::test]
async fn bus_handler_updates_package_on_updated_event() {
    let inv = inventory_in_memory().await;
    let handler = InventoryBusHandler::new(Arc::clone(&inv));

    // Install first
    let p1 = PackageInstalledPayload {
        id: "stalwart".into(),
        version: "0.9.0".into(),
        resource_type: fs_types::ResourceType::Container,
        config_path: String::new(),
        data_path: String::new(),
    };
    handler
        .handle(&Event::new(INVENTORY_PACKAGE_INSTALLED, "test", p1).unwrap())
        .await
        .unwrap();

    // Update via bus
    let p2 = PackageInstalledPayload {
        id: "stalwart".into(),
        version: "0.9.1".into(),
        resource_type: fs_types::ResourceType::Container,
        config_path: String::new(),
        data_path: String::new(),
    };
    handler
        .handle(&Event::new(INVENTORY_PACKAGE_UPDATED, "test", p2).unwrap())
        .await
        .unwrap();

    let resource = inv.resource("stalwart").await.unwrap().unwrap();
    assert_eq!(resource.version, "0.9.1");
}

#[tokio::test]
async fn bus_handler_removes_package_on_removed_event() {
    let inv = inventory_in_memory().await;
    let handler = InventoryBusHandler::new(Arc::clone(&inv));

    // Install
    let p = PackageInstalledPayload {
        id: "stalwart".into(),
        version: "0.9.0".into(),
        resource_type: fs_types::ResourceType::Container,
        config_path: String::new(),
        data_path: String::new(),
    };
    handler
        .handle(&Event::new(INVENTORY_PACKAGE_INSTALLED, "test", p).unwrap())
        .await
        .unwrap();

    // Remove
    let removal = PackageRemovedPayload {
        id: "stalwart".into(),
    };
    handler
        .handle(&Event::new(INVENTORY_PACKAGE_REMOVED, "test", removal).unwrap())
        .await
        .unwrap();

    let resource = inv.resource("stalwart").await.unwrap();
    assert!(resource.is_none(), "resource should be removed");
}

#[tokio::test]
async fn bus_handler_topic_pattern_is_inventory_namespace() {
    let inv = inventory_in_memory().await;
    let handler = InventoryBusHandler::new(inv);
    assert_eq!(handler.topic_pattern(), "inventory::*");
}
