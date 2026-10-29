//! 基于完整 HTML5 实体集合的严格转换，不采用浏览器 tokenizer 的容错替换。
use anyhow::{Context as _, Result, ensure};
use clap::ValueEnum;
use std::{collections::HashMap, sync::OnceLock};

#[derive(Clone, Copy, ValueEnum)]
pub enum Standard {
    Html5,
    Xml,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Style {
    Named,
    Decimal,
    Hex,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Context {
    Text,
    Attribute,
}

fn validate(character: char, standard: Standard, position: usize) -> Result<()> {
    let code = character as u32;
    let valid = match standard {
        Standard::Xml => {
            matches!(code, 0x9 | 0xa | 0xd | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
        }
        // C1 按原 Unicode scalar 保留，不作 HTML 历史 Windows-1252 重映射。
        // 其他 C0/DEL 和所有 noncharacter 拒绝，避免宽松库静默替换损坏输入。
        Standard::Html5 => {
            (code >= 0x20 || matches!(code, 0x9 | 0xa | 0xd))
                && code != 0x7f
                && !(0xfdd0..=0xfdef).contains(&code)
                && code & 0xffff != 0xfffe
                && code & 0xffff != 0xffff
        }
    };
    ensure!(
        valid,
        "字节位置 {position} 的 U+{code:04X} 不符合所选标准的字符范围"
    );
    Ok(())
}

fn named(name: &str, standard: Standard) -> Option<&'static str> {
    match standard {
        Standard::Xml => match name {
            "amp" => Some("&"),
            "lt" => Some("<"),
            "gt" => Some(">"),
            "quot" => Some("\""),
            "apos" => Some("'"),
            _ => None,
        },
        Standard::Html5 => super::entities::NAMED_ENTITIES
            .binary_search_by(|(key, _)| key.cmp(&name.as_bytes()))
            .ok()
            .map(|i| super::entities::NAMED_ENTITIES[i].1),
    }
}

fn numeric(body: &str, position: usize) -> Result<char> {
    let (digits, radix) = match body.strip_prefix('x').or_else(|| body.strip_prefix('X')) {
        Some(digits) => (digits, 16),
        None => (body, 10),
    };
    ensure!(
        !digits.is_empty()
            && digits.bytes().all(|c| if radix == 16 {
                c.is_ascii_hexdigit()
            } else {
                c.is_ascii_digit()
            }),
        "字节位置 {position} 的数字实体格式非法"
    );
    let code = u32::from_str_radix(digits, radix)
        .with_context(|| format!("字节位置 {position} 的数字实体超出范围"))?;
    char::from_u32(code)
        .with_context(|| format!("字节位置 {position} 的实体不是有效 Unicode scalar"))
}

pub fn decode(input: &str, standard: Standard) -> Result<String> {
    let mut result = String::with_capacity(input.len());
    let mut position = 0;
    while position < input.len() {
        let remaining = &input[position..];
        let character = remaining.chars().next().context("实体扫描位置非法")?;
        if character == '&'
            && remaining
                .as_bytes()
                .get(1)
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'#')
        {
            // 扫描原输入一次；追加到结果的 '&lt;' 不会再次进入解码器。
            let end = remaining
                .find(';')
                .with_context(|| format!("字节位置 {position} 的实体缺少分号"))?;
            let body = &remaining[1..end];
            if let Some(number) = body.strip_prefix('#') {
                // XML 1.0 CharRef 语法只允许小写 x，HTML5 另允许大写 X。
                ensure!(
                    !matches!(standard, Standard::Xml) || !number.starts_with('X'),
                    "字节位置 {position} 的 XML 十六进制引用必须使用小写 x"
                );
                let scalar = numeric(number, position)?;
                validate(scalar, standard, position)?;
                result.push(scalar);
            } else {
                let value = named(body, standard)
                    .with_context(|| format!("字节位置 {position} 的命名实体未知"))?;
                for scalar in value.chars() {
                    validate(scalar, standard, position)?;
                }
                result.push_str(value);
            }
            position += end + 1;
        } else {
            validate(character, standard, position)?;
            result.push(character);
            position += character.len_utf8();
        }
    }
    Ok(result)
}

struct ReverseEntities {
    names: HashMap<&'static str, &'static str>,
    max_bytes: usize,
}

fn reverse_entities() -> &'static ReverseEntities {
    static TABLE: OnceLock<ReverseEntities> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut names: HashMap<&str, &str> = HashMap::new();
        let mut max_bytes = 0;
        for (bytes, value) in super::entities::NAMED_ENTITIES {
            let Ok(name) = std::str::from_utf8(bytes) else {
                continue;
            };
            max_bytes = max_bytes.max(value.len());
            // 同值的别名确定地选择最短名称，再按字典序；五个基础实体统一用小写。
            if names
                .get(value)
                .is_none_or(|old| (name.len(), name) < (old.len(), *old))
            {
                names.insert(value, name);
            }
        }
        for (value, name) in [
            ("&", "amp"),
            ("<", "lt"),
            (">", "gt"),
            ("\"", "quot"),
            ("'", "apos"),
        ] {
            names.insert(value, name);
        }
        ReverseEntities { names, max_bytes }
    })
}

