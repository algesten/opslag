//

use core::ops::Add;

/// Abstraction of time.
///
/// On [`Server::new()`][crate::Server::new], the service is at time 0. Every
/// [`Time`] returned as [`Output::Timeout`][crate::Output::Timeout], is in the
/// future from that current time.
///
/// You drive the time forward in the service by passing [`Input::Timeout`][crate::Input::Timeout].
/// Every such drives time forward.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Time(u64);

impl Time {
    /// Create a new time from a millisecond offset from
    /// an imaginary time 0.
    pub fn from_millis(t: u64) -> Self {
        Self(t)
    }

    /// Check how many milliseconds there is from this `Time` to some other `Time`.
    ///
    /// If other is in the past, this returns 0.
    pub fn millis_until(&self, other: Time) -> u64 {
        other.0.saturating_sub(self.0)
    }
}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Time {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl Add<u64> for Time {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Time {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "Time({})", self.0)
    }
}
