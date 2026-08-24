use aimux_lib::service::scheduler_service::retry_limit;

#[test]
fn clamps_retry_limit_to_supported_range() {
    assert_eq!(retry_limit(0), 1);
    assert_eq!(retry_limit(10), 10);
    assert_eq!(retry_limit(99), 20);
}
