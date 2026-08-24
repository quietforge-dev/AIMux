use aimux_lib::upstream::error::response_body;

#[test]
fn truncates_large_upstream_response_bodies() {
    let body = vec![b'a'; 4097];

    assert_eq!(response_body(&body), "a".repeat(4096));
}
