pub mod types;
pub mod database;
pub mod documents;
pub mod views;
pub mod attachments;
// pub mod api;  // Requires axum, tower_http, utoipa - userspace TODO
// pub mod ipfs;  // Requires ipfs-api-backend-hyper - userspace TODO
// pub mod m2m;  // Requires tokio - userspace TODO
pub mod tensor;
pub mod cursor;
pub mod error;
// pub mod git_sync;  // Requires tokio, git2, notify - userspace TODO

pub use types::*;
pub use database::*;
pub use error::CouchError;