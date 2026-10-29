//! 有限候选搜索：所有候选都经过独立验证，原文件始终参与选择。
use super::png::{PngImage, chunks, equivalent, inspect, strip_metadata};
use anyhow::Result;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct OptimizeConfig {
    pub max: bool,
    pub strip: bool,
    pub memory_bytes: usize,
    pub time_limit: Option<Duration>,
}
impl Default for OptimizeConfig {
    fn default() -> Self {
        Self {
            max: false,
            strip: false,
            memory_bytes: 512 * 1024 * 1024,
            time_limit: None,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Trial {
    pub engine: String,
    pub bytes: Option<usize>,
    pub elapsed_ms: u128,
    pub valid: bool,
    /// 可选后端未安装时为 true；跳过从不代表候选已验证，也不能计入基准。
    pub skipped: bool,
    pub error: Option<String>,
}
#[derive(Serialize)]
pub struct Optimized {
    #[serde(skip)]
    pub bytes: Vec<u8>,
    pub engine: String,
    pub trials: Vec<Trial>,
    pub search_complete: bool,
    pub metadata_saved_bytes: usize,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

pub fn oxi_candidate(
    bytes: &[u8],
    config: &OptimizeConfig,
    level: u8,
    zopfli: bool,
) -> Result<Vec<u8>> {
    let mut opts = oxipng::Options::from_preset(level);
    opts.optimize_alpha = false;
    opts.scale_16 = false;
    opts.bit_depth_reduction = inspect(bytes)?.bit_depth != 16;
    opts.max_decompressed_size = Some(config.memory_bytes / 2);
    opts.timeout = config.time_limit;
    opts.strip = oxipng::StripChunks::Keep(
        chunks(bytes)?
            .iter()
            .filter(|c| c.kind[0].is_ascii_lowercase())
            .map(|c| c.kind)
            .collect(),
    );
    if zopfli {
        opts.deflater = oxipng::Deflater::Zopfli(oxipng::ZopfliOptions::default());
        opts.fast_evaluation = true;
    }
    Ok(oxipng::optimize_from_memory(bytes, &opts)?)
}

pub fn optimize(bytes: &[u8], config: &OptimizeConfig) -> Result<Optimized> {
    PngImage::decode(bytes, config.memory_bytes)?;
    let started = Instant::now();
    let stripped = if config.strip {
        strip_metadata(bytes)?
    } else {
        bytes.to_vec()
    };
    let source = if equivalent(bytes, &stripped, config.strip, config.memory_bytes).is_ok() {
        stripped
    } else {
        bytes.to_vec()
    };
    let mut best = Optimized {
        bytes: source.clone(),
        engine: "original".into(),
        trials: Vec::new(),
        search_complete: true,
        metadata_saved_bytes: bytes.len() - source.len(),
        warnings: Vec::new(),
        notes: Vec::new(),
    };
    let strategies = if config.max {
        vec![(2, false), (4, false), (6, false), (6, true)]
    } else {
        vec![(2, false)]
    };
    for (level, zopfli) in strategies {
        if config.time_limit.is_some_and(|t| started.elapsed() >= t) {
            best.search_complete = false;
            break;
        }
        let mut remaining = config.clone();
        remaining.time_limit = config
            .time_limit
            .map(|t| t.saturating_sub(started.elapsed()));
        let name = format!("oxipng-o{level}{}", if zopfli { "-zopfli" } else { "" });
        let begin = Instant::now();
        let candidate = oxi_candidate(&source, &remaining, level, zopfli);
        consider(bytes, candidate, name, begin.elapsed(), config, &mut best);
        if config.time_limit.is_some_and(|t| started.elapsed() >= t) {
            best.search_complete = false;
            break;
        }
    }
    if config.max && best.search_complete {
        for (name, variant) in super::palette::variants(&source, config.memory_bytes)? {
            if config.time_limit.is_some_and(|t| started.elapsed() >= t) {
                best.search_complete = false;
                break;
            }
            let begin = Instant::now();
            let mut opts = oxipng::Options::from_preset(4);
            opts.palette_reduction = false;
            opts.optimize_alpha = false;
            opts.max_decompressed_size = Some(config.memory_bytes / 2);
            opts.timeout = config
                .time_limit
                .map(|t| t.saturating_sub(started.elapsed()));
            let candidate =
                oxipng::optimize_from_memory(&variant, &opts).map_err(anyhow::Error::from);
            consider(bytes, candidate, name, begin.elapsed(), config, &mut best);
        }
        if config.time_limit.is_some_and(|t| started.elapsed() >= t) {
            best.search_complete = false;
        }
        if best.search_complete {
            let begin = Instant::now();
            let mut remaining = config.clone();
            remaining.time_limit = config
                .time_limit
                .map(|t| t.saturating_sub(started.elapsed()));
            let candidate = zopfli_candidate(&source, &remaining);
            consider(
                bytes,
                candidate,
                "zopflipng-m".into(),
                begin.elapsed(),
                config,
                &mut best,
            );
            // 先完成候选验证，再报告结果：成功启动但未输出、输出损坏等都属于失败。
            let trial = best.trials.last().expect("consider 总会记录本次候选");
            if trial.skipped {
                best.notes
                    .push("未安装可选 ZopfliPNG，已跳过；内置优化正常完成。".into());
            } else if let Some(error) = &trial.error {
                best.warnings.push(format!(
                    "可选 ZopfliPNG 执行失败，已保留其他已验证候选：{error}"
                ));
            }
            if config.time_limit.is_some_and(|t| started.elapsed() >= t) {
                best.search_complete = false;
            }
        }
    }
    Ok(best)
}

pub fn zopfli_candidate(bytes: &[u8], config: &OptimizeConfig) -> Result<Vec<u8>> {
    use std::{ffi::OsString, fs};
    let (dir, input) = crate::backend::safe_input(bytes)?;
    let output = dir.path().join("candidate.png");
    let names: Vec<_> = chunks(bytes)?
        .iter()
        .filter(|c| c.kind[0].is_ascii_lowercase() && c.kind != *b"tRNS")
        .map(|c| String::from_utf8_lossy(&c.kind).to_string())
        .collect();
    let mut args: Vec<OsString> = vec!["-m".into(), "--always_zopflify".into()];
    if !names.is_empty() {
        args.push(format!("--keepchunks={}", names.join(",")).into());
    }
    args.push(input.into_os_string());
    args.push(output.as_os_str().to_owned());
    crate::backend::run_zopflipng(&args, config.time_limit)?;
    let bytes = fs::read(output)?;
    Ok(bytes)
}

pub fn consider(
    original: &[u8],
    candidate: Result<Vec<u8>>,
    name: String,
    elapsed: Duration,
    config: &OptimizeConfig,
    best: &mut Optimized,
) {
    let mut trial = Trial {
        engine: name.clone(),
        bytes: None,
        elapsed_ms: elapsed.as_millis(),
        valid: false,
        skipped: false,
        error: None,
    };
    match candidate.and_then(|data| {
        trial.bytes = Some(data.len());
        equivalent(original, &data, config.strip, config.memory_bytes)?;
        Ok(data)
    }) {
        Ok(data) => {
            trial.valid = true;
            if data.len() < best.bytes.len() {
                best.bytes = data;
                best.engine = name;
            }
        }
        Err(e) => {
            trial.skipped = crate::backend::is_optional_backend_missing(&e);
            trial.error = Some(format!("{e:#}"));
        }
    }
    best.trials.push(trial);
}
