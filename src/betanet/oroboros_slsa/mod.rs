pub mod canonicalizer;
pub mod bootstrap;

// ENDGAME-densified SLSA modules
pub mod kernel_attestation;
pub mod wam_dispatch;
pub mod couch_slsa_native;
pub mod self_hosting_verifier;

// Re-export core types for kernel-level SLSA
pub use kernel_attestation::OroborosSLSA;
pub use wam_dispatch::wam_dispatch;
pub use couch_slsa_native::SLSACouch;
pub use self_hosting_verifier::OroborosVerifier;
