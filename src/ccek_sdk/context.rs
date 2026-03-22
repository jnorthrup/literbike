//! CCEK - CoroutineContext Element Key pattern
//!
//! Elements ARE Coroutines. Context hosts Coroutine[Contexts].
//! This guides the compiler through explicit performant locality.
//!
//! Pattern:
//! - Element = Coroutine (async fn returning Self)
//! - Key = static const factory for Coroutine
//! - Context = host of Coroutine[Contexts]
//!
//! The compiler follows this structure for optimal code generation.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};

// ============================================================================
// CcekKey - static const factory for Coroutine (weight 100)
// ============================================================================

pub trait CcekKey: 'static {
    type Element: CcekElement;
}

// ============================================================================
// CcekElement - Coroutine (async) (weight 100 for access/mutate)
// ============================================================================

pub trait CcekElement: Send + Sync + 'static {
    fn key(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Send + Sync + 'static> CcekElement for T {
    fn key(&self) -> &'static str {
        std::any::type_name::<T>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ============================================================================
// CcekCoroutine - async execution unit
// ============================================================================

pub trait CcekCoroutine: CcekElement {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output>;
}

// ============================================================================
// CcekContext - host of Coroutine[Contexts] (explicit locality)
// ============================================================================

#[derive(Clone, Default)]
pub struct CcekContext {
    elements: HashMap<TypeId, Box<dyn CcekElement>>,
}

impl CcekContext {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
        }
    }

    /// Add a Coroutine Element to the Context (explicit locality)
    pub fn with<E: CcekElement + 'static>(mut self, element: E) -> Self {
        self.elements.insert(TypeId::of::<E>(), Box::new(element));
        self
    }

    /// Get Element by Key (compile-time resolved)
    pub fn get<E: CcekElement + 'static>(&self) -> Option<&E> {
        self.elements
            .get(&TypeId::of::<E>())
            .and_then(|e| e.as_any().downcast_ref())
    }

    /// Host a Coroutine[Context] (nested async context)
    pub fn with_coroutine<C: CcekCoroutine>(self, coroutine: C) -> Self {
        self.with(coroutine)
    }

    pub fn plus(&self, other: Self) -> Self {
        let mut result = self.clone();
        for (k, v) in other.elements {
            result.elements.insert(k, v);
        }
        result
    }
}

impl std::ops::Add for CcekContext {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.plus(rhs)
    }
}

// Empty context as base
#[derive(Clone, Default)]
pub struct EmptyContext;

impl CcekElement for EmptyContext {
    fn key(&self) -> &'static str {
        "EmptyContext"
    }
}
