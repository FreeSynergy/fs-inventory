# fs-inventory

Local inventory of installed FreeSynergy resources — the single source of truth
for "what is installed on this node?".

## Build

```sh
cargo build --release
cargo test
```

## Architecture

- `Inventory` — primary interface: open database, install/uninstall resources, query services
- `InstalledResource` — what is installed (version, channel, status, paths)
- `ServiceInstance` — a running (or stopped) container instance
- `ResourceStatus` / `ServiceStatus` — runtime state enums
- Entity types (`entity/`) — SeaORM models for `installed_resources` and `service_instances`

## Database

Uses its own `SQLite` file: `fs-inventory.db`.
No other component may maintain a parallel list of installed resources.
