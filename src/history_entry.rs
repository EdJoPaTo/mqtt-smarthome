use core::time::Duration;
use std::time::SystemTime;

use crate::payload;

#[derive(Debug, Clone)]
#[must_use]
pub struct HistoryEntry {
    time: SystemTime,
    payload: Box<str>,
}

impl HistoryEntry {
    pub(crate) fn new<P: Into<Box<str>>>(time: SystemTime, payload: P) -> Self {
        let payload = payload.into();
        Self { time, payload }
    }

    #[must_use]
    pub fn ago(&self) -> Duration {
        self.time.elapsed().unwrap_or_default()
    }

    #[must_use]
    pub fn as_boolean(&self) -> bool {
        payload::is_true(&self.payload)
    }

    #[must_use]
    pub fn as_float(&self) -> Option<f32> {
        payload::as_f32(&self.payload)
    }

    #[must_use]
    pub const fn payload(&self) -> &str {
        &self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn entry<P: Into<Box<str>>>(payload: P) -> HistoryEntry {
        HistoryEntry {
            time: SystemTime::now(),
            payload: payload.into(),
        }
    }

    #[rstest::rstest]
    fn payload_stays_payload(#[values("42", "666")] payload: &str) {
        assert_eq!(entry(payload).payload(), payload);
    }

    #[test]
    fn ago_works() {
        let entry = entry("42");
        std::thread::sleep(Duration::from_millis(25));
        let ago = entry.ago().as_millis();
        println!("ago: {ago} ms");
        assert!(ago > 20);
        assert!(ago < 300);
    }

    #[test]
    fn payload_as_boolean() {
        assert!(entry("true").as_boolean());
        assert!(!entry("false").as_boolean());
    }

    #[test]
    fn payload_as_float() {
        let actual = entry("12.3 °C").as_float().unwrap();
        float_eq::assert_float_eq!(actual, 12.3, abs <= 0.01);
    }
}
