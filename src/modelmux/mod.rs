//! ModelMux Models API - Model Caching and Selection
//!
//! Provides model caching, selection, and proxy routing similar to Kilo.ai Gateway.
//! Boots from env and .env config, caches model selections.

pub mod cache;
pub mod registry;
pub mod proxy;
pub mod metamodel;
pub mod control;
pub mod toolbar;
pub mod utils;
pub mod streaming;

pub use cache::{CachedModel, ModelCache};
pub use registry::{ModelRegistry, ModelEntry, ProviderEntry};
pub use proxy::{ModelProxy, ProxyConfig, ProxyRoute};
pub use metamodel::{Metamodel, MetamodelCache, BlobStore, HfModelCard, HfCardCache, fetch_hf_model_card};
pub use control::{GatewayRuntimeControl, GatewayControlAction, GatewayControlState};
pub use toolbar::{ToolbarAction, ToolbarState};
