//! SCTP Association - endpoint state
//!
//! This module CANNOT see stream or chunk.

use ccek_core::{Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU32, Ordering};

/// AssociationKey - SCTP association state machine
pub struct AssociationKey;

impl AssociationKey {
    pub const FACTORY: fn() -> AssociationElement = || AssociationElement::new();
}

impl Key for AssociationKey {
    type Element = AssociationElement;
    const FACTORY: fn() -> Self::Element = AssociationKey::FACTORY;
}

/// AssociationElement - SCTP association state
pub struct AssociationElement {
    pub state: AtomicU32,
    pub active_tsn: AtomicU32,
}

impl AssociationElement {
    pub fn new() -> Self {
        Self {
            state: AtomicU32::new(AssociationState::Closed as u32),
            active_tsn: AtomicU32::new(0),
        }
    }

    pub fn state(&self) -> AssociationState {
        let s = self.state.load(Ordering::Relaxed);
        AssociationState::from_u32(s)
    }

    pub fn set_state(&self, state: AssociationState) {
        self.state.store(state as u32, Ordering::Relaxed);
    }
}

impl Element for AssociationElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<AssociationKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// SCTP association states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationState {
    Closed,
    CookieWait,
    CookieEchoed,
    Established,
    ShutdownPending,
    ShutdownReceived,
    ShutdownSent,
    ShutdownAckSent,
}

impl AssociationState {
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => AssociationState::Closed,
            1 => AssociationState::CookieWait,
            2 => AssociationState::CookieEchoed,
            3 => AssociationState::Established,
            4 => AssociationState::ShutdownPending,
            5 => AssociationState::ShutdownReceived,
            6 => AssociationState::ShutdownSent,
            7 => AssociationState::ShutdownAckSent,
            _ => AssociationState::Closed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_association_factory() {
        let elem = AssociationKey::FACTORY();
        assert_eq!(elem.state(), AssociationState::Closed);
    }

    #[test]
    fn test_association_state_machine() {
        let elem = AssociationElement::new();

        elem.set_state(AssociationState::CookieWait);
        assert_eq!(elem.state(), AssociationState::CookieWait);

        elem.set_state(AssociationState::CookieEchoed);
        assert_eq!(elem.state(), AssociationState::CookieEchoed);

        elem.set_state(AssociationState::Established);
        assert_eq!(elem.state(), AssociationState::Established);
    }
}
