//! The location table [EN 302 636-4-1 §8.1] and its duplicate packet list
//! [Annex A.2].
//!
//! One entry per known station: its last position vector, whether it is a direct
//! neighbour, when the entry expires (`itsGnLifetimeLocTE` after the last update), and the
//! last `itsGnDPLLength` sequence numbers seen from it.

use std::collections::{BTreeMap, VecDeque};

use super::packet::{GnAddr, PositionVector};

/// One location table entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LocTe {
    /// The station's last position vector.
    pub pv: PositionVector,
    /// Heard directly (a beacon, SHB, or as the link-layer sender of a forwarded packet).
    pub is_neighbour: bool,
    /// When the entry expires, ns.
    pub expires_ns: u64,
    /// Duplicate packet list: the last sequence numbers seen from this source.
    pub dpl: VecDeque<u16>,
}

/// The location table.
#[derive(Debug, Clone, Default)]
pub struct LocationTable {
    entries: BTreeMap<GnAddr, LocTe>,
    lifetime_ns: u64,
    dpl_length: usize,
}

impl LocationTable {
    /// An empty table whose entries live `lifetime_ms` and keep `dpl_length` sequence
    /// numbers.
    pub fn new(lifetime_ms: u64, dpl_length: usize) -> Self {
        LocationTable {
            entries: BTreeMap::new(),
            lifetime_ns: lifetime_ms * 1_000_000,
            dpl_length: dpl_length.max(1),
        }
    }

    /// Update (or create) the entry for `pv.addr`; a newer timestamp replaces the
    /// position, an older one only refreshes the lifetime [§8.1.2, C.2].
    pub fn update(&mut self, pv: PositionVector, neighbour: bool, now_ns: u64) {
        let expires = now_ns + self.lifetime_ns;
        let e = self.entries.entry(pv.addr).or_insert_with(|| LocTe {
            pv,
            is_neighbour: neighbour,
            expires_ns: expires,
            dpl: VecDeque::new(),
        });
        if pv.t_ns >= e.pv.t_ns {
            e.pv = pv;
        }
        e.is_neighbour |= neighbour;
        e.expires_ns = expires;
    }

    /// Duplicate packet detection [Annex A.2]: true if `sn` from `source` was already
    /// seen; otherwise records it and returns false.
    pub fn check_and_record(&mut self, source: GnAddr, sn: u16, now_ns: u64) -> bool {
        let expires = now_ns + self.lifetime_ns;
        let len = self.dpl_length;
        let e = self.entries.entry(source).or_insert_with(|| LocTe {
            pv: PositionVector { addr: source, t_ns: 0, x: 0.0, y: 0.0, speed: 0.0, heading_deg: 0.0 },
            is_neighbour: false,
            expires_ns: expires,
            dpl: VecDeque::new(),
        });
        if e.dpl.contains(&sn) {
            return true;
        }
        if e.dpl.len() == len {
            e.dpl.pop_front();
        }
        e.dpl.push_back(sn);
        false
    }

    /// The entry for `addr`, if it has not expired.
    pub fn get(&self, addr: GnAddr, now_ns: u64) -> Option<&LocTe> {
        self.entries.get(&addr).filter(|e| e.expires_ns > now_ns)
    }

    /// Live neighbours, in address order.
    pub fn neighbours(&self, now_ns: u64) -> impl Iterator<Item = &LocTe> {
        self.entries
            .values()
            .filter(move |e| e.is_neighbour && e.expires_ns > now_ns && e.pv.t_ns > 0)
    }

    /// Drop expired entries [§8.1.3].
    pub fn purge(&mut self, now_ns: u64) {
        self.entries.retain(|_, e| e.expires_ns > now_ns);
    }

    /// Number of entries (expired ones included until the next purge).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpl_detects_duplicates_and_forgets_after_its_length() {
        let mut t = LocationTable::new(20_000, 2);
        let s = GnAddr(7);
        assert!(!t.check_and_record(s, 1, 0));
        assert!(t.check_and_record(s, 1, 0));
        assert!(!t.check_and_record(s, 2, 0));
        assert!(!t.check_and_record(s, 3, 0));
        // 1 fell out of the two-entry list.
        assert!(!t.check_and_record(s, 1, 0));
    }

    #[test]
    fn entries_expire() {
        let mut t = LocationTable::new(1_000, 8);
        let pv = PositionVector { addr: GnAddr(1), t_ns: 1, x: 0.0, y: 0.0, speed: 0.0, heading_deg: 0.0 };
        t.update(pv, true, 0);
        assert_eq!(t.neighbours(999_999_999).count(), 1);
        assert_eq!(t.neighbours(1_000_000_000).count(), 0);
    }
}
