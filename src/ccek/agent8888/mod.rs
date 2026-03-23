//! Agent8888 Protocol Detection - Top level module
//!
//! This is the ROOT of the CCEK hierarchy. CCEK claims all I/O and
//! production execution code from litebike protocols.
//!
//! ## Module Hierarchy
//!
//! ```text
//! protocol (detection, always enabled)
//!   │
//!   ├── io (PrefixedStream, Connection, ConnectionPool)
//!   │
//!   ├── matcher (speculative parsing)
//!   │
//!   ├── listener (TCP socket acceptance)
//!   │   └── depends on: protocol, io
//!   │
//!   ├── reactor (event loop)
//!   │   └── depends on: listener
//!   │
//!   ├── timer (timeout management)
//!   │   └── depends on: reactor
//!   │
//!   └── handler (protocol-specific I/O)
//!       └── depends on: reactor, io
//! ```

// Private modules - internal implementation
mod protocol;

#[cfg(feature = "io")]
mod io;

#[cfg(feature = "matcher")]
mod matcher;

#[cfg(feature = "listener")]
mod listener;

#[cfg(feature = "reactor")]
mod reactor;

#[cfg(feature = "timer")]
mod timer;

#[cfg(feature = "handler")]
mod handler;

// Public exports - controlled API surface
pub use protocol::{
    detect_protocol, Agent8888Element, Agent8888Key, HttpElement, HttpKey, HttpMethod, HtxElement,
    HtxKey, ProtocolDetection, QuicElement, QuicKey, SctpElement, SctpKey, SshElement, SshKey,
    TlsElement, TlsKey, CcekDetectionResult, CcekProtocolDetector, CcekProtocolHandler,
    CcekHandlerResult, CcekProtocolRegistryKey, CcekProtocolRegistryElement, BitFlags,
};

// Re-export I/O types (when io feature enabled)
#[cfg(feature = "io")]
pub use io::{PrefixedStream, Connection, ConnectionPool, IoStats};

// Re-export core types
pub mod core {
    use std::any::{Any, TypeId};

    /// Element trait - stored in Context
    pub trait Element: Send + Sync + 'static {
        fn key_type(&self) -> TypeId;
        fn as_any(&self) -> &dyn Any;
    }

    /// Key trait - passive SDK provider
    pub trait Key: 'static {
        type Element: Element;
        const FACTORY: fn() -> Self::Element;
    }

    /// Context - hierarchical COW chain
    #[derive(Clone)]
    pub enum Context {
        Empty,
        Cons {
            element: std::sync::Arc<dyn Element>,
            tail: std::sync::Arc<Context>,
        },
    }

    impl Default for Context {
        fn default() -> Self {
            Context::Empty
        }
    }

    impl Context {
        pub fn new() -> Self {
            Context::Empty
        }

        pub fn plus<E: Element>(self, element: E) -> Self {
            Context::Cons {
                element: std::sync::Arc::new(element),
                tail: std::sync::Arc::new(self),
            }
        }

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
                            tail: std::sync::Arc::new(tail.minus_by_key(key_id)),
                        }
                    }
                }
            }
        }

        pub fn is_empty(&self) -> bool {
            matches!(self, Context::Empty)
        }

        pub fn len(&self) -> usize {
            match self {
                Context::Empty => 0,
                Context::Cons { tail, .. } => 1 + tail.len(),
            }
        }
    }
}

pub use core::{Context, Element, Key};

#[cfg(test)]
mod ccek_tests {
    use super::protocol::{Agent8888Element, Agent8888Key};
    use super::*;

    #[test]
    fn test_key_factory_creates_element() {
        // Key provides factory, creates Element
        let elem = Agent8888Key::FACTORY();
        assert_eq!(elem.port, 8888);

        // Element knows its Key type
        assert_eq!(elem.key_type(), std::any::TypeId::of::<Agent8888Key>());
    }

    #[test]
    fn test_context_plus_creates_cow_chain() {
        let ctx = Context::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.len(), 0);

        // COW: plus creates new context, original unchanged
        let ctx = ctx.plus(Agent8888Key::FACTORY());
        assert!(!ctx.is_empty());
        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn test_context_get_by_key_type() {
        let ctx = Context::new().plus(Agent8888Key::FACTORY());

        // Get by Key type, returns Element
        let elem = ctx
            .get::<Agent8888Key>()
            .unwrap()
            .as_any()
            .downcast_ref::<Agent8888Element>();

        assert!(elem.is_some());
        assert_eq!(elem.unwrap().port, 8888);
    }

    #[test]
    fn test_context_minus_removes_element() {
        let ctx = Context::new().plus(Agent8888Key::FACTORY());

        assert_eq!(ctx.len(), 1);

        // minus creates new context without element
        let ctx = ctx.minus::<Agent8888Key>();
        assert!(ctx.is_empty());
        assert_eq!(ctx.len(), 0);
    }

    #[test]
    fn test_hierarchical_context_parent_child() {
        // Parent context has base elements
        let parent = Context::new().plus(Agent8888Key::FACTORY());

        // Child context extends parent (COW tail)
        #[cfg(feature = "listener")]
        {
            use listener::{ListenerElement, ListenerKey};
            let child = parent.clone().plus(ListenerKey::FACTORY());

            // Child can see both
            assert!(child.get::<Agent8888Key>().is_some());
            assert!(child.get::<ListenerKey>().is_some());

            // Parent only sees base
            assert!(parent.get::<Agent8888Key>().is_some());
            assert!(parent.get::<ListenerKey>().is_none());
        }
    }

    #[test]
    fn test_key_element_one_to_one() {
        // Each Key has exactly one Element type
        fn check_key_element_pair<K: Key>() {
            let _elem = K::FACTORY();
            // Type system enforces 1:1 via associated type
        }

        check_key_element_pair::<Agent8888Key>();

        #[cfg(feature = "listener")]
        check_key_element_pair::<listener::ListenerKey>();

        #[cfg(feature = "reactor")]
        check_key_element_pair::<reactor::ReactorKey>();
    }
}

// Feature-gated public exports
#[cfg(feature = "matcher")]
pub use matcher::{Confidence, MatchResult, SpeculativeMatcher};

#[cfg(feature = "listener")]
pub use listener::{ListenerElement, ListenerKey};

#[cfg(feature = "reactor")]
pub use reactor::{InterestSet, ReactorElement, ReactorKey, ReadyEvent};

#[cfg(feature = "timer")]
pub use timer::{TimerElement, TimerId, TimerKey};

#[cfg(feature = "handler")]
pub use handler::{HandlerElement, HandlerKey, HandlerStats, HandlerResult};
