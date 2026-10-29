use std::process::{Command, Output};

fn cli(args: &[&str], timezone: Option<&str>) -> Output {
    let config = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_kito"));
    command
        .arg("time")
        .args(args)
        .env_remove("KITO_TIMEZONE")
        .env("KITO_CONFIG_DIR", config.path());
    if let Some(timezone) = timezone {
        command.env("KITO_TIMEZONE", timezone);
    }
    command.output().unwrap()
}

fn success(args: &[&str]) -> String {
    let output = cli(args, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn json(args: &[&str]) -> serde_json::Value {
    serde_json::from_str(&success(args)).unwrap()
}

#[test]
fn timestamps_preserve_all_four_precisions_in_both_directions() {
    for (unit, value, datetime) in [
        ("s", "1704067200", "2024-01-01T00:00:00Z"),
        ("ms", "1704067200123", "2024-01-01T00:00:00.123Z"),
        ("us", "1704067200123456", "2024-01-01T00:00:00.123456Z"),
        (
            "ns",
            "1704067200123456789",
            "2024-01-01T00:00:00.123456789Z",
        ),
    ] {
        assert_eq!(
            success(&["from-timestamp", value, "--unit", unit, "--value-only"]),
            format!("{datetime}\n")
        );
        assert_eq!(
            success(&["to-timestamp", datetime, "--unit", unit, "--value-only"]),
            format!("{value}\n")
        );
        let report = json(&["from-timestamp", value, "--json"]);
        assert_eq!(report["unit"], unit);
        assert_eq!(report["value"], datetime);
        assert_eq!(report["timestamps"][unit], value);
    }
}

#[test]
fn negative_fractional_timestamps_use_euclidean_remainders() {
    for (unit, datetime) in [
        ("s", "1969-12-31T23:59:59Z"),
        ("ms", "1969-12-31T23:59:59.999Z"),
        ("us", "1969-12-31T23:59:59.999999Z"),
        ("ns", "1969-12-31T23:59:59.999999999Z"),
    ] {
        assert_eq!(
            success(&["from-timestamp", "-1", "--unit", unit, "--value-only"]),
            format!("{datetime}\n")
        );
        assert_eq!(
            success(&["to-timestamp", datetime, "--unit", unit, "--value-only"]),
            "-1\n"
        );
    }
}

#[test]
fn short_and_ambiguous_timestamps_have_an_explicit_unit_escape_hatch() {
    assert_eq!(
        success(&["from-timestamp", "0", "--value-only"]),
        "1970-01-01T00:00:00Z\n"
    );
    assert_eq!(
        success(&[
            "from-timestamp",
            "1",
            "--unit",
            "microseconds",
            "--value-only"
        ]),
        "1970-01-01T00:00:00.000001Z\n"
    );
    let failure = cli(&["from-timestamp", "12345678901"], None);
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("--unit"));
    assert_eq!(
        success(&[
            "from-timestamp",
            "12345678901",
            "--unit",
            "ms",
            "--value-only"
        ]),
        "1970-05-23T21:21:18.901Z\n"
    );
}

#[test]
fn named_and_fractional_offset_timezones_convert_the_same_instant() {
    assert_eq!(
        success(&[
            "from-timestamp",
            "1704067200",
            "--timezone",
            "Asia/Shanghai",
            "--value-only"
        ]),
        "2024-01-01T08:00:00+08:00\n"
    );
    assert_eq!(
        success(&[
            "to-timestamp",
            "2024-01-01 08:00:00.123456",
            "--timezone",
            "Asia/Shanghai",
            "--unit",
            "us",
            "--value-only"
        ]),
        "1704067200123456\n"
    );
    assert_eq!(
        success(&[
            "convert",
            "2024-01-01 08:00:00",
            "--from-timezone",
            "Asia/Shanghai",
            "--to-timezone",
            "America/New_York",
            "--value-only"
        ]),
        "2023-12-31T19:00:00-05:00\n"
    );
    assert_eq!(
        success(&[
            "convert",
            "2024-01-01T00:00:00Z",
            "--to-timezone",
            "+05:45",
            "--value-only"
        ]),
        "2024-01-01T05:45:00+05:45\n"
    );
    assert_eq!(
        success(&[
            "from-timestamp",
            "0",
            "--timezone",
            "-03:30",
            "--value-only"
        ]),
        "1969-12-31T20:30:00-03:30\n"
    );
}

#[test]
fn environment_default_yields_to_explicit_zone_and_embedded_offset() {
    for (args, environment, expected) in [
        (
            vec!["from-timestamp", "0", "--value-only"],
            "Asia/Shanghai",
            "1970-01-01T08:00:00+08:00\n",
        ),
        (
            vec!["from-timestamp", "0", "--timezone", "UTC", "--value-only"],
            "invalid",
            "1970-01-01T00:00:00Z\n",
        ),
        (
            vec!["to-timestamp", "1970-01-01 08:00:00", "--value-only"],
            "Asia/Shanghai",
            "0\n",
        ),
        (
            vec!["to-timestamp", "1970-01-01T00:00:00Z", "--value-only"],
            "invalid",
            "0\n",
        ),
        (
            vec![
                "convert",
                "1970-01-01 08:00:00",
                "--to-timezone",
                "UTC",
                "--value-only",
            ],
            "Asia/Shanghai",
            "1970-01-01T00:00:00Z\n",
        ),
    ] {
        let output = cli(&args, Some(environment));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
    assert!(
        !cli(&["from-timestamp", "0"], Some("invalid"))
            .status
            .success()
    );
}

#[test]
fn dst_gaps_and_overlaps_require_an_unambiguous_instant() {
    for input in ["2024-03-10 02:30:00", "2024-11-03 01:30:00"] {
        let failure = cli(
            &["to-timestamp", input, "--timezone", "America/New_York"],
            None,
        );
        assert!(!failure.status.success());
        assert!(failure.stdout.is_empty());
        let message = String::from_utf8_lossy(&failure.stderr);
        assert!(
            message.contains("不存在") || message.contains("歧义"),
            "{message}"
        );
    }
    for (input, expected) in [
        ("2024-11-03T01:30:00-04:00", "2024-11-03T05:30:00Z\n"),
        ("2024-11-03T01:30:00-05:00", "2024-11-03T06:30:00Z\n"),
    ] {
        assert_eq!(
            success(&[
                "convert",
                input,
                "--from-timezone",
                "America/New_York",
                "--to-timezone",
                "UTC",
                "--value-only"
            ]),
            expected
        );
    }
    assert!(
        !cli(
            &[
                "convert",
                "2024-01-01T00:00:00+08:00",
                "--from-timezone",
                "America/New_York",
                "--to-timezone",
                "UTC"
            ],
            None
        )
        .status
        .success()
    );
}

#[test]
fn json_preserves_large_integers_and_reports_precision_loss() {
    let report = json(&[
        "to-timestamp",
        "9999-12-31T23:59:59.999999999Z",
        "--unit",
        "ns",
        "--json",
    ]);
    assert_eq!(report["value"], "253402300799999999999");
    assert_eq!(report["timestamps"]["ns"], "253402300799999999999");
    assert_eq!(report["precision_lost"], false);
    assert_eq!(
        success(&[
            "from-timestamp",
            "253402300799999999999",
            "--unit",
            "ns",
            "--value-only"
        ]),
        "9999-12-31T23:59:59.999999999Z\n"
    );
    let report = json(&[
        "to-timestamp",
        "1969-12-31T23:59:59.999999999Z",
        "--unit",
        "ms",
        "--json",
    ]);
    assert_eq!(report["value"], "-1");
    assert_eq!(report["precision_lost"], true);
}

#[test]
fn malformed_or_out_of_range_values_fail_without_stdout() {
    for args in [
        vec!["from-timestamp", "1.5"],
        vec!["from-timestamp", ""],
        vec![
            "from-timestamp",
            "99999999999999999999999999999999999999999999999",
            "--unit",
            "ns",
        ],
        vec!["from-timestamp", "9223372036854775807", "--unit", "s"],
        vec!["from-timestamp", "0", "--timezone", "+25:00"],
        vec!["from-timestamp", "0", "--timezone", "Asia/Unknown"],
        vec!["to-timestamp", "2024-02-30 00:00:00"],
        vec!["to-timestamp", "2024-01-01T00:00:00.1234567891Z"],
        vec!["to-timestamp", "2016-12-31T23:59:60Z"],
        vec!["from-timestamp", "0", "--json", "--value-only"],
        vec!["to-timestamp", "1970-01-01T00:00:00Z", "--unit", "auto"],
    ] {
        let output = cli(&args, None);
        assert!(!output.status.success(), "unexpected success: {args:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn historical_second_offsets_are_not_rounded_away() {
    let report = json(&[
        "convert",
        "1900-01-01T00:00:00Z",
        "--to-timezone",
        "Asia/Shanghai",
        "--json",
    ]);
    assert_eq!(report["datetime"], "1900-01-01T08:05:43+08:05:43");
    assert_eq!(report["utc_offset"], "+08:05:43");
    assert_eq!(
        success(&[
            "convert",
            "1900-01-01T08:05:43+08:05:43",
            "--to-timezone",
            "UTC",
            "--value-only"
        ]),
        "1900-01-01T00:00:00Z\n"
    );
}

#[test]
fn now_supports_single_datetime_or_selected_timestamp() {
    let report = json(&["now", "--timezone", "Asia/Shanghai", "--json"]);
    assert_eq!(report["timezone"], "Asia/Shanghai");
    assert_eq!(report["value"], report["datetime"]);
    assert!(report["datetime"].as_str().unwrap().ends_with("+08:00"));
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let value: u128 = success(&["now", "--unit", "ms", "--value-only"])
        .trim()
        .parse()
        .unwrap();
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    assert!((before..=after).contains(&value));
}

#[test]
fn zones_list_is_global_sorted_and_has_a_reference_instant() {
    let report = json(&["zones", "--at", "2024-01-01T00:00:00Z", "--json"]);
    assert_eq!(report["reference_utc"], "2024-01-01T00:00:00Z");
    assert!(!report["tzdb_version"].as_str().unwrap().is_empty());
    let rows = report["zones"].as_array().unwrap();
    assert!(rows.len() > 500);
    let names: Vec<_> = rows
        .iter()
        .map(|row| row["name"].as_str().unwrap())
        .collect();
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    for name in [
        "Africa/Cairo",
        "America/New_York",
        "Antarctica/Casey",
        "Asia/Shanghai",
        "Australia/Sydney",
        "Europe/London",
        "Pacific/Auckland",
        "UTC",
    ] {
        assert!(names.contains(&name), "missing {name}");
    }
}

#[test]
fn zones_filter_supports_copyable_names_and_empty_results() {
    assert_eq!(
        success(&["zones", "sHaNgHaI", "--value-only"]),
        "Asia/Shanghai\n"
    );
    assert_eq!(success(&["zones", "no-such-zone", "--value-only"]), "");
    assert_eq!(
        json(&["zones", "no-such-zone", "--json"])["zones"],
        serde_json::json!([])
    );
    let report = json(&["zones", "+05:45", "--at", "2024-01-01T00:00:00Z", "--json"]);
    assert!(
        report["zones"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["name"] == "Asia/Kathmandu")
    );
    let output = cli(&["zones", "Shanghai", "--value-only"], Some("invalid"));
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "Asia/Shanghai\n");
}

#[test]
fn zones_offsets_follow_dst_at_the_reference_instant() {
    for (at, offset, abbreviation) in [
        ("2024-01-01T00:00:00Z", "-05:00", "EST"),
        ("2024-07-01T00:00:00Z", "-04:00", "EDT"),
    ] {
        let report = json(&["zones", "America/New_York", "--at", at, "--json"]);
        assert_eq!(report["zones"].as_array().unwrap().len(), 1);
        assert_eq!(report["zones"][0]["utc_offset"], offset);
        assert_eq!(report["zones"][0]["abbreviation"], abbreviation);
    }
}

#[test]
fn zones_rejects_invalid_or_unzoned_reference_dates() {
    for at in ["not-a-date", "2024-01-01 00:00:00", "2016-12-31T23:59:60Z"] {
        let output = cli(&["zones", "--at", at], None);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn malformed_date_boundaries_cannot_be_reinterpreted_as_another_day() {
    for input in [
        "2024-01-112:00:00",
        "2024-1-123:00:00",
        "2024-01-0100:00:00",
        "2024-1-1 0:0:0",
        "2024-01-01  00:00:00",
        "2024-01-01T00:00:00.Z",
        "日期不是有效时间",
    ] {
        let output = cli(
            &["convert", input, "--to-timezone", "UTC", "--value-only"],
            None,
        );
        assert!(!output.status.success(), "invalid date accepted: {input}");
        assert!(output.stdout.is_empty());
    }
    for input in [
        "2024-01-01t00:00:00z",
        "2024-01-01T00:00:00",
        "2024-01-01 00:00:00",
    ] {
        assert_eq!(
            success(&["convert", input, "--to-timezone", "UTC", "--value-only"]),
            "2024-01-01T00:00:00Z\n"
        );
    }
}

#[test]
fn custom_formats_support_spaces_optional_offsets_and_literal_text() {
    for (format, expected) in [
        (
            "%Y-%m-%d %H:%M:%S%.6f%:z",
            "2024-01-01 08:00:00.123456+08:00\n",
        ),
        ("%Y-%m-%d %H:%M:%S%.6f", "2024-01-01 08:00:00.123456\n"),
        ("%Y/%m/%d %H:%M:%S", "2024/01/01 08:00:00\n"),
        ("%Y年%m月%d日 %H:%M:%S %%", "2024年01月01日 08:00:00 %\n"),
    ] {
        assert_eq!(
            success(&[
                "from-timestamp",
                "1704067200123456",
                "--timezone",
                "Asia/Shanghai",
                "--format",
                format,
                "--value-only"
            ]),
            expected
        );
        assert_eq!(
            success(&[
                "convert",
                "2024-01-01T00:00:00.123456Z",
                "--to-timezone",
                "Asia/Shanghai",
                "-f",
                format,
                "--value-only"
            ]),
            expected
        );
    }
}

#[test]
fn custom_fractional_precision_truncates_or_pads_display_without_float_rounding() {
    for (value, format, expected) in [
        ("1704067200123456789", "%T%.f", "00:00:00.123456789\n"),
        ("1704067200123456789", "%T%.3f", "00:00:00.123\n"),
        ("1704067200123456789", "%T%.6f", "00:00:00.123456\n"),
        ("1704067200123456789", "%T%.9f", "00:00:00.123456789\n"),
        ("1704067200000000000", "%T%.6f", "00:00:00.000000\n"),
        (
            "-1",
            "%F %T%.9f%:z",
            "1969-12-31 23:59:59.999999999+00:00\n",
        ),
    ] {
        assert_eq!(
            success(&[
                "from-timestamp",
                value,
                "--unit",
                "ns",
                "--format",
                format,
                "--value-only"
            ]),
            expected
        );
    }
}

#[test]
fn formatted_json_keeps_the_full_utc_instant_and_timestamp_fields() {
    let format = "%F %T%.3f";
    let report = json(&[
        "from-timestamp",
        "1704067200123456789",
        "--timezone",
        "Asia/Shanghai",
        "--format",
        format,
        "--json",
    ]);
    assert_eq!(report["value"], "2024-01-01 08:00:00.123");
    assert_eq!(report["datetime"], report["value"]);
    assert_eq!(report["format"], format);
    assert_eq!(report["utc"], "2024-01-01T00:00:00.123456789Z");
    assert_eq!(report["timestamps"]["ns"], "1704067200123456789");
    assert_eq!(report["utc_offset"], "+08:00");
    assert_eq!(report["timezone"], "Asia/Shanghai");
    assert_eq!(report["precision_lost"], false);
    let default = json(&["from-timestamp", "1704067200123456789", "--json"]);
    assert!(default.get("format").is_none());
}

#[test]
fn formatting_dates_does_not_change_primary_timestamp_results() {
    let report = json(&[
        "to-timestamp",
        "2024-01-01T08:00:00.123456+08:00",
        "--unit",
        "us",
        "--format",
        "%F %T",
        "--json",
    ]);
    assert_eq!(report["value"], "1704067200123456");
    assert_eq!(report["datetime"], "2024-01-01 08:00:00");
    assert_eq!(report["utc"], "2024-01-01T00:00:00.123456Z");
    assert_eq!(report["precision_lost"], false);
    assert_eq!(
        success(&[
            "now",
            "--timezone",
            "+05:45",
            "--format",
            "%:z",
            "--value-only"
        ]),
        "+05:45\n"
    );
    let now = json(&[
        "now",
        "--unit",
        "ms",
        "--timezone",
        "+05:45",
        "--format",
        "%:z",
        "--json",
    ]);
    assert_eq!(now["datetime"], "+05:45");
    assert_eq!(now["value"], now["timestamps"]["ms"]);
}

#[test]
fn custom_formats_use_dst_abbreviations_and_can_keep_historical_offset_seconds() {
    for (input, expected) in [
        ("2024-01-01T00:00:00Z", "2023-12-31 19:00:00 EST -05:00\n"),
        ("2024-07-01T00:00:00Z", "2024-06-30 20:00:00 EDT -04:00\n"),
    ] {
        assert_eq!(
            success(&[
                "convert",
                input,
                "--to-timezone",
                "America/New_York",
                "--format",
                "%F %T %Z %:z",
                "--value-only"
            ]),
            expected
        );
    }
    assert_eq!(
        success(&[
            "convert",
            "1900-01-01T00:00:00Z",
            "--to-timezone",
            "Asia/Shanghai",
            "--format",
            "%F %T %::z",
            "--value-only"
        ]),
        "1900-01-01 08:05:43 +08:05:43\n"
    );
}

#[test]
fn invalid_output_formats_return_errors_without_partial_stdout_or_panics() {
    for format in ["", "  ", "%", "%Q", "%.5f", "%#z"] {
        let output = cli(
            &["from-timestamp", "0", "--format", format, "--value-only"],
            None,
        );
        assert!(
            !output.status.success(),
            "invalid format accepted: {format:?}"
        );
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("format"), "{error}");
        assert!(!error.contains("panicked"), "{error}");
    }
}

#[test]
fn output_format_does_not_change_input_parsing_or_date_range_validation() {
    for args in [
        vec![
            "to-timestamp",
            "01/01/2024 08:00:00",
            "--format",
            "%d/%m/%Y %T",
        ],
        vec![
            "convert",
            "9999-12-31T23:59:59Z",
            "--to-timezone",
            "+01:00",
            "--format",
            "%Y",
        ],
    ] {
        let output = cli(&args, None);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}
