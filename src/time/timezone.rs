use anyhow::{Context, Result, bail, ensure};
use chrono::{
    DateTime, Datelike, FixedOffset, LocalResult, NaiveDateTime, Offset, TimeDelta, TimeZone, Utc,
};
use chrono_tz::Tz;
use std::str::FromStr;

/// IANA 区域保留历史和夏令时规则，固定偏移只表示全年不变的 UTC 差值。
#[derive(Clone, Debug)]
pub enum Zone {
    Named(Tz),
    Fixed(FixedOffset),
}

impl FromStr for Zone {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        let input = input.trim();
        if input.eq_ignore_ascii_case("utc") || input.eq_ignore_ascii_case("z") {
            return Ok(Self::Named(chrono_tz::UTC));
        }
        if input.starts_with(['+', '-']) {
            return Ok(Self::Fixed(parse_fixed_offset(input)?));
        }
        input.parse::<Tz>().map(Self::Named).map_err(|_| {
            anyhow::anyhow!("未知时区 {input:?}；请运行 kito time zones 查询 IANA 名称，或使用 +08:00 这样的固定偏移")
        })
    }
}

impl Zone {
    pub fn name(&self) -> String {
        match self {
            Self::Named(zone) => zone.name().to_owned(),
            Self::Fixed(offset) if offset.local_minus_utc() == 0 => "UTC".to_owned(),
            Self::Fixed(offset) => format_offset(*offset),
        }
    }

    pub fn offset_at(&self, instant: DateTime<Utc>) -> (FixedOffset, String) {
        match self {
            Self::Named(zone) => {
                let offset = zone.offset_from_utc_datetime(&instant.naive_utc());
                (offset.fix(), offset.to_string())
            }
            Self::Fixed(offset) => (*offset, format!("UTC{}", format_offset(*offset))),
        }
    }

    pub fn resolve_local(&self, local: NaiveDateTime) -> Result<DateTime<Utc>> {
        let result = match self {
            Self::Named(zone) => zone
                .from_local_datetime(&local)
                .map(|date| date.with_timezone(&Utc)),
            Self::Fixed(offset) => offset
                .from_local_datetime(&local)
                .map(|date| date.with_timezone(&Utc)),
        };
        // 跳过的时间没有对应时刻，回拨的时间有两个时刻；不能任意选取一个。
        match result {
            LocalResult::Single(instant) => Ok(instant),
            LocalResult::None => bail!(
                "{local} 在时区 {} 中不存在（可能处于夏令时跳变），请核对日期时间",
                self.name()
            ),
            LocalResult::Ambiguous(first, second) => {
                let first = format_offset(self.offset_at(first).0);
                let second = format_offset(self.offset_at(second).0);
                bail!(
                    "{local} 在时区 {} 中存在歧义；请在日期后附上 {first} 或 {second} 明确偏移",
                    self.name()
                )
            }
        }
    }
}

pub(super) fn parse_fixed_offset(input: &str) -> Result<FixedOffset> {
    let bytes = input.as_bytes();
    ensure!(
        matches!(bytes.len(), 6 | 9)
            && matches!(bytes[0], b'+' | b'-')
            && bytes[3] == b':'
            && (bytes.len() == 6 || bytes[6] == b':')
            && [1, 2, 4, 5].into_iter().all(|i| bytes[i].is_ascii_digit())
            && (bytes.len() == 6 || (bytes[7].is_ascii_digit() && bytes[8].is_ascii_digit())),
        "固定时区偏移格式应为 +08:00、-03:30 或 ±HH:MM:SS"
    );
    let pair = |i: usize| i32::from(bytes[i] - b'0') * 10 + i32::from(bytes[i + 1] - b'0');
    let (hour, minute, second) = (pair(1), pair(4), if bytes.len() == 9 { pair(7) } else { 0 });
    ensure!(
        hour < 24 && minute < 60 && second < 60,
        "UTC 偏移必须小于 24 小时，分秒须在 00–59 之间"
    );
    let sign = if bytes[0] == b'-' { -1 } else { 1 };
    FixedOffset::east_opt(sign * (hour * 3600 + minute * 60 + second)).context("无效 UTC 偏移")
}

pub(super) fn format_offset(offset: FixedOffset) -> String {
    let seconds = offset.local_minus_utc();
    let sign = if seconds < 0 { '-' } else { '+' };
    let value = seconds.unsigned_abs();
    let base = format!("{sign}{:02}:{:02}", value / 3600, value / 60 % 60);
    if value.is_multiple_of(60) {
        base
    } else {
        format!("{base}:{:02}", value % 60)
    }
}

pub(super) fn format_datetime(instant: DateTime<Utc>, offset: FixedOffset) -> Result<String> {
    // 先做有检查的日期偏移，避免边界年份在格式化阶段 panic。
    let local = instant
        .naive_utc()
        .checked_add_signed(TimeDelta::seconds(i64::from(offset.local_minus_utc())))
        .context("目标时区日期超出可表示范围")?;
    ensure!(
        (0..=9999).contains(&local.year()),
        "目标时区日期年份必须在 0000–9999 之间"
    );
    let suffix = if offset.local_minus_utc() == 0 {
        "Z".to_owned()
    } else {
        format_offset(offset)
    };
    Ok(format!("{}{suffix}", local.format("%Y-%m-%dT%H:%M:%S%.f")))
}
