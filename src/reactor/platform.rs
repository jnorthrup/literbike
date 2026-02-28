//! Platform I/O factory abstraction for reactor components.

use crate::reactor::selector::{ManualSelector, SelectorBackend};
use std::io;

pub trait PlatformIO {
    type Selector: SelectorBackend;

    fn create_selector(&self) -> io::Result<Self::Selector>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PortablePlatformIO;

impl PlatformIO for PortablePlatformIO {
    type Selector = ManualSelector;

    fn create_selector(&self) -> io::Result<Self::Selector> {
        Ok(ManualSelector::new())
    }
}
