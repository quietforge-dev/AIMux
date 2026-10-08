use aimux_lib::config::Settings;

#[test]
fn old_settings_use_monitoring_defaults() {
    let mut value = serde_json::to_value(Settings::default()).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("monitoring_interval_minutes");
    object.remove("monitoring_recent_count");
    object.insert("port".into(), serde_json::json!(7790));
    let settings: Settings = serde_json::from_value(value).unwrap();
    assert_eq!(settings.port, 7790);
    assert_eq!(settings.monitoring_interval_minutes, 2);
    assert_eq!(settings.monitoring_recent_count, 30);
}

#[test]
fn validates_monitoring_option_boundaries() {
    use aimux_lib::service::settings_service::validate;
    let mut settings = Settings::default();
    for (interval, count) in [(2, 10), (10, 40), (2, 30)] {
        settings.monitoring_interval_minutes = interval;
        settings.monitoring_recent_count = count;
        assert!(validate(&settings).is_ok());
    }
    for (interval, count) in [(1, 30), (11, 30), (2, 9), (2, 41)] {
        settings.monitoring_interval_minutes = interval;
        settings.monitoring_recent_count = count;
        assert!(validate(&settings).is_err());
    }
}

#[test]
fn database_path_always_uses_aimux_db() {
    let path = Settings::default().database_path();
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("aimux.db")
    );
    assert_eq!(path.parent(), Some(Settings::data_dir().as_path()));
}
