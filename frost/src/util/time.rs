use std::fmt;
use std::{io, time::Duration};

use crate::std_msgs::std_msgs::Time;

use super::parsing;

pub const MIN: Time = Time { secs: 0, nsecs: 1 };
pub const MAX: Time = Time {
    secs: u32::MAX,
    nsecs: 999999999,
};
pub const ZERO: Time = Time { secs: 0, nsecs: 0 };

pub const NS_TO_S: f32 = 1e-9;

impl From<Time> for Duration {
    fn from(time: Time) -> Self {
        Duration::from_secs(time.secs as u64) + Duration::from_nanos(time.nsecs as u64)
    }
}

impl From<&Time> for Duration {
    fn from(time: &Time) -> Self {
        Duration::from(*time)
    }
}

impl From<Time> for f32 {
    fn from(time: Time) -> Self {
        time.secs as f32 + (time.nsecs as f32 * NS_TO_S)
    }
}

impl From<&Time> for f32 {
    fn from(time: &Time) -> Self {
        f32::from(*time)
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", f32::from(self))
    }
}

impl Time {
    fn new(secs: u32, nsecs: u32) -> Time {
        Time { secs, nsecs }
    }
    pub fn from(buf: &[u8]) -> io::Result<Time> {
        let secs = parsing::parse_le_u32(buf)?;
        let nsecs = parsing::parse_le_u32_at(buf, 4)?;
        Ok(Time { secs, nsecs })
    }
    pub fn dur(&self, other: &Time) -> Duration {
        Duration::from(self) - Duration::from(other)
    }
}

impl Ord for Time {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        Duration::from(self).cmp(&Duration::from(other))
    }
}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Time {
    fn eq(&self, other: &Self) -> bool {
        self.secs == other.secs && self.nsecs == other.nsecs
    }
}
