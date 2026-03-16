# Implementation Plan: Userspace Channel Fixes

## Task 1.1: Add Clone implementation for RendezvousChannel

**File:** `/Users/jim/work/userspace/src/concurrency/channels/channel.rs`

**Current Issue:**
```rust
pub fn channel<T: Send>() -> (Sender<T>, Receiver<T>) {
    let ch = RendezvousChannel::new();
    (Sender(ch.clone()), Receiver(ch))  // ERROR: no method named `clone`
}
```

**Solution:**
Since `RendezvousChannel<T>` contains `Arc<T>` fields, we can implement `Clone` manually:

```rust
impl<T: Send> Clone for RendezvousChannel<T> {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            closed: self.closed.clone(),
            sender_waker: self.sender_waker.clone(),
            receiver_waker: self.receiver_waker.clone(),
        }
    }
}
```

**Verification:**
- Channel constructors compile without errors
- Tests pass: `cargo test --lib test_rendezvous_channel`

---

## Task 1.2: Implement missing trait methods for UnboundedChannel

**File:** `/Users/jim/work/userspace/src/concurrency/channels/channel.rs`

**Current Issue:**
The `Channel` trait requires `try_send()` and `try_recv()` methods:
```rust
fn try_send(&self, value: T) -> Result<(), SendError<T>>;
fn try_recv(&self) -> Result<T, RecvError>;
```

But the `UnboundedChannel` implementation doesn't include them:
```rust
impl<T: Send> Channel<T> for UnboundedChannel<T> {
    // ... poll_send, poll_recv, close, is_closed, capacity ...
    // MISSING: try_send, try_recv
}
```

**Solution:**
The methods already exist as standalone methods (lines 510-528). Move them into the trait impl:

```rust
impl<T: Send> Channel<T> for UnboundedChannel<T> {
    // ... existing methods ...

    fn try_send(&self, value: T) -> Result<(), SendError<T>> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(SendError::Closed(value));
        }
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push_back(value);
        Ok(())
    }

    fn try_recv(&self) -> Result<T, RecvError> {
        let mut buffer = self.buffer.lock().unwrap();
        if let Some(value) = buffer.pop_front() {
            Ok(value)
        } else if self.closed.load(Ordering::SeqCst) {
            Err(RecvError::Closed)
        } else {
            Err(RecvError::Empty)
        }
    }
}
```

Then remove the duplicate standalone methods (or keep them as convenience wrappers).

**Verification:**
- Trait implementation is complete
- No "missing trait items" error
- All channel tests pass

---

## Task 1.3: Fix type mismatches in SendFuture and RecvFuture

**File:** `/Users/jim/work/userspace/src/concurrency/channels/channel.rs`

**Current Issue:**
```rust
pub struct Sender<T: Send>(Arc<dyn Channel<T>>);

impl<T: Send> Sender<T> {
    pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
        SendFuture {
            channel: &*self.0,  // ERROR: type mismatch
            value: Some(value),
        }.await
    }
}
```

The futures expect `Arc<dyn Channel<_>>` but receive `&dyn Channel<T>`.

**Solution:**
Clone the Arc instead of dereferencing:

```rust
pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
    SendFuture {
        channel: self.0.clone(),  // Clone the Arc
        value: Some(value),
    }.await
}

// Similarly for Receiver:
pub async fn recv(&self) -> Result<T, RecvError> {
    RecvFuture {
        channel: self.0.clone(),  // Clone the Arc
    }.await
}
```

**Verification:**
- Type checker passes
- Sender/Receiver methods compile
- Integration tests pass

---

## Task 1.4: Verify complete fix

**Command:**
```bash
cd /Users/jim/work/literbike
cargo check --lib --features json
```

**Expected Output:**
- No compilation errors
- Only warnings (if any)
- Build finishes successfully

**Follow-up:**
```bash
cargo test --lib --features json
cargo test --lib channels
```

All tests should pass.

---

## Summary

**Files to modify:**
- `/Users/jim/work/userspace/src/concurrency/channels/channel.rs`

**Total changes:**
1. Add `Clone` impl for `RendezvousChannel<T>` (~10 lines)
2. Add `try_send()` and `try_recv()` to `Channel<T>` impl for `UnboundedChannel<T>` (~20 lines)
3. Fix 2 type mismatches in `Sender::send()` and `Receiver::recv()` (2 lines)

**Estimated time:** 15 minutes

**Blocker removed:** Bun JSON Confluence Phase 2 can proceed
