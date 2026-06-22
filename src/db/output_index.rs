//! # Output Index Database
//!
//! Permanent index of all outputs ever created on-chain.
//! Unlike the UTXO set, entries are NEVER removed when spent —
//! only during reorg (block disconnection). This enables full
//! validation of ring member commitments even for spent outputs.

use crate::db::shim::{Db, Tree};
use crate::error::{Error, Result};
use super::{serialize, deserialize};
use borsh::{BorshSerialize, BorshDeserialize};

/// Minimal metadata for validating a ring member that may have been spent.
///
/// Stored permanently in sled keyed by stealth address bytes.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct OutputIndexEntry {
    /// Pedersen commitment (needed to verify ring member commitments)
    pub commitment: [u8; 32],
    /// Block height where the output was created
    pub height: u64,
    /// Whether this output came from a coinbase transaction
    pub is_coinbase: bool,
    /// Optional time-lock height
    pub lock_height: Option<u64>,
}

impl OutputIndexEntry {
    /// Validate semantic invariants on a deserialized entry.
    ///
    /// Borsh deserialization checks structural well-formedness (field
    /// types, lengths) but not SEMANTIC correctness — a malformed-but-
    /// well-typed entry would deserialize fine and then poison downstream
    /// validation logic that trusts the entry's fields.
    ///
    /// Current invariants checked:
    ///   - `lock_height >= height` if `lock_height` is Some — a time-lock
    ///     in the past is semantically meaningless (the output is already
    ///     spendable by the time it lands on chain), and a wallet/validator
    ///     that trusts a `lock_height < height` entry might apply incorrect
    ///     spendability gating.
    ///
    /// ## Why this matters
    ///
    /// The `output_index` tree is the source of truth that ring-member
    /// validation in `validate_transaction` consults to verify that decoy
    /// outputs in a ring exist and have the claimed properties. A
    /// corrupted entry (disk corruption, attacker with FS access,
    /// migration bug) that bypasses `validate()` could cause the
    /// validator to accept a ring with a structurally-invalid decoy
    /// — privacy degradation, not consensus break, but worth catching.
    ///
    /// ## Future invariants this can grow
    ///
    ///   - height <= current_chain_height (requires chain context — defer)
    ///   - lock_height < some_far_future_cap (sanity bound)
    ///   - commitment != zero-point (defense against degenerate entries)
    ///
    /// Added as item 4 of 18 in the 2026-06-20 audit-fix backport.
    pub fn validate(&self) -> Result<()> {
        if let Some(lock) = self.lock_height {
            if lock < self.height {
                return Err(Error::DatabaseError(format!(
                    "OutputIndexEntry invariant violated: lock_height ({}) < height ({}). \
                     Entry refers to an output time-locked to a height BEFORE the output \
                     was created, which is semantically impossible (a past-dated lock is \
                     never enforceable). Likely cause: DB corruption or attacker-crafted \
                     entry. Reject this entry rather than trusting downstream validators.",
                    lock, self.height,
                )));
            }
        }
        Ok(())
    }
}

/// Persistent output index database
pub struct OutputIndexDb {
    /// stealth_address_bytes -> OutputIndexEntry
    pub(crate) tree: Tree,
}

impl OutputIndexDb {
    /// Create new output index database
    pub fn new(db: &Db) -> Result<Self> {
        let tree = db.open_tree("output_index")
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(OutputIndexDb { tree })
    }

