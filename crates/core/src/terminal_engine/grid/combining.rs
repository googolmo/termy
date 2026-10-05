//! Bounded, per-terminal interning for repeated composed cells.
//!
//! A set-associative table keeps lookup work constant. Entries are immutable:
//! appending another mark produces a new value and cannot change an older cell.

use std::sync::Arc;

use super::super::types::{Cell, CellExtra, MAX_COMBINING_BYTES};

const BUCKETS: usize = 64;
const WAYS: usize = 4;
// Avoid pinning large OSC 8 strings after their last visible cell is erased.
const MAX_CACHED_HYPERLINK_BYTES: usize = 1024;

#[inline]
fn bytes_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    // A UTF-8 mark is at most four bytes, and the usual prefix is empty.
    // Fixed-width comparisons avoid a library memcmp call for these tiny keys.
    match left.len() {
        0 => true,
        1 => left[0] == right[0],
        2 => u16::from_ne_bytes([left[0], left[1]]) == u16::from_ne_bytes([right[0], right[1]]),
        3 => {
            u16::from_ne_bytes([left[0], left[1]]) == u16::from_ne_bytes([right[0], right[1]])
                && left[2] == right[2]
        }
        4 => {
            u32::from_ne_bytes([left[0], left[1], left[2], left[3]])
                == u32::from_ne_bytes([right[0], right[1], right[2], right[3]])
        }
        _ => left == right,
    }
}

fn bucket_for(suffix: &[u8], encoded: &[u8], link_identity: usize) -> usize {
    // FNV-1a avoids a general-purpose hasher's setup for the usual two-byte
    // mark. The table is fixed-size and every lookup checks at most WAYS full
    // keys, so even deliberately colliding input has bounded lookup cost.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ (link_identity as u64).rotate_right(4);
    for &byte in suffix {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    for &byte in encoded {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    (hash ^ (hash >> 32)) as usize % BUCKETS
}

pub(super) struct CombiningCache {
    slots: [[Option<Arc<CellExtra>>; WAYS]; BUCKETS],
    next: [u8; BUCKETS],
}

impl Default for CombiningCache {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| std::array::from_fn(|_| None)),
            next: [0; BUCKETS],
        }
    }
}

impl CombiningCache {
    pub(super) fn clear(&mut self) {
        for bucket in &mut self.slots {
            bucket.fill(None);
        }
        self.next.fill(0);
    }

