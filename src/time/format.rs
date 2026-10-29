//! 日期输出模板。只改变展示，不参与输入解析或 Unix 时间戳计算。

use super::Zone;
use anyhow::{Context, Result, ensure};
use chrono::{
    DateTime, Utc,
    format::{Item, StrftimeItems},
};
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct DateFormat {
    pattern: String,
    items: Vec<Item<'static>>,
}

impl FromStr for DateFormat {
    type Err = anyhow::Error;

    fn from_str(pattern: &str) -> Result<Self> {
        ensure!(
            !pattern.trim().is_empty(),
            "--format 不能是空字符串或纯空白"
        );
        let items = StrftimeItems::new(pattern)
            .parse_to_owned()
            .context("无效的 --format 模板；例如 %Y-%m-%d %H:%M:%S%.6f%:z")?;
        Ok(Self {
            pattern: pattern.to_owned(),
            items,
        })
    }
}

impl DateFormat {
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    pub(super) fn render(&self, instant: DateTime<Utc>, zone: &Zone) -> Result<String> {
        let mut rendered = String::new();
        // 直接使用真实时区进行格式化，让 %Z 取到当时的 EST/EDT 等缩写，
        // %s 仍计算同一个 UTC 时刻，不能先丢弃 offset 再格式化本地时间。
        let result = match zone {
            Zone::Named(zone) => instant
                .with_timezone(zone)
                .format_with_items(self.items.iter())
                .write_to(&mut rendered),
            Zone::Fixed(offset) => instant
                .with_timezone(offset)
                .format_with_items(self.items.iter())
                .write_to(&mut rendered),
        };
        // 部分合法的 strptime 标记（例如 %#z）不能用于输出。write_to 返回错误，
        // 避免 Display::to_string 在格式化失败时 panic，也不输出已经生成的半截内容。
        result.context("--format 包含无法用于输出的格式标记；偏移可用 %:z 或 %::z")?;
        Ok(rendered)
    }
}
