//! CCEK Context - Compile-time bound service registry
//!
//! Each service in the context is bound at compile time via its CcekKey.
//! Channels connect tributaries at compile time.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// CCEK Key - uniquely identifies a context element at compile time
pub trait CcekKey: 'static {
    type Element: CcekElement;
}

/// CCEK Element - a service bound into the context
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

/// CCEK Context - compile-time bound service registry
pub struct CcekContext {
    elements: HashMap<TypeId, Box<dyn CcekElement>>,
}

impl CcekContext {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
        }
    }

    pub fn with<K: CcekKey>(mut self, element: K::Element) -> Self
    where
        K::Element: CcekElement,
    {
        self.elements.insert(TypeId::of::<K>(), Box::new(element));
        self
    }

    pub fn get<K: CcekKey>(&self) -> Option<&K::Element>
    where
        K::Element: 'static,
    {
        self.elements
            .get(&TypeId::of::<K>())
            .and_then(|e| e.as_any().downcast_ref::<K::Element>())
    }
}

impl Default for CcekContext {
    fn default() -> Self {
        Self::new()
    }
}
