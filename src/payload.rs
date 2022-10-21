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

#[test]
fn is_true_off() {
    assert!(!is_true("off"));
}

#[test]
fn is_true_on() {
    assert!(is_true("on"));
}

#[test]
fn is_true_zero() {
    assert!(!is_true("0"));
}

#[test]
fn is_true_one() {
    assert!(is_true("1"));
}

#[test]
fn is_true_false() {
    assert!(!is_true("false"));
}

#[test]
fn is_true_true() {
    assert!(is_true("true"));
}
