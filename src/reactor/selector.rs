// Manual readiness selector (port of Trikeshed ManualSelector.kt)

use crate::reactor::operation::Interest;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(pub u32);

#[derive(Debug, Clone)]
pub struct ReadinessState {
    pub channel_id: ChannelId,
    pub readable: bool,
    pub writable: bool,
}

pub struct ManualSelector {
    interests: Arc<Mutex<HashMap<ChannelId, Interest>>>,
    ready: Arc<Mutex<Vec<ReadinessState>>>,
}

impl ManualSelector {
    pub fn new() -> Self {
        Self {
            interests: Arc::new(Mutex::new(HashMap::new())),
            ready: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn register(&self, id: ChannelId, interest: Interest) {
        self.interests.lock().insert(id, interest);
    }
    pub fn unregister(&self, id: ChannelId) {
        self.interests.lock().remove(&id);
        self.ready.lock().retain(|r| r.channel_id != id);
    }
    pub fn mark_ready(&self, id: ChannelId, readable: bool, writable: bool) {
        let mut ready = self.ready.lock();
        if let Some(pos) = ready.iter().position(|r| r.channel_id == id) {
            ready[pos] = ReadinessState { channel_id: id, readable, writable };
        } else {
            ready.push(ReadinessState { channel_id: id, readable, writable });
        }
    }
    pub fn poll(&self, _timeout_ms: u64) -> Vec<ReadinessState> {
        let mut ready = self.ready.lock();
        let out = ready.clone();
        ready.clear();
        out
    }
}

impl Default for ManualSelector {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mark_and_poll() {
        let sel = ManualSelector::new();
        sel.register(ChannelId(1), Interest::read());
        sel.mark_ready(ChannelId(1), true, false);
        let ready = sel.poll(0);
        assert_eq!(ready.len(), 1);
        assert!(ready[0].readable);
    }
}
