/// Detect common `true` / `false` states in a string payload.
#[must_use]
pub fn is_true(payload: &str) -> bool {
    match payload {
        "true" | "True" | "TRUE" | "on" | "On" | "ON" | "online" | "Online" | "ONLINE" | "1"
        | "2" => true,
        "false" | "False" | "FALSE" | "off" | "Off" | "OFF" | "offline" | "Offline" | "OFFLINE"
        | "0" => false,
        _ => {
            eprintln!("WARNING is_true unclear, assumes true: {payload:?}");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    #[rstest::rstest]
    fn is_true(#[values("on", "1", "true")] payload: &str) {
        assert!(super::is_true(payload));
    }

    #[rstest::rstest]
    fn is_false(#[values("off", "0", "false")] payload: &str) {
        assert!(!super::is_true(payload));
    }
}
