//! CLI commands for `fs-inventory`.
//!
//! Usage:
//!   fs-inventory list
//!   fs-inventory status <id>
//!   fs-inventory uninstall <id>
//!   fs-inventory services [--role <role>]
//!   fs-inventory serve [--grpc-port <port>] [--rest-port <port>]

use clap::{Parser, Subcommand};

// ── Cli ───────────────────────────────────────────────────────────────────────

/// `FreeSynergy` local resource inventory.
#[derive(Debug, Parser)]
#[command(name = "fs-inventory", about = "Manage the local resource inventory")]
pub struct Cli {
    /// Path to the inventory database file.
    #[arg(
        long,
        env = "FS_INVENTORY_DB",
        default_value = "/var/lib/freesynergy/inventory.db"
    )]
    pub db: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List all installed resources.
    List,

    /// Show status of an installed resource.
    Status {
        /// Resource id (e.g. "kanidm").
        id: String,
    },

    /// Uninstall a resource by id.
    Uninstall {
        /// Resource id to remove.
        id: String,
    },

    /// List service instances.
    Services {
        /// Filter by role (e.g. "iam").
        #[arg(long)]
        role: Option<String>,
    },

    /// Start the inventory daemon (gRPC + REST).
    Serve {
        /// gRPC listen port.
        #[arg(long, env = "FS_GRPC_PORT", default_value_t = 50_052)]
        grpc_port: u16,

        /// REST listen port.
        #[arg(long, env = "FS_REST_PORT", default_value_t = 8082)]
        rest_port: u16,
    },
}
