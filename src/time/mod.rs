//! 时间领域：所有转换先归一为 UTC 时刻，再按目标时区显示。
//! Unix 时间戳始终表示同一条时间轴上的计数，时区只参与日期时间的解释和展示。

pub mod cli;
mod config;
mod format;
mod timezone;
pub mod zones;

use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime, TimeZone, Utc};
use clap::ValueEnum;
pub use format::DateFormat;
use serde::Serialize;
pub use timezone::Zone;

/// 使用整数倍率计算时间戳，避免经 f64 转换后丢失微秒/纳秒低位。
#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
pub enum Unit {
    #[value(name = "s", aliases = ["second", "seconds"])]
    #[serde(rename = "s")]
    Seconds,
    #[value(name = "ms", aliases = ["millisecond", "milliseconds"])]
    #[serde(rename = "ms")]
    Milliseconds,
    #[value(name = "us", aliases = ["microsecond", "microseconds"])]
    #[serde(rename = "us")]
    Microseconds,
    #[value(name = "ns", aliases = ["nanosecond", "nanoseconds"])]
    #[serde(rename = "ns")]
    Nanoseconds,
}

impl Unit {
    pub fn name(self) -> &'static str {
        match self {
            Self::Seconds => "s",
            Self::Milliseconds => "ms",
            Self::Microseconds => "us",
            Self::Nanoseconds => "ns",
        }
    }

    fn per_second(self) -> i128 {
        match self {
            Self::Seconds => 1,
            Self::Milliseconds => 1_000,
            Self::Microseconds => 1_000_000,
            Self::Nanoseconds => 1_000_000_000,
        }
    }

    fn nanoseconds(self) -> i128 {
        1_000_000_000 / self.per_second()
    }
}

/// auto 只是方便输入现代常见时间戳的规则，无法推断任意年代的实际单位。
/// 先去掉符号和前导零；非典型位数必须明确指定，避免悄悄转换成错误年代。
pub fn parse_timestamp(input: &str, unit: Option<Unit>) -> Result<(DateTime<Utc>, Unit)> {
    let input = input.trim();
    let digits = input.strip_prefix(['-', '+']).unwrap_or(input);
    ensure!(
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()),
        "时间戳必须是整数；请用 --unit s/ms/us/ns 指定精度"
    );
    let value: i128 = input.parse().context("时间戳超出整数范围")?;
    let unit = match unit {
        Some(unit) => unit,
        None => match digits.trim_start_matches('0').len() {
            0..=10 => Unit::Seconds,
            13 => Unit::Milliseconds,
            16 => Unit::Microseconds,
            19 => Unit::Nanoseconds,
            _ => bail!("无法可靠识别时间戳单位，请明确指定 --unit s/ms/us/ns"),
        },
    };

    // Chrono 的秒为向负无穷取整后的秒，纳秒余数必须非负。
    // 例如 -1 ms 应拆成 (-1 s, 999_000_000 ns)，不能使用向零截断的 / 和 %。
    let seconds =
        i64::try_from(value.div_euclid(unit.per_second())).context("时间戳超出可表示的日期范围")?;
    let nanos = (value.rem_euclid(unit.per_second()) * unit.nanoseconds()) as u32;
    let instant = DateTime::from_timestamp(seconds, nanos).context("时间戳超出可表示的日期范围")?;
    validate_instant(instant)?;
    Ok((instant, unit))
}

/// 将语法解析与时区选择分开：输入自带 offset 时，不必读取环境默认时区。
#[derive(Debug)]
pub struct ParsedDate {
    local: NaiveDateTime,
    offset: Option<FixedOffset>,
}

impl ParsedDate {
    pub fn embedded_zone(&self) -> Option<Zone> {
        self.offset.map(Zone::Fixed)
    }

    pub fn resolve(&self, source: &Zone) -> Result<DateTime<Utc>> {
        let instant = if let Some(offset) = self.offset {
            let instant = offset
                .from_local_datetime(&self.local)
                .single()
                .context("日期时间超出可表示范围")?
                .with_timezone(&Utc);
            // 显式源时区与输入 offset 同时存在时，必须描述同一个本地时间。
            // 这也允许用户用 -04:00 / -05:00 消除纽约秋季回拨时的歧义。
            ensure!(
                source.offset_at(instant).0 == offset,
                "输入的 UTC 偏移与源时区 {} 在该时刻的规则不一致",
                source.name()
            );
            instant
        } else {
            source.resolve_local(self.local)?
        };
        validate_instant(instant)?;
        Ok(instant)
    }
}