    pub(super) fn append(&mut self, cell: &mut Cell, character: char) {
        let suffix = cell.combining();
        let mut encoded = [0; 4];
        let encoded = character.encode_utf8(&mut encoded).as_bytes();
        let len = suffix.len() + encoded.len();
        if len > MAX_COMBINING_BYTES {
            return;
        }
        let hyperlink = cell.hyperlink();
        if hyperlink.is_some_and(|link| {
            link.id.capacity().saturating_add(link.uri.capacity()) > MAX_CACHED_HYPERLINK_BYTES
        }) {
            cell.push_combining(character);
            return;
        }

        let link_pointer = hyperlink.map_or(std::ptr::null(), Arc::as_ptr);
        // Use the full resulting byte string as the key; grouping does not
        // affect identity when a value was built over different feed chunks.
        let bucket = bucket_for(suffix.as_bytes(), encoded, link_pointer as usize);
        for entry in self.slots[bucket].iter().flatten() {
            let same_link = entry
                .hyperlink
                .as_ref()
                .map_or(std::ptr::null(), Arc::as_ptr)
                == link_pointer;
            if same_link
                && entry.combining.len() == len
                && bytes_equal(
                    &entry.combining.as_bytes()[..suffix.len()],
                    suffix.as_bytes(),
                )
                && bytes_equal(&entry.combining.as_bytes()[suffix.len()..], encoded)
            {
                cell.extra = Some(Arc::clone(entry));
                return;
            }
        }

        let mut combining = String::with_capacity(len);
        combining.push_str(suffix);
        combining.push(character);
        let extra = Arc::new(CellExtra {
            combining,
            hyperlink: hyperlink.cloned(),
        });
        let slot = usize::from(self.next[bucket]);
        self.next[bucket] = ((slot + 1) % WAYS) as u8;
        self.slots[bucket][slot] = Some(Arc::clone(&extra));
        cell.extra = Some(extra);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_engine::Hyperlink;

    #[test]
    fn short_and_long_byte_keys_check_every_byte_and_length() {
        for len in [0, 1, 2, 3, 4, 5, MAX_COMBINING_BYTES] {
            // Include bytes that cannot stand alone as valid UTF-8: this helper
            // compares the entire key without assuming scalar boundaries.
            let key: Vec<_> = (0..len).map(|index| index as u8).collect();
            assert!(bytes_equal(&key, &key));
            for index in 0..len {
                let mut other = key.clone();
                other[index] ^= 0xff;
                assert!(!bytes_equal(&key, &other), "length {len}, byte {index}");
            }
            let mut longer = key.clone();
            longer.push(0);
            assert!(!bytes_equal(&key, &longer));
            assert!(!bytes_equal(&longer, &key));
        }
    }

    #[test]
    fn repeated_compositions_share_immutable_metadata() {
        let mut cache = CombiningCache::default();
        let mut first = Cell::default();
        let mut second = Cell::default();
        cache.append(&mut first, '\u{301}');
        cache.append(&mut second, '\u{301}');
        assert!(Arc::ptr_eq(
            first.extra.as_ref().unwrap(),
            second.extra.as_ref().unwrap()
        ));
        cache.append(&mut second, '\u{308}');
        assert_eq!(first.combining(), "\u{301}");
        assert_eq!(second.combining(), "\u{301}\u{308}");
        assert!(!Arc::ptr_eq(
            first.extra.as_ref().unwrap(),
            second.extra.as_ref().unwrap()
        ));
    }

    #[test]
    fn colliding_compositions_remain_distinct_and_survive_eviction() {
        let bucket = bucket_for(&[], "\u{300}".as_bytes(), 0);
        let colliding: Vec<_> = (0x300..0x1000)
            .filter_map(char::from_u32)
            .filter(|character| {
                let mut encoded = [0; 4];
                bucket_for(&[], character.encode_utf8(&mut encoded).as_bytes(), 0) == bucket
            })
            .take(WAYS + 1)
            .collect();
        assert_eq!(colliding.len(), WAYS + 1);

        let mut cache = CombiningCache::default();
        let mut cells = Vec::new();
        for &character in &colliding {
            let mut cell = Cell::default();
            cache.append(&mut cell, character);
            assert_eq!(cell.combining(), character.to_string());
            let mut repeated = Cell::default();
            cache.append(&mut repeated, character);
            assert!(Arc::ptr_eq(
                cell.extra.as_ref().unwrap(),
                repeated.extra.as_ref().unwrap()
            ));
            cells.push(cell);
        }

        // A full bucket evicts the oldest entry, but the cell still owns it.
        assert_eq!(Arc::strong_count(cells[0].extra.as_ref().unwrap()), 1);
        let original = cells[0].clone();
        cache.append(&mut cells[0], '\u{308}');
        assert_eq!(original.combining(), colliding[0].to_string());
        assert_eq!(cells[0].combining(), format!("{}\u{308}", colliding[0]));
        cache.clear();
        for (cell, character) in cells.iter().zip(&colliding).skip(1) {
            assert_eq!(cell.combining(), character.to_string());
        }
    }

    #[test]
    fn distinct_hyperlinks_never_share_composed_metadata() {
        let mut cache = CombiningCache::default();
        let mut first = Cell {
            extra: Some(Arc::new(CellExtra {
                combining: String::new(),
                hyperlink: Some(Arc::new(Hyperlink {
                    id: "first".into(),
                    uri: "https://one.test".into(),
                })),
            })),
            ..Cell::default()
        };
        let mut second = Cell {
            extra: Some(Arc::new(CellExtra {
                combining: String::new(),
                hyperlink: Some(Arc::new(Hyperlink {
                    id: "second".into(),
                    uri: "https://two.test".into(),
                })),
            })),
            ..Cell::default()
        };
        cache.append(&mut first, '\u{301}');
        // Force a pointer-hash collision so this exercises the full hyperlink
        // identity check rather than relying on placement in different buckets.
        let second_bucket = bucket_for(
            &[],
            "\u{301}".as_bytes(),
            Arc::as_ptr(second.hyperlink().unwrap()) as usize,
        );
        cache.slots[second_bucket][0] = first.extra.clone();
        cache.next[second_bucket] = 1;
        cache.append(&mut second, '\u{301}');
        assert_eq!(first.hyperlink().unwrap().id, "first");
        assert_eq!(second.hyperlink().unwrap().id, "second");
        assert!(!Arc::ptr_eq(
            first.extra.as_ref().unwrap(),
            second.extra.as_ref().unwrap()
        ));
    }

    #[test]
    fn bucket_hash_uses_the_full_composition_bytes() {
        let combined = "\u{301}\u{308}";
        assert_eq!(
            bucket_for("\u{301}".as_bytes(), "\u{308}".as_bytes(), 0x1234),
            bucket_for(&[], combined.as_bytes(), 0x1234)
        );
    }

    #[test]
    fn cache_storage_and_suffixes_are_bounded_under_unique_input() {
        let mut cache = CombiningCache::default();
        for index in 0..10_000 {
            let mut cell = Cell::default();
            cache.append(&mut cell, char::from_u32(0x1000 + index).unwrap());
        }
        let slots: Vec<_> = cache.slots.iter().flatten().flatten().collect();
        assert!(slots.len() <= BUCKETS * WAYS);
        assert!(
            slots
                .iter()
                .all(|entry| entry.combining.capacity() <= MAX_COMBINING_BYTES)
        );
        let mut cell = Cell::default();
        for _ in 0..1000 {
            cache.append(&mut cell, '\u{1d185}');
        }
        assert!(cell.combining().len() <= MAX_COMBINING_BYTES);
        cache.clear();
        assert!(cache.slots.iter().flatten().all(Option::is_none));
    }

    #[test]
    fn large_hyperlinks_are_not_retained_by_the_cache() {
        let link = Arc::new(Hyperlink {
            id: String::new(),
            uri: "x".repeat(MAX_CACHED_HYPERLINK_BYTES + 1),
        });
        let mut cell = Cell {
            extra: Some(Arc::new(CellExtra {
                combining: String::new(),
                hyperlink: Some(Arc::clone(&link)),
            })),
            ..Cell::default()
        };
        let mut cache = CombiningCache::default();
        cache.append(&mut cell, '\u{301}');
        assert_eq!(cell.combining(), "\u{301}");
        assert!(cache.slots.iter().flatten().all(Option::is_none));
        drop(cell);
        assert_eq!(Arc::strong_count(&link), 1);
    }
}
