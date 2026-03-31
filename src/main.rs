//! `fs-inventory` — `FreeSynergy` local resource inventory daemon + CLI.

#![deny(clippy::all, clippy::pedantic, warnings)]
#![allow(clippy::module_name_repetitions)]

use std::sync::Arc;

use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};

use fs_bus::MessageBus;
use fs_inventory::{
    api::grpc::{GrpcInventory, InventoryServiceServer},
    api::rest,
    bus_handler::InventoryBusHandler,
    cli::{Cli, Command},
    repo::Inventory,
    store::InventoryStore,
    DbConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let cli = Cli::parse();

    match cli.command {
        Command::List => cmd_list(&cli.db).await,
        Command::Status { id } => cmd_status(&cli.db, &id).await,
        Command::Uninstall { id } => cmd_uninstall(&cli.db, &id).await,
        Command::Services { role } => cmd_services(&cli.db, role.as_deref()).await,
        Command::Serve {
            grpc_port,
            rest_port,
        } => cmd_serve(&cli.db, grpc_port, rest_port).await,
    }
}

// ── CLI commands ──────────────────────────────────────────────────────────────

async fn open_inventory(db: &str) -> Result<Inventory, Box<dyn std::error::Error>> {
    Ok(Inventory::open(DbConfig::sqlite(db)).await?)
}

async fn cmd_list(db: &str) -> Result<(), Box<dyn std::error::Error>> {
    let inv = open_inventory(db).await?;
    let resources = inv.list_resources().await?;
    if resources.is_empty() {
        println!("No resources installed.");
        return Ok(());
    }
    println!("{:<30} {:<12} {:<10} STATUS", "ID", "VERSION", "CHANNEL");
    println!("{}", "-".repeat(72));
    for r in &resources {
        println!(
            "{:<30} {:<12} {:<10} {}",
            r.id, r.version, r.channel, r.status
        );
    }
    Ok(())
}

async fn cmd_status(db: &str, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let inv = open_inventory(db).await?;
    if let Some(r) = inv.get_resource(id).await? {
        println!("ID:          {}", r.id);
        println!("Version:     {}", r.version);
        println!("Channel:     {}", r.channel);
        println!("Status:      {}", r.status);
        println!("Installed:   {}", r.installed_at);
        println!("Config:      {}", r.config_path);
        println!("Data:        {}", r.data_path);
        println!("Validation:  {:?}", r.validation);
    } else {
        error!("Resource not found: {id}");
        std::process::exit(1);
    }
    Ok(())
}

async fn cmd_uninstall(db: &str, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let inv = open_inventory(db).await?;
    inv.uninstall(id).await?;
    info!("Resource uninstalled: {id}");
    Ok(())
}

async fn cmd_services(db: &str, role: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let inv = open_inventory(db).await?;
    let services = match role {
        Some(r) => inv.services_with_role(r).await?,
        None => inv.list_services().await?,
    };
    if services.is_empty() {
        println!("No services registered.");
        return Ok(());
    }
    println!("{:<20} {:<20} {:<10} STATUS", "NAME", "RESOURCE", "PORT");
    println!("{}", "-".repeat(72));
    for s in &services {
        let port = s.port.map_or_else(|| "-".to_string(), |p| p.to_string());
        println!(
            "{:<20} {:<20} {:<10} {}",
            s.instance_name, s.resource_id, port, s.status
        );
    }
    Ok(())
}

// ── Daemon ────────────────────────────────────────────────────────────────────

async fn cmd_serve(
    db: &str,
    grpc_port: u16,
    rest_port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let grpc_addr: std::net::SocketAddr = format!("0.0.0.0:{grpc_port}")
        .parse()
        .expect("valid grpc addr");
    let rest_addr: std::net::SocketAddr = format!("0.0.0.0:{rest_port}")
        .parse()
        .expect("valid rest addr");

    info!(db, grpc = %grpc_addr, rest = %rest_addr, "starting fs-inventory");

    let inventory_arc = Arc::new(Inventory::open(DbConfig::sqlite(db)).await?);

    // ── In-process bus ────────────────────────────────────────────────────────
    let mut bus = MessageBus::new();
    bus.add_handler(Arc::new(InventoryBusHandler::new(Arc::clone(
        &inventory_arc,
    ))));
    let _bus = Arc::new(bus);

    let shared: Arc<dyn InventoryStore> = inventory_arc;

    let grpc_svc = InventoryServiceServer::new(GrpcInventory::new(Arc::clone(&shared)));
    let grpc_server = tonic::transport::Server::builder()
        .add_service(grpc_svc)
        .serve(grpc_addr);

    let rest_router = rest::router(Arc::clone(&shared));
    let rest_listener = tokio::net::TcpListener::bind(rest_addr).await?;
    let rest_server = axum::serve(rest_listener, rest_router);

    info!("gRPC listening on {grpc_addr}");
    info!("REST listening on {rest_addr}");

    tokio::select! {
        result = grpc_server => {
            if let Err(e) = result {
                error!("gRPC server error: {e}");
            }
        }
        result = rest_server => {
            if let Err(e) = result {
                error!("REST server error: {e}");
            }
        }
    }

    warn!("fs-inventory shutting down");
    Ok(())
}
