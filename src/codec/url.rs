//! 严格的查询参数解析：先按原始分隔符切分，再进行一次百分号解码。
use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use serde::Serialize;

#[derive(Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Form,
    Percent,
}

#[derive(Serialize)]
pub struct Parameter {
    pub name: String,
    pub value: String,
    pub has_equals: bool,
}

#[derive(Serialize)]
pub struct Components {
    pub scheme: String,
    pub username: String,
    pub password: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: String,
    pub fragment: Option<String>,
}

#[derive(Serialize)]
pub struct Parsed {
    pub mode: Mode,
    /// 组件来自 URL 库规范化后的结果；raw_query 则始终保留输入中的查询片段。
    pub components_are_normalized: bool,
    pub components: Option<Components>,
    pub raw_query: Option<String>,
    pub parameters: Vec<Parameter>,
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode(input: &str, mode: Mode, base_offset: usize) -> Result<String> {
    let bytes = input.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut position = 0;
    while position < bytes.len() {
        if bytes[position] == b'%' {
            let value = bytes
                .get(position + 1)
                .and_then(|a| hex(*a))
                .zip(bytes.get(position + 2).and_then(|b| hex(*b)));
            let (high, low) = value.with_context(|| {
                format!(
                    "查询串字节位置 {} 的百分号转义不完整或不是十六进制",
                    base_offset + position
                )
            })?;
            result.push(high * 16 + low);
            position += 3;
        } else {
            result.push(if bytes[position] == b'+' && matches!(mode, Mode::Form) {
                b' '
            } else {
                bytes[position]
            });
            position += 1;
        }
    }
    String::from_utf8(result)
        .with_context(|| format!("查询串字节位置 {base_offset} 的参数解码后不是有效 UTF-8"))
}

pub fn parse(input: &str, query_only: bool, mode: Mode) -> Result<Parsed> {
    ensure!(
        !input
            .bytes()
            .any(|b| b <= 0x1f || b == 0x7f || (!query_only && b == b' ')),
        "输入含未经编码的空格或控制字符；完整 URL 中请使用百分号编码"
    );
    let (components, raw_query) = if query_only {
        (
            None,
            Some(input.strip_prefix('?').unwrap_or(input).to_owned()),
        )
    } else {
        let parsed = ::url::Url::parse(input)
            .context("请输入带 scheme 的绝对 URL；原始查询串需显式指定 --query")?;
        // 先排除 fragment 再寻找 query，以免把 '#?x' 误当查询；不使用库重编码后的 query。
        let before_fragment = input.split('#').next().unwrap_or(input);
        let raw_query = before_fragment
            .split_once('?')
            .map(|(_, query)| query.to_owned());
        (
            Some(Components {
                scheme: parsed.scheme().to_owned(),
                username: parsed.username().to_owned(),
                password: parsed.password().map(str::to_owned),
                host: parsed.host_str().map(str::to_owned),
                port: parsed.port(),
                path: parsed.path().to_owned(),
                fragment: parsed.fragment().map(str::to_owned),
            }),
            raw_query,
        )
    };
    let mut parameters = Vec::new();
    let mut offset = 0;
    for segment in raw_query.as_deref().unwrap_or("").split('&') {
        if !segment.is_empty() {
            let (name, value, has_equals) = match segment.split_once('=') {
                Some((n, v)) => (n, v, true),
                None => (segment, "", false),
            };
            parameters.push(Parameter {
                name: decode(name, mode, offset)?,
                value: decode(value, mode, offset + name.len() + usize::from(has_equals))?,
                has_equals,
            });
        }
        offset += segment.len() + 1;
    }
    Ok(Parsed {
        mode,
        components_are_normalized: !query_only,
        components,
        raw_query,
        parameters,
    })
}

pub fn display(report: &Parsed) -> Result<String> {
    let mut result = String::new();
    if let Some(c) = &report.components {
        result.push_str(&format!(
            "URL 组件（解析后规范化）：\nscheme: {}\nhost: {}\nport: {}\npath: {}\nfragment: {}\n",
            c.scheme,
            c.host.as_deref().unwrap_or(""),
            c.port.map(|p| p.to_string()).unwrap_or_default(),
            c.path,
            c.fragment.as_deref().unwrap_or("")
        ));
    }
    result.push_str(&format!(
        "原始 query：{}\n参数（保留顺序，JSON 可无歧义表示换行）：\n",
        report.raw_query.as_deref().unwrap_or("")
    ));
    for p in &report.parameters {
        result.push_str(&format!(
            "{}{}{}\n",
            p.name,
            if p.has_equals { "=" } else { "" },
            p.value
        ));
    }
    // 便于直接复制到请求参数中：单值保持字符串，重复键收集为数组，避免覆盖丢值。
    // 对象键按名称排序，数组保留出现次序；原始参数顺序和等号信息仍在上方逐项展示。
    let mut grouped = BTreeMap::<&str, Vec<&str>>::new();
    for parameter in &report.parameters {
        grouped
            .entry(&parameter.name)
            .or_default()
            .push(&parameter.value);
    }
    let query: BTreeMap<_, serde_json::Value> = grouped
        .into_iter()
        .map(|(name, values)| {
            let value = match values.as_slice() {
                [value] => serde_json::Value::from(*value),
                _ => serde_json::Value::from(values),
            };
            (name, value)
        })
        .collect();
    result.push_str("\nQuery JSON:\n");
    result.push_str(&serde_json::to_string_pretty(&query).context("无法生成 Query JSON")?);
    result.push('\n');
    Ok(result)
}
