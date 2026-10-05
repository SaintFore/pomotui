use pomotui_sync::{Beginning, Document};

#[test]
fn empty_fresh_start_is_durable_and_causally_supersedes_an_observed_reset() {
    let first = Beginning::parse(1, "ffffffff-ffff-4fff-8fff-ffffffffffff").unwrap();
    let successor = Beginning::parse(2, "00000000-0000-4000-8000-000000000001").unwrap();
    let document = Document::with_beginning(successor.clone(), &[]).unwrap();
    let restored = Document::from_json(&document.to_json().unwrap()).unwrap();
    assert_eq!(restored.beginning(), &successor);
    assert!(successor > first);
    assert_eq!(Beginning::default(), Beginning::default());
}
