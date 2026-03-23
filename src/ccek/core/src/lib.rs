//! CCEK Core - Shared traits and Context for all assemblies
//!
//! This is the foundation that all assemblies (agent8888, quic, sctp) depend on.
//! Provides:
//! - Element trait (1:1 with Key)
//! - Key trait (passive SDK provider)
//! - Context (hierarchical COW chain)
//!
//! Usage:
//! ```rust
//! use ccek_core::{Context, Element, Key};
//! use std::any::{Any, TypeId};
//!
//! pub struct MyKey;
//!
//! pub struct MyElement(u32);
//!
//! impl MyElement {
//!     pub fn new() -> Self { MyElement(0) }
//! }
//!
//! impl Element for MyElement {
//!     fn key_type(&self) -> TypeId { TypeId::of::<MyKey>() }
//!     fn as_any(&self) -> &dyn Any { self }
//! }
//!
//! impl Key for MyKey {
//!     type Element = MyElement;
//!     const FACTORY: fn() -> Self::Element = || MyElement::new();
//! }
//! ```

use std::any::{Any, TypeId};
use std::sync::Arc;

// ============================================================================
// Element - stored in Context, knows its Key type
// ============================================================================

/// Element trait - stored in Context, 1:1 with Key
///
/// Each Element has exactly one Key that creates it.
/// The Element knows its Key's TypeId for lookup.
pub trait Element: Send + Sync + 'static {
    /// Returns the TypeId of this Element's Key
    fn key_type(&self) -> TypeId;

    /// Downcast to Any for type-erased storage
    fn as_any(&self) -> &dyn Any;
}

// ============================================================================
// Key - passive SDK provider
// ============================================================================

/// Key trait - passive SDK provider, creates exactly one Element
///
/// Key provides:
/// - FACTORY: fn() -> Element constructor
/// - Consts, enums, strings for the protocol
///
/// Key never takes action - it only provides the factory.
pub trait Key: 'static {
    /// The Element type this Key creates (1:1 mapping)
    type Element: Element;

    /// Factory function - Context calls this to create Element
    const FACTORY: fn() -> Self::Element;

    /// Get this Key's TypeId
    fn key_id() -> TypeId {
        TypeId::of::<Self>()
    }

    /// Get the Element's TypeId (same as Key's for 1:1)
    fn element_id() -> TypeId {
        TypeId::of::<Self::Element>()
    }
}

// ============================================================================
// Context - hierarchical COW chain
// ============================================================================

/// Context - immutable Copy-on-Write hierarchical chain
///
/// Each element in the chain is preceded by its parent (tail).
/// Lookup traverses from head to tail (most recent to oldest).
///
/// ```text
/// Context::Empty
///   .plus(ElementA)     → Cons { ElementA, tail: Empty }
///   .plus(ElementB)     → Cons { ElementB, tail: Cons { ElementA, tail: Empty } }
/// ```
///
/// Lookup: Context.get::<KeyA>() → traverses chain → finds ElementA
#[derive(Clone)]
pub enum Context {
    Empty,
    Cons {
        element: Arc<dyn Element>,
        tail: Arc<Context>,
    },
}

impl Default for Context {
    fn default() -> Self {
        Context::Empty
    }
}

impl Context {
    /// Create new empty context
    pub fn new() -> Self {
        Context::Empty
    }

    /// Add Element to chain (COW - returns new context)
    pub fn plus<E: Element>(self, element: E) -> Self {
        Context::Cons {
            element: Arc::new(element),
            tail: Arc::new(self),
        }
    }

    /// Lookup Element by Key type
    pub fn get<K: 'static>(&self) -> Option<&dyn Element> {
        let key_id = TypeId::of::<K>();
        self.get_by_key(key_id)
    }

    fn get_by_key(&self, key_id: TypeId) -> Option<&dyn Element> {
        match self {
            Context::Empty => None,
            Context::Cons { element, tail } => {
                if element.key_type() == key_id {
                    Some(element.as_ref())
                } else {
                    tail.get_by_key(key_id)
                }
            }
        }
    }

    /// Remove Element by Key type (COW)
    pub fn minus<K: 'static>(&self) -> Self {
        let key_id = TypeId::of::<K>();
        self.minus_by_key(key_id)
    }

    fn minus_by_key(&self, key_id: TypeId) -> Self {
        match self {
            Context::Empty => Context::Empty,
            Context::Cons { element, tail } => {
                if element.key_type() == key_id {
                    tail.minus_by_key(key_id)
                } else {
                    Context::Cons {
                        element: element.clone(),
                        tail: Arc::new(tail.minus_by_key(key_id)),
                    }
                }
            }
        }
    }

    /// Check if context is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, Context::Empty)
    }

    /// Get chain length
    pub fn len(&self) -> usize {
        match self {
            Context::Empty => 0,
            Context::Cons { tail, .. } => 1 + tail.len(),
        }
    }

    /// Check if context contains element for key
    pub fn contains<K: 'static>(&self) -> bool {
        self.get::<K>().is_some()
    }
}

// ============================================================================
// Context Element - for embedding Context in Context
// ============================================================================

/// Element that wraps a Context (for nested contexts)
pub struct ContextElement(pub Context);

impl Element for ContextElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<ContextElement>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ============================================================================
// Common utilities
// ============================================================================

/// Type-safe downcast helper
pub fn downcast<E: Element + 'static>(element: &dyn Element) -> Option<&E> {
    element.as_any().downcast_ref::<E>()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestKey;
    struct TestElement(u32);

    impl Element for TestElement {
        fn key_type(&self) -> TypeId {
            TypeId::of::<TestKey>()
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    impl Key for TestKey {
        type Element = TestElement;
        const FACTORY: fn() -> Self::Element = || TestElement(42);
    }

    #[test]
    fn test_key_factory() {
        let elem = TestKey::FACTORY();
        assert_eq!(elem.0, 42);
    }

    #[test]
    fn test_context_plus_get() {
        let ctx = Context::new().plus(TestKey::FACTORY());
        let elem = ctx.get::<TestKey>().unwrap();
        let e = downcast::<TestElement>(elem).unwrap();
        assert_eq!(e.0, 42);
    }

    #[test]
    fn test_context_minus() {
        let ctx = Context::new().plus(TestKey::FACTORY());
        let ctx = ctx.minus::<TestKey>();
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_context_chain_len() {
        let ctx = Context::new()
            .plus(TestKey::FACTORY())
            .plus(TestKey::FACTORY());
        assert_eq!(ctx.len(), 2);
    }
}
