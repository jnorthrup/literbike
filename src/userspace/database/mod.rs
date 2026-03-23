#[cfg(feature = "database")]
pub mod couch;
#[cfg(feature = "database")]
pub mod lsmr;

#[cfg(feature = "database")]
pub use couch::{CouchDatabase, Document};
#[cfg(feature = "database")]
pub use lsmr::{LsmrConfig, LsmrDatabase};