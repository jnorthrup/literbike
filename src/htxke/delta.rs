//! CCEK Protocol Delta - Multiple inlets, tributaries, outflows per protocol
//!
//! Like a river delta, each protocol has:
//! - INLETS: incoming data sources
//! - TRIBUTARIES: branching sub-streams
//! - OUTFLOWS: outgoing data sinks
//!
//! ## Architecture
//!
//! ```text
//!                    DELTA
//!    ┌──────────────────────────────────────┐
//!    │           HTTP PROTOCOL               │
//!    │                                      │
//!    │  INLETS         TRIBUTARIES    OUTFLOWS
//!    │  ┌─────┐       ┌─────┐       ┌─────┐  │
//!    │  │req_h│──┬────│chunk│──┬────│res_h│  │
//!    │  │req_b│  │    │body │  │    │res_b│  │
//!    │  └─────┘  │    └─────┘  │    └─────┘  │
//!    │            │             │             │
//!    │       ┌────▼────┐  ┌─────▼─────┐      │
//!    │       │ header  │  │  body     │       │
//!    │       │ stream  │  │  stream   │       │
//!    │       └─────────┘  └───────────┘       │
//!    └──────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::channels::{Channel, ChannelRx, ChannelTx};

#[derive(Clone)]
pub struct Inlet<T> {
    tx: ChannelTx<T>,
}

impl<T: Send + 'static> Inlet<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            tx: Channel::new(capacity).tx,
        }
    }

    pub fn send(&self, value: T) -> Result<(), super::ChannelError<T>> {
        self.tx.send(value)
    }

    pub fn try_send(&self, value: T) -> Result<(), super::ChannelError<T>> {
        self.tx.try_send(value)
    }
}

#[derive(Clone)]
pub struct Outflow<T> {
    rx: ChannelRx<T>,
}

impl<T: Send + 'static> Outflow<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            rx: Channel::new(capacity).rx,
        }
    }

    pub fn recv(&self) -> Option<T> {
        self.rx.recv()
    }

    pub fn try_recv(&self) -> Option<T> {
        self.rx.try_recv()
    }
}

#[derive(Clone)]
pub struct Tributary<T> {
    ch: Channel<T>,
}

impl<T: Send + 'static> Tributary<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            ch: Channel::new(capacity),
        }
    }

    pub fn inlet(&self) -> Inlet<T> {
        Inlet {
            tx: self.ch.tx.clone(),
        }
    }

    pub fn outflow(&self) -> Outflow<T> {
        Outflow {
            rx: self.ch.rx.clone(),
        }
    }

    pub fn split(self) -> (Inlet<T>, Outflow<T>) {
        (self.inlet(), self.outflow())
    }
}

pub struct Delta<T> {
    inlets: HashMap<&'static str, ChannelTx<T>>,
    tributaries: HashMap<&'static str, Channel<T>>,
    outflows: HashMap<&'static str, ChannelRx<T>>,
}

impl<T: Send + 'static> Delta<T> {
    pub fn new() -> Self {
        Self {
            inlets: HashMap::new(),
            tributaries: HashMap::new(),
            outflows: HashMap::new(),
        }
    }

    pub fn add_inlet(mut self, name: &'static str, capacity: usize) -> Self {
        let ch = Channel::new(capacity);
        self.inlets.insert(name, ch.tx);
        self
    }

    pub fn add_tributary(mut self, name: &'static str, capacity: usize) -> Self {
        let ch = Channel::new(capacity);
        self.tributaries.insert(name, ch);
        self
    }

    pub fn add_outflow(mut self, name: &'static str, capacity: usize) -> Self {
        let ch = Channel::new(capacity);
        self.outflows.insert(name, ch.rx);
        self
    }

    pub fn inlet(&self, name: &str) -> Option<Inlet<T>> {
        self.inlets.get(name).map(|tx| Inlet { tx: tx.clone() })
    }

    pub fn tributary(&self, name: &str) -> Option<Tributary<T>> {
        self.tributaries
            .get(name)
            .map(|ch| Tributary { ch: ch.clone() })
    }

    pub fn outflow(&self, name: &str) -> Option<Outflow<T>> {
        self.outflows.get(name).map(|rx| Outflow { rx: rx.clone() })
    }
}

impl Default for Delta<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait ProtocolDelta: Send + Sync + 'static {
    type Item;

    fn delta(&self) -> &Delta<Self::Item>;

    fn inlet(&self, name: &str) -> Option<Inlet<Self::Item>> {
        self.delta().inlet(name)
    }

    fn tributary(&self, name: &str) -> Option<Tributary<Self::Item>> {
        self.delta().tributary(name)
    }

    fn outflow(&self, name: &str) -> Option<Outflow<Self::Item>> {
        self.delta().outflow(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_creation() {
        let delta: Delta<Vec<u8>> = Delta::new()
            .add_inlet("request", 64)
            .add_tributary("chunked_body", 32)
            .add_outflow("response", 64);

        assert!(delta.inlet("request").is_some());
        assert!(delta.tributary("chunked_body").is_some());
        assert!(delta.outflow("response").is_some());
    }

    #[test]
    fn test_inlet_send() {
        let inlet: Inlet<Vec<u8>> = Inlet::new(10);
        inlet.send(vec![1, 2, 3]).unwrap();
    }

    #[test]
    fn test_tributary_split() {
        let trib: Tributary<Vec<u8>> = Tributary::new(10);
        let (inlet, outflow) = trib.split();
        inlet.send(vec![1, 2, 3]).unwrap();
        assert_eq!(outflow.recv(), Some(vec![1, 2, 3]));
    }
}
