mod common;
use std::fs;
#[test]
fn benchmark_records_verified_baselines_and_real_totals() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.png");
    let report = dir.path().join("report.json");
    let bytes = common::rgba();
    fs::write(&input, &bytes).unwrap();
    let result = common::cli(&[
        "image",
        "benchmark",
        common::path(&input),
        "--output",
        common::path(&report),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(value["totals"]["input_bytes"], bytes.len());
    assert!(value["totals"]["kito_bytes"].as_u64().unwrap() <= bytes.len() as u64);
    assert!(
        value["files"][0]["baselines"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["valid"] == true)
            .count()
            >= 2
    );
    assert_eq!(fs::read(&input).unwrap(), bytes);
}
#[test]
fn zero_budget_does_not_claim_complete_search() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.png");
    let report = dir.path().join("report.json");
    fs::write(&input, common::rgba()).unwrap();
    let result = common::cli(&[
        "image",
        "benchmark",
        common::path(&input),
        "--time-limit",
        "0",
        "--output",
        common::path(&report),
    ]);
    assert!(result.status.success());
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(value["files"][0]["kito"]["search_complete"], false);
    assert!(
        value["files"][0]["baselines"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["valid"] == false)
    );
}

#[test]
fn benchmark_report_cannot_overwrite_an_input_in_a_directory() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.png");
    let bytes = common::rgba();
    fs::write(&input, &bytes).unwrap();
    let result = common::cli(&[
        "image",
        "benchmark",
        common::path(dir.path()),
        "--output",
        common::path(&input),
        "--overwrite",
    ]);
    assert!(!result.status.success());
    assert_eq!(fs::read(&input).unwrap(), bytes);
}
