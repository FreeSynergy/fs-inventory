# CLAUDE.md – fs-inventory

## What is this?

FreeSynergy Inventory — the single source of truth for what is installed on this node.
Answers "What resources are installed?" and "Which service instances are running?"
Uses its own `SQLite` file: `fs-inventory.db`.

## Rules

- Language in files: **English** (comments, code, variable names)
- Language in chat: **German**
- OOP everywhere: traits over match blocks, types carry their own behavior
- No CHANGELOG.md
- After every feature: commit directly

## Quality Gates (before every commit)

```
1. Design Pattern (Traits, Object hierarchy)
2. Structs + Traits — no impl code yet
3. cargo check
4. Impl (OOP)
5. cargo clippy --all-targets -- -D warnings
6. cargo fmt --check
7. Unit tests (min. 1 per public module)
8. cargo test
9. commit + push
```

Every lib.rs / main.rs must have:
```rust
#![deny(clippy::all, clippy::pedantic, warnings)]
```

## Architecture

- `Inventory` — primary interface: open, install, uninstall, query
- `InstalledResource` — what is installed (version, channel, status, paths)
- `ServiceInstance` — a running instance derived from a resource
- `ResourceStatus` / `ServiceStatus` — runtime state enums
- Entity types (`entity/`) — SeaORM models for `installed_resources` and `service_instances`

## Dependencies

- `fs-types` from `../fs-libs/` — `ResourceType`, `Role`, `ValidationStatus`
- `sea-orm =2.0.0-rc.37` (SQLite)
