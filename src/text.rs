//! 文本统计能力：默认统计 Unicode scalar value，必要时统计 UTF-8 字节数。

use anyhow::{Context, Result};
use std::{collections::HashSet, path::Path};

/// 按 Unicode 字符或 UTF-8 字节统计文本长度。
///
/// Rust 的 `char` 对应一个 Unicode scalar value，因此一个汉字或 emoji
/// （不含组合序列）计为一个字符；`--bytes` 模式则统计 UTF-8 编码后的字节数。
pub fn count(text: &str, bytes: bool) -> usize {
    if bytes {
        text.len()
    } else {
        text.chars().count()
    }
}

/// 读取 UTF-8 文本文件；解码失败时将文件路径带入错误，避免静默替换字符。
pub fn read_utf8_file(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("无法读取 UTF-8 文本文件：{}", path.display()))
}

/// 按行去重，保留每行首次出现的位置、原始行尾和文件末尾是否有换行符。
///
/// LF 与 CRLF 都作为换行符处理，比较时忽略 CRLF 中的 `\r`，输出仍保留
/// 首次出现那一行的原始字节序列；空行也作为普通行参与去重。
pub fn deduplicate_lines(text: &str) -> String {
    let mut seen = HashSet::new();
    let mut unique = String::with_capacity(text.len());

    for line_with_ending in text.split_inclusive('\n') {
        // 仅在比较键中去掉行尾，结果仍追加原文，避免改写换行格式。
        let key = match line_with_ending.strip_suffix('\n') {
            Some(line) => line.strip_suffix('\r').unwrap_or(line),
            None => line_with_ending,
        };
        if seen.insert(key) {
            unique.push_str(line_with_ending);
        }
    }

    unique
}

/// 仅将文本的首个 Unicode scalar value 转成大写，其余内容保持原样。
///
/// 使用 `char::to_uppercase` 而不是 ASCII 转换，因此首字符可能展开为多个字符，
/// 例如德语 `ß` 会转换成 `SS`；空文本保持为空。
pub fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    let mut result = String::with_capacity(text.len());
    result.extend(first.to_uppercase());
    result.extend(chars);
    result
}

/// 按空白分隔单词：每个词首字符转大写，其余字符转小写，并保留原空白。
pub fn capitalize_words(text: &str) -> String {
    let mut at_word_start = true;
    let mut result = String::with_capacity(text.len());

    for ch in text.chars() {
        if ch.is_whitespace() {
            at_word_start = true;
            result.push(ch);
        } else if at_word_start {
            result.extend(ch.to_uppercase());
            at_word_start = false;
        } else {
            result.extend(ch.to_lowercase());
        }
    }

    result
}
