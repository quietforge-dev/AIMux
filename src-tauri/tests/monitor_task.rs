use std::cmp::Ordering;

use aimux_lib::background::monitor_task::{
    candidate_order, should_promote, successful_accounts_by_probe_model,
};

#[test]
fn chooses_the_lowest_multiplier_then_the_fastest_average_duration() {
    assert_eq!(
        candidate_order(0.04, Some(900), 0.10, Some(100)),
        Ordering::Less
    );
    assert_eq!(
        candidate_order(0.10, Some(500), 0.10, Some(800)),
        Ordering::Less
    );
    assert_eq!(candidate_order(0.10, Some(500), 0.10, None), Ordering::Less);
    assert_eq!(
        candidate_order(0.10, None, 0.10, Some(500)),
        Ordering::Greater
    );
}

#[test]
fn promotes_a_successful_lower_multiplier_account() {
    assert!(should_promote(0.04, None, 0.10, Some(500)));
}

#[test]
fn promotes_a_faster_account_when_multipliers_match() {
    assert!(should_promote(0.10, Some(500), 0.10, Some(800)));
    assert!(should_promote(0.10, Some(500), 0.10, None));
}

#[test]
fn does_not_promote_an_unknown_or_slower_account_when_multipliers_match() {
    assert!(!should_promote(0.10, None, 0.10, Some(800)));
    assert!(!should_promote(0.10, None, 0.10, None));
    assert!(!should_promote(0.10, Some(800), 0.10, Some(500)));
}

#[test]
fn groups_successful_accounts_by_protocol_and_test_model() {
    let groups = successful_accounts_by_probe_model(&[
        (
            "luna-account".into(),
            "openai".into(),
            true,
            "gpt-5.6-luna".into(),
        ),
        (
            "terra-account".into(),
            "openai".into(),
            true,
            "gpt-5.6-terra".into(),
        ),
        (
            "failed-terra".into(),
            "openai".into(),
            false,
            "gpt-5.6-terra".into(),
        ),
        (
            "anthropic-account".into(),
            "anthropic".into(),
            true,
            "claude-sonnet-4-8".into(),
        ),
    ]);

    assert_eq!(groups.len(), 3);
    assert_eq!(
        groups
            .get(&("openai".into(), "gpt-5.6-luna".into()))
            .expect("应存在 Luna 分组"),
        &std::collections::BTreeSet::from(["luna-account".into()])
    );
    assert_eq!(
        groups
            .get(&("openai".into(), "gpt-5.6-terra".into()))
            .expect("应存在 Terra 分组"),
        &std::collections::BTreeSet::from(["terra-account".into()])
    );
}
