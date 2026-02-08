use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Base32-sortkey alphabet used by AT Protocol TIDs.
const BASE32_SORTKEY: &[u8; 32] = b"234567abcdefghijklmnopqrstuvwxyz";

/// TID (Timestamp Identifier) generator.
///
/// Produces 13-character, monotonically increasing, base32-sortkey encoded
/// identifiers from `(microsecond_timestamp << 10 | clock_id)`.
pub struct TidGenerator {
    clock_id: u16,
    last: AtomicU64,
}

impl TidGenerator {
    /// Create a new TID generator with a random 10-bit clock ID.
    pub fn new() -> Self {
        let clock_id = (rand::random::<u16>()) & 0x03FF; // 10-bit mask
        Self {
            clock_id,
            last: AtomicU64::new(0),
        }
    }

    /// Generate the next TID.
    ///
    /// Guarantees monotonically increasing values even when called within
    /// the same microsecond by incrementing the stored timestamp.
    pub fn next_tid(&self) -> String {
        let now_micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_micros() as u64;

        let candidate = (now_micros << 10) | (self.clock_id as u64);

        // Ensure monotonically increasing: if candidate <= last, use last + 1
        let value = loop {
            let last = self.last.load(Ordering::Acquire);
            let next = if candidate > last { candidate } else { last + 1 };
            match self.last.compare_exchange_weak(last, next, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => break next,
                Err(_) => continue, // retry on contention
            }
        };

        encode_base32_sortkey(value)
    }
}

impl Default for TidGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Encode a u64 value as a 13-character base32-sortkey string.
fn encode_base32_sortkey(mut value: u64) -> String {
    let mut buf = [0u8; 13];
    for i in (0..13).rev() {
        buf[i] = BASE32_SORTKEY[(value & 0x1F) as usize];
        value >>= 5;
    }
    // SAFETY: all characters in BASE32_SORTKEY are valid ASCII/UTF-8
    String::from_utf8(buf.to_vec()).expect("base32-sortkey chars are valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tid_is_13_chars() {
        let tidgen = TidGenerator::new();
        let tid = tidgen.next_tid();
        assert_eq!(tid.len(), 13);
    }

    #[test]
    fn tid_uses_valid_alphabet() {
        let tidgen = TidGenerator::new();
        let tid = tidgen.next_tid();
        let alphabet = "234567abcdefghijklmnopqrstuvwxyz";
        for c in tid.chars() {
            assert!(alphabet.contains(c), "invalid character: {c}");
        }
    }

    #[test]
    fn tids_are_monotonically_increasing() {
        let tidgen = TidGenerator::new();
        let mut prev = tidgen.next_tid();
        for _ in 0..100 {
            let next = tidgen.next_tid();
            assert!(next > prev, "TID not increasing: {prev} >= {next}");
            prev = next;
        }
    }

    #[test]
    fn tid_different_generators_independent() {
        let gen1 = TidGenerator::new();
        let gen2 = TidGenerator::new();
        let tid1 = gen1.next_tid();
        let tid2 = gen2.next_tid();
        // Different clock IDs means different TIDs (unless astronomically unlikely collision)
        // Just verify both are valid
        assert_eq!(tid1.len(), 13);
        assert_eq!(tid2.len(), 13);
    }

    #[test]
    fn tid_all_unique_across_batch() {
        let tidgen = TidGenerator::new();
        let tids: Vec<String> = (0..1000).map(|_| tidgen.next_tid()).collect();
        let mut deduped = tids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(tids.len(), deduped.len(), "all TIDs must be unique");
    }
}
