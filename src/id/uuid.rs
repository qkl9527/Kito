//! RFC 9562 UUID v4/v7：随机字节显式取自可失败的 OS 随机源，位布局在此固定。

use anyhow::{Context, Result, anyhow, ensure};
use clap::ValueEnum;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Version {
    #[default]
    V4,
    V7,
}

#[derive(Clone, Copy, Debug, Default, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Hyphenated,
    Simple,
    Urn,
}

#[derive(Serialize)]
pub struct Report {
    pub version: Version,
    pub format: Format,
    pub values: Vec<String>,
}

pub fn generate(version: Version, format: Format, count: u16) -> Result<Report> {
    generate_with(
        version,
        format,
        count,
        || unix_millis(SystemTime::now()),
        |bytes| {
            getrandom::fill(bytes).map_err(|error| anyhow!("获取操作系统随机数据失败：{error}"))
        },
    )
}

fn unix_millis(now: SystemTime) -> Result<u128> {
    Ok(now
        .duration_since(UNIX_EPOCH)
        .context("系统时间早于 Unix epoch，不能生成 UUID v7")?
        .as_millis())
}

fn generate_with(
    version: Version,
    format: Format,
    count: u16,
    mut now: impl FnMut() -> Result<u128>,
    mut random: impl FnMut(&mut [u8]) -> Result<()>,
) -> Result<Report> {
    ensure!((1..=10_000).contains(&count), "UUID 数量须在 1–10000 之间");
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let timestamp = match version {
            Version::V4 => None,
            Version::V7 => {
                let milliseconds = now()?;
                ensure!(
                    milliseconds <= 0xffff_ffff_ffff,
                    "系统时间超出 UUID v7 的 48 位毫秒范围"
                );
                Some(milliseconds)
            }
        };
        let mut bytes = [0_u8; 16];
        random(&mut bytes)?;
        if let Some(milliseconds) = timestamp {
            // v7 前 48 位是网络字节序的 Unix 毫秒，其余随机位保留；不承诺同毫秒单调性。
            bytes[..6].copy_from_slice(&milliseconds.to_be_bytes()[10..]);
            bytes[6] = (bytes[6] & 0x0f) | 0x70;
        } else {
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
        }
        // RFC variant 固定为二进制 10，不改变低 6 位随机数据。
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let value = Uuid::from_bytes(bytes);
        values.push(match format {
            Format::Hyphenated => value.hyphenated().to_string(),
            Format::Simple => value.simple().to_string(),
            Format::Urn => value.urn().to_string(),
        });
    }
    // 全批次生成后再交给 CLI 输出，随机源或时钟在中途失败时不发布部分结果。
    Ok(Report {
        version,
        format,
        values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_v7_layout_preserves_random_bits() {
        let report = generate_with(
            Version::V7,
            Format::Hyphenated,
            1,
            || Ok(0x0123456789ab),
            |bytes| {
                bytes.fill(0xff);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(report.values, ["01234567-89ab-7fff-bfff-ffffffffffff"]);
    }

    #[test]
    fn v4_does_not_require_a_valid_clock() {
        let report = generate_with(
            Version::V4,
            Format::Simple,
            1,
            || Err(anyhow!("无效时钟")),
            |bytes| {
                bytes.fill(0);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(report.values, ["00000000000040008000000000000000"]);
    }

    #[test]
    fn random_failure_aborts_the_entire_batch() {
        let mut calls = 0;
        let result = generate_with(
            Version::V4,
            Format::Hyphenated,
            2,
            || Ok(0),
            |_| {
                calls += 1;
                if calls == 2 {
                    Err(anyhow!("测试随机源失败"))
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.err().unwrap().to_string().contains("测试随机源失败"));
    }

    #[test]
    fn invalid_clock_and_timestamp_overflow_return_errors() {
        assert!(unix_millis(UNIX_EPOCH - std::time::Duration::from_secs(1)).is_err());
        for timestamp in [Err(anyhow!("测试时间失败")), Ok(0x1_0000_0000_0000)] {
            let mut timestamp = Some(timestamp);
            assert!(
                generate_with(
                    Version::V7,
                    Format::Simple,
                    1,
                    || timestamp.take().unwrap(),
                    |_| Ok(())
                )
                .is_err()
            );
        }
    }
}
