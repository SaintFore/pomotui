use std::time::{Duration, Instant};

pub fn wait_for_focus_review(pending: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(75);
    while !pending() {
        assert!(
            Instant::now() < deadline,
            "Focus Session did not reach its deadline"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}
