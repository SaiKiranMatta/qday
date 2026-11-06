//! The last signatures and rejections, kept in memory.

use crate::frame::{Action, Address};

/// Entries kept.
pub const ACTIVITY_ENTRIES: usize = 20;

/// What a transaction did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Summary {
    /// A transfer.
    Transfer {
        /// The recipient.
        to: Address,
        /// The amount in wei.
        value: u128,
    },
    /// A rotation.
    Rotation,
    /// Anything else.
    Other,
}

impl Summary {
    /// Summarises a transaction's action.
    #[must_use]
    pub fn of(action: &Action) -> Self {
        match action {
            Action::Transfer { to, value } => Self::Transfer {
                to: *to,
                value: *value,
            },
            Action::Rotate { .. } => Self::Rotation,
            Action::Other { .. } => Self::Other,
        }
    }
}

/// One thing that happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// True if it was signed, false if it was rejected.
    pub signed: bool,
    /// The key involved.
    pub index: u32,
    /// What the transaction did.
    pub summary: Summary,
}

/// The newest entries.
#[derive(Default)]
pub struct Activity {
    entries: [Option<Entry>; ACTIVITY_ENTRIES],
    next: usize,
}

impl Activity {
    /// Adds an entry, dropping the oldest if full.
    pub fn push(&mut self, entry: Entry) {
        self.entries[self.next] = Some(entry);
        self.next = (self.next + 1) % ACTIVITY_ENTRIES;
    }

    /// Entries held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    /// Whether nothing has happened yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entry at `position`, where 0 is the newest.
    #[must_use]
    pub fn get(&self, position: usize) -> Option<Entry> {
        if position >= ACTIVITY_ENTRIES {
            return None;
        }
        self.entries[(self.next + ACTIVITY_ENTRIES - 1 - position) % ACTIVITY_ENTRIES]
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTIVITY_ENTRIES, Activity, Entry, Summary};

    fn entry(index: u32) -> Entry {
        Entry {
            signed: true,
            index,
            summary: Summary::Rotation,
        }
    }

    #[test]
    fn it_starts_empty() {
        let activity = Activity::default();
        assert!(activity.is_empty());
        assert_eq!(activity.get(0), None);
    }

    #[test]
    fn the_newest_comes_first() {
        let mut activity = Activity::default();
        activity.push(entry(1));
        activity.push(entry(2));
        assert_eq!(activity.len(), 2);
        assert_eq!(activity.get(0), Some(entry(2)));
        assert_eq!(activity.get(1), Some(entry(1)));
        assert_eq!(activity.get(2), None);
    }

    #[test]
    fn only_the_last_20_are_kept() {
        let mut activity = Activity::default();
        for index in 0..25 {
            activity.push(entry(index));
        }
        assert_eq!(activity.len(), ACTIVITY_ENTRIES);
        assert_eq!(activity.get(0), Some(entry(24)));
        assert_eq!(activity.get(19), Some(entry(5)));
        assert_eq!(activity.get(20), None);
    }
}