    /// Insert an output entry (oldest-wins semantics to match stealth_index).
    ///
    /// If the stealth address already exists, the entry is NOT overwritten.
    /// This matches the in-memory stealth_index behavior where old coinbase
    /// outputs sharing the same stealth address keep the oldest entry.
    pub fn insert(&self, stealth: &[u8; 32], entry: &OutputIndexEntry) -> Result<()> {
        // Only insert if not already present (oldest wins)
        if self.tree.contains_key(stealth)
            .map_err(|e| Error::DatabaseError(e.to_string()))? {
            return Ok(());
        }

        let data = serialize(entry)?;
        self.tree.insert(stealth, data)
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Get an output entry by stealth address.
    ///
    /// Returns an error (not Ok(None)) if the on-disk entry deserializes
    /// successfully but fails semantic validation. Treating a corrupt
    /// entry as "not found" would silently mask DB corruption or
    /// attacker-crafted entries; failing loud surfaces the problem to
    /// the operator instead. See `OutputIndexEntry::validate()` for
    /// the invariants checked.
    pub fn get(&self, stealth: &[u8; 32]) -> Result<Option<OutputIndexEntry>> {
        match self.tree.get(stealth) {
            Ok(Some(data)) => {
                let entry: OutputIndexEntry = deserialize(&data)?;
                entry.validate()?;
                Ok(Some(entry))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(Error::DatabaseError(e.to_string())),
        }
    }

    /// Remove an output entry (only during reorg/block disconnection)
    pub fn remove(&self, stealth: &[u8; 32]) -> Result<()> {
        self.tree.remove(stealth)
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// Count total entries
    pub fn count(&self) -> usize {
        self.tree.len()
    }

    /// Check if the index is empty (used for migration detection)
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_index_insert_and_lookup() {
        let db = crate::db::shim::Config::new().temporary(true).open().unwrap();
        let idx = OutputIndexDb::new(&db).unwrap();

        let stealth = [42u8; 32];
        let entry = OutputIndexEntry {
            commitment: [1u8; 32],
            height: 100,
            is_coinbase: false,
            lock_height: None,
        };

        idx.insert(&stealth, &entry).unwrap();
        let retrieved = idx.get(&stealth).unwrap().unwrap();
        assert_eq!(retrieved.height, 100);
        assert!(!retrieved.is_coinbase);
        assert_eq!(idx.count(), 1);
    }

    #[test]
    fn test_output_index_oldest_wins() {
        let db = crate::db::shim::Config::new().temporary(true).open().unwrap();
        let idx = OutputIndexDb::new(&db).unwrap();

        let stealth = [99u8; 32];
        let entry1 = OutputIndexEntry {
            commitment: [1u8; 32],
            height: 50,
            is_coinbase: true,
            lock_height: None,
        };
        let entry2 = OutputIndexEntry {
            commitment: [2u8; 32],
            height: 200,
            is_coinbase: false,
            lock_height: None,
        };

        idx.insert(&stealth, &entry1).unwrap();
        idx.insert(&stealth, &entry2).unwrap(); // should NOT overwrite
        let retrieved = idx.get(&stealth).unwrap().unwrap();
        assert_eq!(retrieved.height, 50, "oldest entry should win");
        assert!(retrieved.is_coinbase);
    }

    #[test]
    fn test_output_index_remove() {
        let db = crate::db::shim::Config::new().temporary(true).open().unwrap();
        let idx = OutputIndexDb::new(&db).unwrap();

        let stealth = [7u8; 32];
        let entry = OutputIndexEntry {
            commitment: [0u8; 32],
            height: 10,
            is_coinbase: false,
            lock_height: Some(500),
        };

        idx.insert(&stealth, &entry).unwrap();
        assert!(!idx.is_empty());
        idx.remove(&stealth).unwrap();
        assert!(idx.get(&stealth).unwrap().is_none());
    }

    // ─── OutputIndexEntry::validate() regression tests ────────────────

    /// Happy path: lock_height >= height passes validate(). Pinned so
    /// future invariant additions don't accidentally reject legitimate
    /// time-locked outputs.
    #[test]
    fn validate_legitimate_locked_entry_passes() {
        let entry = OutputIndexEntry {
            commitment: [1u8; 32],
            height: 100,
            is_coinbase: false,
            lock_height: Some(200),
        };
        assert!(entry.validate().is_ok());
    }

    /// Edge case: lock_height == height. The lock is enforceable for
    /// exactly one block window (the block that creates the output).
    /// Allowed by the `lock < height` rejection.
    #[test]
    fn validate_lock_at_creation_height_passes() {
        let entry = OutputIndexEntry {
            commitment: [1u8; 32],
            height: 100,
            is_coinbase: false,
            lock_height: Some(100),
        };
        assert!(entry.validate().is_ok());
    }

    /// Unlocked entry (no lock_height) always passes.
    #[test]
    fn validate_unlocked_entry_passes() {
        let entry = OutputIndexEntry {
            commitment: [1u8; 32],
            height: 100,
            is_coinbase: false,
            lock_height: None,
        };
        assert!(entry.validate().is_ok());
    }

    /// Corruption guard: lock_height < height is semantically impossible
    /// (past-dated time-lock is never enforceable). MUST be rejected
    /// before downstream validators trust it.
    #[test]
    fn validate_past_dated_lock_rejected() {
        let entry = OutputIndexEntry {
            commitment: [1u8; 32],
            height: 200,
            is_coinbase: false,
            lock_height: Some(100),  // < height
        };
        let err = entry.validate().unwrap_err();
        assert!(
            err.to_string().contains("lock_height") && err.to_string().contains("height"),
            "error should explain the lock_height vs height violation; got: {}", err,
        );
    }

    /// `get()` propagates validation errors instead of returning Ok(None).
    /// This is the load-bearing wire-in: callers don't need to remember
    /// to call validate() themselves; trusting `get()` is safe.
    #[test]
    fn get_rejects_stored_invalid_entry() {
        let db = crate::db::shim::Config::new().temporary(true).open().unwrap();
        let idx = OutputIndexDb::new(&db).unwrap();
        let stealth = [42u8; 32];

        // Bypass insert() and write a malformed entry directly to the
        // tree to simulate disk corruption / migration bug. (insert()
        // doesn't call validate() — by design, since validate() is the
        // READ-time defense; writers are trusted.)
        let bad_entry = OutputIndexEntry {
            commitment: [1u8; 32],
            height: 500,
            is_coinbase: false,
            lock_height: Some(100),  // INVALID
        };
        let data = serialize(&bad_entry).unwrap();
        idx.tree.insert(&stealth, data).unwrap();

        // get() must surface the validation error, NOT silently return None
        let result = idx.get(&stealth);
        assert!(result.is_err(), "get() should propagate invariant violation");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("OutputIndexEntry invariant violated"),
            "expected invariant-violation error from get(); got: {}", err,
        );
    }
}
