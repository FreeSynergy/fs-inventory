//! Error type for inventory operations.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum InventoryError {
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("resource not found: {id}")]
    NotFound { id: String },

    #[error("resource already installed: {id}")]
    AlreadyInstalled { id: String },

    #[error("serialisation error: {0}")]
    Json(#[from] serde_json::Error),
}
