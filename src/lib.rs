//! `fs-inventory` — local inventory of installed `FreeSynergy` resources.
//!
//! The Inventory answers the question *"What is installed on this node?"*.
//! It is the **single source of truth** for:
//!
//! - Which resources are installed and at what version
//! - Which service instances are running and on which ports
//!
//! # Database
//!
//! Uses its own `SQLite` file: `fs-inventory.db`.
//! No other component may maintain a parallel list of installed resources.
//!
//! # Example
//!
//! ```no_run
//! use fs_inventory::{Inventory, InventoryStore, InventoryError};
//! use fs_db::DbConfig;
//!
//! # async fn example() -> Result<(), InventoryError> {
//! let inv = Inventory::open(DbConfig::sqlite("fs-inventory.db")).await?;
//! let services = inv.services_with_role("iam").await?;
//! # Ok(())
//! # }
//! ```

#![deny(clippy::all, clippy::pedantic, warnings)]
#![allow(clippy::module_name_repetitions)]

pub mod api;
pub mod bus_handler;
pub mod cli;
pub mod entity;
pub mod error;
pub mod models;
pub mod repo;
pub mod store;

pub use bus_handler::{InventoryBusHandler, PackageInstalledPayload, PackageRemovedPayload};
pub use error::InventoryError;
pub use fs_db::DbConfig;
pub use models::{
    InstalledResource, ReleaseChannel, ResourceStatus, ServiceInstance, ServiceStatus,
};
pub use repo::Inventory;
pub use store::InventoryStore;
