//! ENDGAME Architecture
//!
//! Processing paths for lean mean I/O.

pub mod endgame;

pub use endgame::{
    ProcessingPath, EndgameCapabilities, SimdLevel,
    UringFacade, SqEntry, CqEntry, OpCode,
};