pub fn parse_datetime(input: &str) -> Result<ParsedDate> {
    let input = input.trim();
    // Chrono 的 %Y-%m-%d %H:%M:%S 允许不定宽数字，且空白能匹配零个字符。
    // 必须先固定字段边界，否则 2024-01-112:00:00 会被误读为 1 月 11 日 02:00。
    // 前缀逐字节确认是 ASCII 后才进入解析，短字符串和 Unicode 输入也安全报错。
    let bytes = input.as_bytes();
    ensure!(
        bytes.len() >= 19
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && matches!(bytes[10], b'T' | b't' | b' ')
            && bytes[13] == b':'
            && bytes[16] == b':'
            && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
                .into_iter()
                .all(|index| bytes[index].is_ascii_digit()),
        "日期时间格式无效；请使用 YYYY-MM-DD HH:MM:SS 或 RFC 3339，保留分隔符并补齐两位月、日、时、分、秒"
    );
    // Chrono 可以接受超过 9 位的小数并截断；CLI 必须拒绝这种隐式精度损失。
    if let Some((_, fraction)) = input.split_once('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        ensure!((1..=9).contains(&digits), "日期时间的小数秒必须为 1–9 位");
    }
    let parsed = if let Ok(aware) = DateTime::parse_from_rfc3339(input) {
        ParsedDate {
            local: aware.naive_local(),
            offset: Some(*aware.offset()),
        }
    } else {
        // RFC 3339 不支持秒级时区偏移；历史 IANA 规则可能包含 +08:05:43。
        // 接受这个扩展，使工具自己的历史日期输出也能无损转换回来。
        let offset_start = input
            .get(10..)
            .and_then(|tail| tail.rfind(['+', '-']))
            .map(|index| index + 10);
        let (date, offset) = if let Some(start) = offset_start {
            let offset = timezone::parse_fixed_offset(&input[start..])?;
            (&input[..start], Some(offset))
        } else {
            (input, None)
        };
        let local = NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S%.f")
            .or_else(|_| NaiveDateTime::parse_from_str(date, "%Y-%m-%dT%H:%M:%S%.f"))
            .context("日期时间格式无效，请用 RFC 3339 或 YYYY-MM-DD HH:MM:SS[.小数秒]")?;
        ParsedDate { local, offset }
    };
    ensure!(
        parsed.local.and_utc().timestamp_subsec_nanos() < 1_000_000_000,
        "Unix 时间戳不表示闰秒，请使用不含 :60 的日期时间"
    );
    ensure!(
        (0..=9999).contains(&parsed.local.year()),
        "日期年份必须在 0000–9999 之间"
    );
    Ok(parsed)
}

fn validate_instant(instant: DateTime<Utc>) -> Result<()> {
    ensure!(
        (0..=9999).contains(&instant.year()),
        "UTC 日期年份必须在 0000–9999 之间"
    );
    ensure!(
        instant.timestamp_subsec_nanos() < 1_000_000_000,
        "不支持闰秒"
    );
    Ok(())
}

fn total_nanoseconds(instant: DateTime<Utc>) -> i128 {
    i128::from(instant.timestamp()) * 1_000_000_000 + i128::from(instant.timestamp_subsec_nanos())
}

pub fn timestamp(instant: DateTime<Utc>, unit: Unit) -> String {
    total_nanoseconds(instant)
        .div_euclid(unit.nanoseconds())
        .to_string()
}

/// JSON 使用字符串，保留超出 JavaScript 2^53 或 i64 范围的纳秒整数。
#[derive(Serialize)]
pub struct Timestamps {
    pub s: String,
    pub ms: String,
    pub us: String,
    pub ns: String,
}

#[derive(Serialize)]
pub struct Report {
    pub operation: &'static str,
    pub value: String,
    pub datetime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    pub timezone: String,
    pub utc: String,
    pub utc_offset: String,
    pub abbreviation: String,
    pub timestamps: Timestamps,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<Unit>,
    pub precision_lost: bool,
}

pub fn report(
    operation: &'static str,
    instant: DateTime<Utc>,
    zone: &Zone,
    unit: Option<Unit>,
    timestamp_result: bool,
    format: Option<&DateFormat>,
) -> Result<Report> {
    validate_instant(instant)?;
    let (offset, abbreviation) = zone.offset_at(instant);
    let datetime = timezone::format_datetime(instant, offset)?;
    // 先通过默认格式的目标日期范围检查，再应用用户模板；自定义格式不能绕过范围限制。
    // 即使模板省略时区或截短小数，utc 和 timestamps 仍保留完整时刻。
    let datetime = match format {
        Some(format) => format.render(instant, zone)?,
        None => datetime,
    };
    let selected_unit = unit.unwrap_or(Unit::Seconds);
    Ok(Report {
        operation,
        value: if timestamp_result {
            timestamp(instant, selected_unit)
        } else {
            datetime.clone()
        },
        datetime,
        format: format.map(|format| format.pattern().to_owned()),
        timezone: zone.name(),
        utc: timezone::format_datetime(instant, FixedOffset::east_opt(0).unwrap())?,
        utc_offset: timezone::format_offset(offset),
        abbreviation,
        timestamps: Timestamps {
            s: timestamp(instant, Unit::Seconds),
            ms: timestamp(instant, Unit::Milliseconds),
            us: timestamp(instant, Unit::Microseconds),
            ns: timestamp(instant, Unit::Nanoseconds),
        },
        unit,
        precision_lost: timestamp_result
            && total_nanoseconds(instant).rem_euclid(selected_unit.nanoseconds()) != 0,
    })
}