fn core_name(character: char) -> Option<&'static str> {
    match character {
        '&' => Some("amp"),
        '<' => Some("lt"),
        '>' => Some("gt"),
        '"' => Some("quot"),
        '\'' => Some("apos"),
        _ => None,
    }
}

fn encode_scalar(result: &mut String, character: char, style: Style, standard: Standard) {
    if matches!(style, Style::Named) {
        let name = match standard {
            Standard::Xml => core_name(character),
            Standard::Html5 => reverse_entities()
                .names
                .get(character.encode_utf8(&mut [0; 4]))
                .copied(),
        };
        if let Some(name) = name {
            result.push_str(&format!("&{name};"));
            return;
        }
    }
    result.push_str(&match style {
        Style::Hex => format!("&#x{:X};", character as u32),
        _ => format!("&#{};", character as u32),
    });
}

pub fn encode(
    input: &str,
    standard: Standard,
    style: Style,
    context: Context,
    all: bool,
) -> Result<String> {
    // 先完整校验，再构造结果；CLI 只会在整个转换成功后发布字节。
    for (position, character) in input.char_indices() {
        validate(character, standard, position)?;
    }
    let mut result = String::with_capacity(input.len());
    let mut position = 0;
    while position < input.len() {
        let remaining = &input[position..];
        let character = remaining.chars().next().context("实体扫描位置非法")?;
        if all && matches!(standard, Standard::Html5) && matches!(style, Style::Named) {
            let table = reverse_entities();
            // 最长匹配保留多码点实体；边界来自 UTF-8 char_indices，不切开字符。
            let found = remaining
                .char_indices()
                .map(|(i, c)| i + c.len_utf8())
                .take_while(|end| *end <= table.max_bytes)
                .filter_map(|end| table.names.get(&remaining[..end]).map(|name| (end, *name)))
                .last();
            if let Some((end, name)) = found {
                result.push_str(&format!("&{name};"));
                position += end;
                continue;
            }
        }
        let attribute = matches!(context, Context::Attribute);
        let xml_whitespace = matches!(standard, Standard::Xml)
            && attribute
            && matches!(character, '\t' | '\n' | '\r');
        if all
            || matches!(character, '&' | '<' | '>')
            || attribute && matches!(character, '"' | '\'')
            || xml_whitespace
        {
            encode_scalar(&mut result, character, style, standard);
        } else {
            result.push(character);
        }
        position += character.len_utf8();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_all_encoding_preserves_multiscalar_values() {
        let encoded = encode(
            "fj≂\u{338}",
            Standard::Html5,
            Style::Named,
            Context::Attribute,
            true,
        )
        .unwrap();
        assert_eq!(encoded, "&fjlig;&nesim;");
        assert_eq!(decode(&encoded, Standard::Html5).unwrap(), "fj≂\u{338}");
    }

    #[test]
    fn all_standard_named_entities_survive_the_encoding_round_trip() {
        // fixture 是完整标准表，回环覆盖别名、组合字符和补充平面，避免仅测基础五项。
        for (name, value) in super::super::entities::NAMED_ENTITIES {
            let entity = format!("&{};", std::str::from_utf8(name).unwrap());
            assert_eq!(decode(&entity, Standard::Html5).unwrap(), value, "{entity}");
            let encoded = encode(
                value,
                Standard::Html5,
                Style::Named,
                Context::Attribute,
                true,
            )
            .unwrap();
            assert_eq!(
                decode(&encoded, Standard::Html5).unwrap(),
                value,
                "{entity}"
            );
        }
    }

    #[test]
    fn c1_is_preserved_xml_uses_its_own_char_range_and_offsets_are_bytes() {
        assert_eq!(
            decode("&#x80;&#159;", Standard::Html5).unwrap(),
            "\u{80}\u{9f}"
        );
        assert_eq!(
            decode("&#xFDD0;&#x1FFFF;", Standard::Xml).unwrap(),
            "\u{fdd0}\u{1ffff}"
        );
        assert!(decode("&#xFDD0;", Standard::Html5).is_err());
        assert!(decode("&#x7F;", Standard::Html5).is_err());
        assert!(
            decode("你好&unknown;", Standard::Html5)
                .unwrap_err()
                .to_string()
                .contains("字节位置 6")
        );
    }
}
