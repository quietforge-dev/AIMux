pub fn retry_limit(configured: u32) -> u32 {
    configured.clamp(1, 20)
}
