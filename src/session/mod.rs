use std::sync::{Arc, Mutex};

use libpijul::change::ChangeHeader;
use libpijul::changestore::memory::Memory as MemCs;
use libpijul::changestore::ChangeStore;
use libpijul::pristine::sanakirja::Pristine;
use libpijul::pristine::{ArcTxn, Base32, ChannelRef};
use libpijul::record::{Algorithm, Builder};
use libpijul::working_copy::memory::Memory as MemWc;
use libpijul::{Hash, MutTxnT, MutTxnTExt, TxnTExt};

pub struct SessionStore {
    repo: MemWc,
    changes: MemCs,
    env: Pristine,
    session_id: String,
    turn_count: usize,
}

pub struct SessionChannel {
    pub session_id: String,
    store: Arc<Mutex<SessionStore>>,
}

/// Open or create a pijul channel for a session (in-memory).
pub fn open_channel(session_id: &str) -> Result<SessionChannel, String> {
    let repo = MemWc::new();
    let changes = MemCs::new();
    let env = Pristine::new_anon().map_err(|e| format!("Pristine::new_anon failed: {e}"))?;

    // Create the "main" channel so it exists before any operations.
    {
        let txn = env
            .arc_txn_begin()
            .map_err(|e| format!("arc_txn_begin failed: {e}"))?;
        txn.write()
            .open_or_create_channel("main")
            .map_err(|e| format!("open_or_create_channel failed: {e}"))?;
        txn.commit()
            .map_err(|e| format!("commit failed: {e}"))?;
    }

    let store = SessionStore {
        repo,
        changes,
        env,
        session_id: session_id.to_string(),
        turn_count: 0,
    };

    Ok(SessionChannel {
        session_id: session_id.to_string(),
        store: Arc::new(Mutex::new(store)),
    })
}

/// Record a turn (user or assistant message) as a patch.
/// Returns the base-32 encoded hash of the patch.
pub fn record_turn(channel: &SessionChannel, role: &str, content: &str) -> Result<String, String> {
    let mut store = channel
        .store
        .lock()
        .map_err(|e| format!("mutex poisoned: {e}"))?;

    // Write the file contents into the in-memory working copy.
    let path = format!("turns/turn_{}", store.turn_count);
    let file_content = format!("{}: {}", role, content).into_bytes();
    store.repo.add_file(&path, file_content);

    // Begin a mutable transaction.
    let txn: ArcTxn<_> = store
        .env
        .arc_txn_begin()
        .map_err(|e| format!("arc_txn_begin failed: {e}"))?;

    // Track the file in the pristine DB.
    txn.write()
        .add_file(&path, 0)
        .map_err(|e| format!("add_file txn failed: {e}"))?;

    // Get the channel reference.
    let channel_ref: ChannelRef<_> = txn
        .write()
        .open_or_create_channel("main")
        .map_err(|e| format!("open_or_create_channel failed: {e}"))?;

    // Record the change using the record_all_change pattern from the libpijul tests.
    let mut state = Builder::new();
    state
        .record(
            txn.clone(),
            Algorithm::default(),
            false,
            &libpijul::DEFAULT_SEPARATOR,
            channel_ref.clone(),
            &store.repo,
            &store.changes,
            "",
            1,
        )
        .map_err(|e| format!("Builder::record failed: {e}"))?;

    let rec = state.finish();

    let actions: Vec<_> = rec
        .actions
        .into_iter()
        .map(|a| {
            a.globalize(&*txn.read())
                .map_err(|e| format!("globalize failed: {e}"))
        })
        .collect::<Result<_, _>>()?;

    let mut change = libpijul::change::Change::make_change(
        &*txn.read(),
        &channel_ref,
        actions,
        std::mem::take(&mut *rec.contents.lock()),
        ChangeHeader {
            message: format!("turn {}: {}", store.turn_count, role),
            ..ChangeHeader::default()
        },
        Vec::new(),
    )
    .map_err(|e| format!("Change::make_change failed: {e}"))?;

    let hash = store
        .changes
        .save_change(&mut change, |_, _| Ok::<_, anyhow::Error>(()))
        .map_err(|e| format!("save_change failed: {e}"))?;

    libpijul::apply::apply_local_change(
        &mut *txn.write(),
        &channel_ref,
        &change,
        &hash,
        &rec.updatables,
    )
    .map_err(|e| format!("apply_local_change failed: {e}"))?;

    txn.commit()
        .map_err(|e| format!("commit failed: {e}"))?;

    store.turn_count += 1;
    Ok(hash.to_base32())
}

/// Get all patch hashes in the channel, optionally starting after `from_hash`.
pub fn patch_feed(
    channel: &SessionChannel,
    from_hash: Option<&str>,
) -> Result<Vec<String>, String> {
    let store = channel
        .store
        .lock()
        .map_err(|e| format!("mutex poisoned: {e}"))?;

    let txn = store
        .env
        .arc_txn_begin()
        .map_err(|e| format!("arc_txn_begin failed: {e}"))?;

    let channel_ref = txn
        .write()
        .open_or_create_channel("main")
        .map_err(|e| format!("open_or_create_channel failed: {e}"))?;

    // Determine the starting serial number (0 = from beginning).
    let from_n: u64 = if let Some(h_str) = from_hash {
        let h = Hash::from_base32(h_str.as_bytes())
            .ok_or_else(|| format!("invalid hash: {h_str}"))?;
        let txn_r = txn.read();
        match txn_r
            .get_revchanges(&channel_ref, &h)
            .map_err(|e| format!("get_revchanges failed: {e}"))?
        {
            Some(n) => n + 1,
            None => return Err(format!("hash not found in channel: {h_str}")),
        }
    } else {
        0
    };

    let mut hashes = Vec::new();
    {
        let txn_r = txn.read();
        let log = txn_r
            .log(&channel_ref.read(), from_n)
            .map_err(|e| format!("log failed: {e}"))?;

        for item in log {
            let (_n, (serialized_hash, _merkle)) =
                item.map_err(|e| format!("log iteration failed: {e}"))?;
            let h: Hash = serialized_hash.into();
            hashes.push(h.to_base32());
        }
    }

    txn.commit()
        .map_err(|e| format!("commit failed: {e}"))?;

    Ok(hashes)
}

/// Revert a turn by unapplying its patch.
pub fn revert_turn(channel: &SessionChannel, hash_str: &str) -> Result<(), String> {
    let store = channel
        .store
        .lock()
        .map_err(|e| format!("mutex poisoned: {e}"))?;

    let h = Hash::from_base32(hash_str.as_bytes())
        .ok_or_else(|| format!("invalid hash: {hash_str}"))?;

    let txn = store
        .env
        .arc_txn_begin()
        .map_err(|e| format!("arc_txn_begin failed: {e}"))?;

    let channel_ref = txn
        .write()
        .open_or_create_channel("main")
        .map_err(|e| format!("open_or_create_channel failed: {e}"))?;

    txn.write()
        .unrecord(&store.changes, &channel_ref, &h, 0)
        .map_err(|e| format!("unrecord failed: {e}"))?;

    txn.commit()
        .map_err(|e| format!("commit failed: {e}"))?;

    Ok(())
}
