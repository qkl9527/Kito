//! 对所有引擎使用相同的无损验证器；失败、缺失和超时不作为有效基线。
use super::{
    optimize::{self, OptimizeConfig, Trial},
    png,
};
use crate::{backend, files};
use anyhow::{Result, bail, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub fn validate_report_path(input: &Path, report: &Path) -> Result<()> {
    let scratch = tempfile::tempdir()?;
    for job in files::plan(input, None, Some(scratch.path()), true, false, false)? {
        ensure!(
            !files::same(&job.input, report),
            "报告路径与基准输入重合，拒绝覆盖：{}",
            report.display()
        );
    }
    Ok(())
}
pub fn run(input: &Path, config: &OptimizeConfig) -> Result<serde_json::Value> {
    let scratch = tempfile::tempdir()?;
    let jobs = files::plan(input, None, Some(scratch.path()), true, false, false)?;
    let mut rows = Vec::new();
    let mut input_total = 0usize;
    let mut output_total = 0usize;
    let mut best_total = 0usize;
    let mut compared_kito_bytes = 0usize;
    let mut compared = 0usize;
    let mut wins = 0usize;
    let mut ties = 0usize;
    let mut losses = 0usize;
    let mut failed = 0usize;
    let mut all_baselines_valid = true;
    let mut elapsed = Vec::new();
    for job in jobs {
        let result = (|| -> Result<serde_json::Value> {
            let bytes = files::read_bounded(&job.input, config.memory_bytes)?;
            png::PngImage::decode(&bytes, config.memory_bytes)?;
            let source = if config.strip {
                png::strip_metadata(&bytes)?
            } else {
                bytes.clone()
            };
            let mut baselines = Vec::new();
            for strategy in 0..3 {
                let name = ["oxipng-o6", "oxipng-o6-zopfli", "zopflipng-m"][strategy];
                let start = Instant::now();
                let result = if config.time_limit == Some(Duration::ZERO) {
                    bail_candidate()
                } else if strategy == 2 {
                    optimize::zopfli_candidate(&source, config)
                } else {
                    optimize::oxi_candidate(&source, config, 6, strategy == 1)
                };
                let mut trial = Trial {
                    engine: name.into(),
                    bytes: None,
                    elapsed_ms: 0,
                    valid: false,
                    skipped: false,
                    error: None,
                };
                match result.and_then(|data| {
                    trial.bytes = Some(data.len());
                    png::equivalent(&bytes, &data, config.strip, config.memory_bytes)?;
                    Ok(())
                }) {
                    Ok(()) => {
                        trial.valid = true;
                    }
                    Err(e) => {
                        trial.skipped = backend::is_optional_backend_missing(&e);
                        trial.error = Some(format!("{e:#}"));
                    }
                }
                if config
                    .time_limit
                    .is_some_and(|limit| start.elapsed() >= limit)
                {
                    trial.valid = false;
                    trial.error = Some("预算耗尽，不能视为完整基线".into());
                }
                trial.elapsed_ms = start.elapsed().as_millis();
                baselines.push(trial);
            }
            let begin = Instant::now();
            let result = optimize::optimize(&bytes, config)?;
            let elapsed_ms = begin.elapsed().as_millis();
            elapsed.push(elapsed_ms);
            input_total += bytes.len();
            output_total += result.bytes.len();
            let minimum = baselines
                .iter()
                .filter(|t| t.valid)
                .filter_map(|t| t.bytes)
                .min();
            all_baselines_valid &= baselines.iter().all(|t| t.valid);
            if let Some(minimum) = minimum {
                let minimum = minimum.min(bytes.len());
                best_total += minimum;
                compared_kito_bytes += result.bytes.len();
                compared += 1;
                match result.bytes.len().cmp(&minimum) {
                    std::cmp::Ordering::Less => wins += 1,
                    std::cmp::Ordering::Equal => ties += 1,
                    std::cmp::Ordering::Greater => losses += 1,
                };
            }
            Ok(
                serde_json::json!({"input":job.input,"sha256":format!("{:x}",Sha256::digest(&bytes)),"input_bytes":bytes.len(),"output_bytes":result.bytes.len(),"baselines":baselines,"kito":result,"kito_elapsed_ms":elapsed_ms,"peak_rss_bytes":null}),
            )
        })();
        match result {
            Ok(row) => rows.push(row),
            Err(e) => {
                failed += 1;
                all_baselines_valid = false;
                rows.push(serde_json::json!({"input":job.input,"error":format!("{e:#}")}));
            }
        }
    }
    elapsed.sort_unstable();
    let per_baseline:Vec<_>=["oxipng-o6","oxipng-o6-zopfli","zopflipng-m"].iter().map(|name| {
        let mut n=0usize;let mut size=0u64;let mut kito=0u64;let mut time=0u64;let mut win=0usize;let mut tie=0usize;let mut loss=0usize;
        for row in &rows {if let Some(trial)=row["baselines"].as_array().and_then(|a|a.iter().find(|t|t["engine"]==*name && t["valid"]==true)) {
            let b=trial["bytes"].as_u64().unwrap().min(row["input_bytes"].as_u64().unwrap());let c=row["output_bytes"].as_u64().unwrap();n+=1;size+=b;kito+=c;time+=trial["elapsed_ms"].as_u64().unwrap_or(0);
            match c.cmp(&b) {std::cmp::Ordering::Less=>win+=1,std::cmp::Ordering::Equal=>tie+=1,std::cmp::Ordering::Greater=>loss+=1};
        }}
        serde_json::json!({"engine":name,"valid_files":n,"bytes_including_original_fallback":size,"kito_bytes_for_same_files":kito,"total_ms":time,"wins":win,"ties":tie,"losses":loss})
    }).collect();
    #[derive(Serialize)]
    struct Settings {
        effort: &'static str,
        strip_metadata: bool,
        memory_bytes: usize,
        time_limit_seconds: Option<f64>,
    }
    Ok(
        serde_json::json!({"schema_version":2,"versions":{"kito":env!("CARGO_PKG_VERSION"),"oxipng":"10.2.1 (locked)","zopflipng":backend::zopflipng_version().ok()},"environment":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH},"settings":Settings{effort:if config.max {"max"} else {"fast"},strip_metadata:config.strip,memory_bytes:config.memory_bytes,time_limit_seconds:config.time_limit.map(|t|t.as_secs_f64())},"comparison_complete":all_baselines_valid && rows.iter().all(|r|r["kito"]["search_complete"]==true),"totals":{"input_bytes":input_total,"kito_bytes":output_total,"saved_bytes":input_total-output_total,"saved_percent":if input_total>0 {100.0*(input_total-output_total) as f64/input_total as f64} else {0.0},"compared_files":compared,"kito_bytes_for_compared_files":compared_kito_bytes,"improvement_over_available_baseline_bytes":best_total as i128-compared_kito_bytes as i128,"best_available_baseline_bytes_for_compared_files":best_total,"wins":wins,"ties":ties,"losses":losses,"failed":failed,"kito_total_ms":elapsed.iter().sum::<u128>(),"kito_median_ms":elapsed.get(elapsed.len()/2),"peak_rss_bytes":null},"baselines":per_baseline,"files":rows,"notes":["峰值 RSS 未在本报告测量，以 null 表示；内存预算为工作集预检，不是 RSS 硬上限。","字节数比较保留 original 候选；缺失、失败或预算未完成的基线不计入比较。","时间预算会在优化阶段边界检查；内置单次 DEFLATE 调用不能立即抢占。"]}),
    )
}
fn bail_candidate() -> Result<Vec<u8>> {
    bail!("时间预算为零，未运行")
}
