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
//! use fs_inventory::{Inventory, InventoryError};
//!
//! # async fn example() -> Result<(), InventoryError> {
//! let inv = Inventory::open("fs-inventory.db").await?;
//! let services = inv.services_with_role("iam").await?;
//! # Ok(())
//! # }
//! ```

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod entity;
pub mod error;
pub mod models;
pub mod repo;

pub use error::InventoryError;
pub use models::{
    InstalledResource, ReleaseChannel, ResourceStatus, ServiceInstance, ServiceStatus,
};
pub use repo::Inventory;
