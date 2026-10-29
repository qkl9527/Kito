//! 默认时区持久化。配置目录按平台选择，写入复用共享的原子发布逻辑。
//! 读取延迟到需要默认值时，确保显式参数能够覆盖损坏或不可读的配置。

use super::Zone;
use crate::files;
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::value::{RawValue, to_raw_value};
use std::{
    collections::BTreeMap,
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

const MAX_CONFIG_BYTES: usize = 64 * 1024;
// 其他字段作为原始 JSON 片段保存，避免超 u64 整数或高精度小数经 f64 往返改值。
type Document = BTreeMap<String, Box<RawValue>>;

pub(super) struct DefaultZone {
    pub zone: Zone,
    pub source: &'static str,
    pub path: Option<PathBuf>,
}

#[derive(Serialize)]
pub(super) struct ConfigReport {
    pub action: &'static str,
    pub timezone: Option<String>,
    pub source: Option<&'static str>,
    pub config_path: Option<PathBuf>,
}

fn absolute_directory(path: PathBuf, variable: &str) -> Result<PathBuf> {
    ensure!(path.is_absolute(), "{variable} 必须是非空的绝对路径");
    Ok(path)
}

fn user_directory(variable: &str) -> Result<Option<PathBuf>> {
    env::var_os(variable)
        .map(|path| absolute_directory(PathBuf::from(path), variable))
        .transpose()
}

fn config_path() -> Result<Option<PathBuf>> {
    if let Some(directory) = env::var_os("KITO_CONFIG_DIR") {
        return Ok(Some(
            absolute_directory(PathBuf::from(directory), "KITO_CONFIG_DIR")?.join("time.json"),
        ));
    }
    #[cfg(target_os = "windows")]
    let base = user_directory("APPDATA")?;
    #[cfg(target_os = "macos")]
    let base = user_directory("HOME")?.map(|home| home.join("Library").join("Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = match env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        Some(path) => Some(path),
        // XDG 约定忽略空值或相对路径，以用户目录下的 .config 为后备。
        None => user_directory("HOME")?.map(|home| home.join(".config")),
    };
    Ok(base.map(|path| path.join("kito").join("time.json")))
}

fn writable_path() -> Result<PathBuf> {
    config_path()?.context("无法定位用户配置目录；请用 KITO_CONFIG_DIR 指定绝对路径")
}

fn read_document(path: &Path) -> Result<Option<Document>> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("无法访问时区配置 {}", path.display()));
        }
        Ok(metadata) => ensure!(
            metadata.is_file(),
            "时区配置必须是普通文件，不能是目录或符号链接：{}",
            path.display()
        ),
    }
    // read_bounded 的文件上限为预算的 1/8；统一使用共享的有界读取实现。
    let bytes = files::read_bounded(path, MAX_CONFIG_BYTES * 8)?;
    let document = serde_json::from_slice(&bytes)
        .with_context(|| format!("时区配置必须是有效 JSON 对象：{}", path.display()))?;
    Ok(Some(document))
}

fn write_document(path: &Path, document: &Document) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(document)?;
    bytes.push(b'\n');
    ensure!(
        bytes.len() <= MAX_CONFIG_BYTES,
        "时区配置超过 64 KiB，未保存"
    );
    // 只在完整 JSON 和时区验证成功后发布。同目录临时文件先落盘再原子替换，
    // 保留原文件权限，写入失败时不留下半个配置，也不替换目录/符号链接。
    files::publish(&bytes, path, true)
        .with_context(|| format!("无法保存时区配置 {}", path.display()))
}

pub(super) fn effective() -> Result<DefaultZone> {
    match env::var("KITO_TIMEZONE") {
        Ok(value) => {
            return Ok(DefaultZone {
                zone: value.parse().context("KITO_TIMEZONE 配置无效")?,
                source: "environment",
                path: None,
            });
        }
        Err(env::VarError::NotPresent) => {}
        Err(error) => return Err(error).context("KITO_TIMEZONE 必须是有效文本"),
    }
    let path = config_path()?;
    if let Some(path) = &path
        && let Some(document) = read_document(path)?
        && let Some(value) = document.get("timezone")
    {
        let value: String = serde_json::from_str(value.get())
            .with_context(|| format!("timezone 必须是字符串：{}", path.display()))?;
        return Ok(DefaultZone {
            zone: value
                .parse()
                .with_context(|| format!("保存的默认时区无效：{}", path.display()))?,
            source: "config",
            path: Some(path.clone()),
        });
    }
    Ok(DefaultZone {
        zone: Zone::Named(chrono_tz::UTC),
        source: "fallback",
        // 精简环境中可能没有 HOME/APPDATA；无从发现保存项时，普通转换仍可用 UTC。
        path,
    })
}

pub(super) fn get() -> Result<ConfigReport> {
    let selected = effective()?;
    Ok(ConfigReport {
        action: "get",
        timezone: Some(selected.zone.name()),
        source: Some(selected.source),
        config_path: selected.path,
    })
}

pub(super) fn set(zone: &Zone) -> Result<ConfigReport> {
    let path = writable_path()?;
    let mut document = read_document(&path)?.unwrap_or_default();
    let timezone = zone.name();
    // 仅更新自己的字段，保留用户或未来版本保存的其他配置项。
    document.insert("timezone".to_owned(), to_raw_value(&timezone)?);
    write_document(&path, &document)?;
    // set 报告保存值，不重新解析环境默认；避免成功写入后因环境覆盖无效而误报失败。
    Ok(ConfigReport {
        action: "set",
        timezone: Some(timezone),
        source: Some("config"),
        config_path: Some(path),
    })
}

pub(super) fn reset() -> Result<ConfigReport> {
    let path = writable_path()?;
    if let Some(mut document) = read_document(&path)?
        && document.remove("timezone").is_some()
    {
        write_document(&path, &document)?;
    }
    // reset 只清除落盘项；环境覆盖仍有效，且不为缺失的配置创建文件。
    Ok(ConfigReport {
        action: "reset",
        timezone: None,
        source: None,
        config_path: Some(path),
    })
}
