//! The one exception to locking in the background (ADR-0017): on a phone the system's file
//! picker covers the app, so the trip to it that the user just asked for must not lock the
//! vault. It is good for a single departure and a short time.

pub const PICKER_TIME_LIMIT_MS: u64 = 2 * 60 * 1000;

/// Both clocks are watched: the monotonic one may stand still while a phone sleeps, and the
/// wall clock may be changed by the user.
#[derive(Clone, Copy)]
pub struct Now {
    pub monotonic_ms: u64,
    pub unix_ms: i64,
}

#[derive(Default)]
pub struct PickerExcuse(Option<Excuse>);

struct Excuse {
    granted_at: Now,
    away: bool,
}

impl Excuse {
    fn expired(&self, now: Now) -> bool {
        let monotonic = now
            .monotonic_ms
            .saturating_sub(self.granted_at.monotonic_ms);
        let wall = now.unix_ms.saturating_sub(self.granted_at.unix_ms);
        monotonic > PICKER_TIME_LIMIT_MS || wall.unsigned_abs() > PICKER_TIME_LIMIT_MS
    }
}

impl PickerExcuse {
    pub fn grant(&mut self, now: Now) {
        self.0 = Some(Excuse {
            granted_at: now,
            away: false,
        });
    }

    /// Whether the app leaving the screen right now is the excused trip.
    pub fn covers_leaving(&mut self, now: Now) -> bool {
        match &mut self.0 {
            Some(excuse) if !excuse.away && !excuse.expired(now) => {
                excuse.away = true;
                true
            }
            _ => {
                self.0 = None;
                false
            }
        }
    }

    /// Whether the app has stayed away longer than the excuse allows.
    pub fn is_overstayed(&self, now: Now) -> bool {
        self.0
            .as_ref()
            .is_some_and(|excuse| excuse.away && excuse.expired(now))
    }

    pub fn clear(&mut self) {
        self.0 = None;
    }
}
