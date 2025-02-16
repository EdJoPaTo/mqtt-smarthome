use core::time::Duration;
use std::time::SystemTime;

use crate::payload;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    time: SystemTime,
    payload: Box<str>,
}

impl HistoryEntry {
    #[must_use]
    pub fn new<I>(payload: I) -> Self
    where
        I: Into<Box<str>>,
    {
        Self {
            time: SystemTime::now(),
            payload: payload.into(),
        }
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

    #[rstest::rstest]
    fn payload_stays_payload(#[values("42", "666")] payload: &str) {
        assert_eq!(HistoryEntry::new(payload.to_owned()).payload(), payload);
    }

    #[test]
    fn ago_works() {
        let entry = HistoryEntry::new("42".to_owned());
        std::thread::sleep(Duration::from_millis(25));
        let ago = entry.ago().as_millis();
        println!("ago: {ago} ms");
        assert!(ago > 20);
        assert!(ago < 300);
    }

    #[test]
    fn payload_as_boolean() {
        assert!(HistoryEntry::new("true".to_owned()).as_boolean());
        assert!(!HistoryEntry::new("false".to_owned()).as_boolean());
    }

    #[test]
    fn payload_as_float() {
        let actual = HistoryEntry::new("12.3 °C".to_owned()).as_float().unwrap();
        float_eq::assert_float_eq!(actual, 12.3, abs <= 0.01);
    }
}
