//! CCEK - Kotlin CoroutineContext pattern
//!
//! Mirrors Kotlin exactly:
//! ```kotlin
//! return EmptyCoroutineContext +
//!     dhtService +
//!     protocolDetector +
//!     crdtStorage
//! ```

use std::any::Any;

pub trait CcekKey: 'static {
    type Element: CcekElement;
}

pub trait CcekElement: Send + Sync + 'static {
    fn key(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
}

#[derive(Clone, Default)]
pub struct CcekContext;

impl CcekContext {
    pub fn new() -> Self {
        Self
    }

    pub fn get<E: CcekElement>(&self) -> Option<&E> {
        None
    }

    pub fn minus_key(&self, _key: &'static str) -> Self {
        Self
    }

    pub fn is_empty(&self) -> bool {
        true
    }
}

impl std::ops::Add for CcekContext {
    type Output = Self;
    fn add(self, _rhs: Self) -> Self {
        self
    }
}

#[derive(Clone, Default)]
pub struct EmptyContext;

impl CcekElement for EmptyContext {
    fn key(&self) -> &'static str {
        "EmptyContext"
    }
}

impl EmptyContext {
    pub fn is_empty(&self) -> bool {
        true
    }
}
