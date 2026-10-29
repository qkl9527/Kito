//! 全球时区目录使用与转换相同的内置 IANA 数据，所有条目的偏移取自同一个时刻。

use super::{Zone, parse_datetime, timezone};
use anyhow::{Context, Result};
use chrono::{DateTime, FixedOffset, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub struct ZoneEntry {
    pub name: String,
    pub utc_offset: String,
    pub offset_seconds: i32,
    pub abbreviation: String,
}

#[derive(Serialize)]
pub struct ZoneList {
    pub reference_utc: String,
    pub tzdb_version: &'static str,
    pub zones: Vec<ZoneEntry>,
}

pub fn list(filter: Option<&str>, at: Option<&str>) -> Result<ZoneList> {
    let reference: DateTime<Utc> = if let Some(at) = at {
        let parsed = parse_datetime(at)?;
        let zone = parsed
            .embedded_zone()
            .context("--at 必须包含 UTC 偏移，例如 2024-07-01T00:00:00Z")?;
        parsed.resolve(&zone)?
    } else {
        Utc::now()
    };
    let filter = filter.unwrap_or("").trim().to_ascii_lowercase();
    let mut zones: Vec<_> = chrono_tz::TZ_VARIANTS
        .iter()
        .filter_map(|zone| {
            let (offset, abbreviation) = Zone::Named(*zone).offset_at(reference);
            let row = ZoneEntry {
                name: zone.name().to_owned(),
                utc_offset: timezone::format_offset(offset),
                offset_seconds: offset.local_minus_utc(),
                abbreviation,
            };
            // 同时搜索名称、偏移和缩写；缩写可能对应多个地区，因此保留所有匹配项。
            let matches = [&row.name, &row.utc_offset, &row.abbreviation]
                .into_iter()
                .any(|text| text.to_ascii_lowercase().contains(&filter));
            matches.then_some(row)
        })
        .collect();
    zones.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(ZoneList {
        reference_utc: timezone::format_datetime(reference, FixedOffset::east_opt(0).unwrap())?,
        tzdb_version: chrono_tz::IANA_TZDB_VERSION,
        zones,
    })
}
