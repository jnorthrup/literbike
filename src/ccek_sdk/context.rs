//! CCEK - CoroutineContext Element Key pattern
//!
//! Based on Kotlin's CoroutineContext:
//! - Context = compile-time optimized map with const keys
//! - Elements = values in the map (implement CoroutineContext.Element)
//! - Keys = const compile-time Key singletons (companion objects)
//!
//! Pattern mirrors Kotlin:
//! ```kotlin
//! return EmptyCoroutineContext +
//!     dhtService +
////!     protocolDetector +
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
    pub const fn new() -> Self {
        Self
    }

    pub fn with<E: CcekElement + 'static>(self, _element: E) -> Self {
        self
    }

    pub fn get<E: CcekElement + 'static>(&self) -> Option<&E> {
        None
    }

    pub fn is_empty(&self) -> bool {
        true
    }
}

impl const std::ops::Add for CcekContext {
    type Output = Self;
    fn add(self, _rhs: Self) -> Self {
        self
    }
}

#[derive(Clone, Default)]
pub struct EmptyContext;

// EmptyContext is covered by the blanket impl<T: Send + Sync + 'static> CcekElement for T
// key() returns std::any::type_name::<EmptyContext>() which is "ccek_sdk::context::EmptyContext"
