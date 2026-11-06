//! Keys: debouncing, and the rules for approving a signature.

/// Keys on the device.
pub const KEYS: usize = 4;
/// A state must hold this long to count.
pub const DEBOUNCE_MS: u64 = 20;
/// Presses are ignored for this long after a review appears.
pub const APPROVAL_GUARD_MS: u64 = 500;
/// No decision in this long rejects.
pub const APPROVAL_TIMEOUT_MS: u64 = 30_000;

/// Which keys are down.
pub type KeyState = [bool; KEYS];

/// Removes contact bounce from raw key readings.
#[derive(Default)]
pub struct Debouncer {
    stable: KeyState,
    candidate: KeyState,
    changed_at: [u64; KEYS],
}

impl Debouncer {
    /// A debouncer that starts from the keys as they are now.
    #[must_use]
    pub fn new(keys: KeyState) -> Self {
        Self {
            stable: keys,
            candidate: keys,
            changed_at: [0; KEYS],
        }
    }

    /// Takes a raw reading and returns the settled state.
    pub fn update(&mut self, now_ms: u64, raw: KeyState) -> KeyState {
        for (key, down) in raw.into_iter().enumerate() {
            if down != self.candidate[key] {
                self.candidate[key] = down;
                self.changed_at[key] = now_ms;
            }
            if now_ms.saturating_sub(self.changed_at[key]) >= DEBOUNCE_MS {
                self.stable[key] = self.candidate[key];
            }
        }
        self.stable
    }
}

/// How an approval ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Still waiting.
    Waiting,
    /// K4 was pressed.
    Approved,
    /// K1 or K2 was pressed, or time ran out.
    Rejected,
}

/// Waits for a decision on a signing screen. Only K4 approves, and K3 does nothing.
pub struct Approval {
    started_ms: u64,
    armed: KeyState,
}

impl Approval {
    /// Starts waiting. Keys already down must be released before they count.
    #[must_use]
    pub fn new(now_ms: u64, keys: KeyState) -> Self {
        Self {
            started_ms: now_ms,
            armed: keys.map(|down| !down),
        }
    }

    /// Takes the settled key state.
    pub fn update(&mut self, now_ms: u64, keys: KeyState) -> Decision {
        let elapsed = now_ms.saturating_sub(self.started_ms);
        if elapsed >= APPROVAL_TIMEOUT_MS {
            return Decision::Rejected;
        }
        let guarded = elapsed < APPROVAL_GUARD_MS;
        for (armed, down) in self.armed.iter_mut().zip(keys) {
            if !down {
                *armed = true;
            } else if guarded {
                *armed = false;
            }
        }
        if guarded {
            return Decision::Waiting;
        }
        let pressed = |key: usize| keys[key] && self.armed[key];
        if pressed(0) || pressed(1) {
            Decision::Rejected
        } else if pressed(3) {
            Decision::Approved
        } else {
            Decision::Waiting
        }
    }

    /// Time left to decide.
    #[must_use]
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        APPROVAL_TIMEOUT_MS.saturating_sub(now_ms.saturating_sub(self.started_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::{Approval, Debouncer, Decision, KeyState};

    const NONE: KeyState = [false; 4];
    const K1: KeyState = [true, false, false, false];
    const K2: KeyState = [false, true, false, false];
    const K3: KeyState = [false, false, true, false];
    const K4: KeyState = [false, false, false, true];

    #[test]
    fn a_held_state_counts_after_20_ms() {
        let mut keys = Debouncer::new(NONE);
        assert_eq!(keys.update(100, K1), NONE);
        assert_eq!(keys.update(119, K1), NONE);
        assert_eq!(keys.update(120, K1), K1);
    }

    #[test]
    fn a_bounce_restarts_the_wait() {
        let mut keys = Debouncer::new(NONE);
        keys.update(100, K1);
        keys.update(110, NONE);
        keys.update(115, K1);
        assert_eq!(keys.update(125, K1), NONE);
        assert_eq!(keys.update(135, K1), K1);
    }

    #[test]
    fn a_release_is_debounced_too() {
        let mut keys = Debouncer::new(K2);
        assert_eq!(keys.update(100, NONE), K2);
        assert_eq!(keys.update(120, NONE), NONE);
    }

    #[test]
    fn keys_settle_independently() {
        let mut keys = Debouncer::new(NONE);
        keys.update(100, K1);
        keys.update(110, [true, true, false, false]);
        assert_eq!(keys.update(120, [true, true, false, false]), K1);
        assert_eq!(
            keys.update(130, [true, true, false, false]),
            [true, true, false, false]
        );
    }

    fn past_guard() -> Approval {
        let mut approval = Approval::new(1000, NONE);
        assert_eq!(approval.update(1500, NONE), Decision::Waiting);
        approval
    }

    #[test]
    fn k4_approves_after_the_guard() {
        assert_eq!(past_guard().update(1600, K4), Decision::Approved);
    }

    #[test]
    fn k1_and_k2_reject() {
        assert_eq!(past_guard().update(1600, K1), Decision::Rejected);
        assert_eq!(past_guard().update(1600, K2), Decision::Rejected);
    }

    #[test]
    fn k3_does_nothing() {
        let mut approval = past_guard();
        assert_eq!(approval.update(1600, K3), Decision::Waiting);
        assert_eq!(approval.update(1700, NONE), Decision::Waiting);
    }

    #[test]
    fn presses_in_the_first_500_ms_are_ignored() {
        let mut approval = Approval::new(1000, NONE);
        assert_eq!(approval.update(1499, K4), Decision::Waiting);
    }

    #[test]
    fn a_press_started_in_the_guard_must_be_released_first() {
        let mut approval = Approval::new(1000, NONE);
        approval.update(1400, K4);
        assert_eq!(approval.update(1600, K4), Decision::Waiting);
        assert_eq!(approval.update(1700, NONE), Decision::Waiting);
        assert_eq!(approval.update(1800, K4), Decision::Approved);
    }

    #[test]
    fn a_key_held_from_before_must_be_released_first() {
        let mut approval = Approval::new(1000, K4);
        assert_eq!(approval.update(2000, K4), Decision::Waiting);
        assert_eq!(approval.update(2100, NONE), Decision::Waiting);
        assert_eq!(approval.update(2200, K4), Decision::Approved);
    }

    #[test]
    fn a_held_key_does_not_block_the_others() {
        let mut approval = Approval::new(1000, K4);
        assert_eq!(
            approval.update(2000, [true, false, false, true]),
            Decision::Rejected
        );
    }

    #[test]
    fn k4_with_a_reject_key_rejects() {
        assert_eq!(
            past_guard().update(1600, [true, false, false, true]),
            Decision::Rejected
        );
    }

    #[test]
    fn no_decision_in_30_seconds_rejects() {
        let mut approval = Approval::new(1000, NONE);
        assert_eq!(approval.update(30_999, NONE), Decision::Waiting);
        assert_eq!(approval.update(31_000, NONE), Decision::Rejected);
    }

    #[test]
    fn a_late_press_is_still_a_timeout() {
        let mut approval = Approval::new(1000, NONE);
        assert_eq!(approval.update(31_000, K4), Decision::Rejected);
    }

    #[test]
    fn the_countdown_runs_down() {
        let approval = Approval::new(1000, NONE);
        assert_eq!(approval.remaining_ms(1000), 30_000);
        assert_eq!(approval.remaining_ms(11_000), 20_000);
        assert_eq!(approval.remaining_ms(99_000), 0);
    }
}
