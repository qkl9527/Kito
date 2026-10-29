//! RFC 4648 编解码：字母表、填充和兼容空白策略彼此独立。
use anyhow::{Context, Result};
use base64::{
    Engine, alphabet,
    engine::{
        DecodePaddingMode,
        general_purpose::{GeneralPurpose, GeneralPurposeConfig},
    },
};
use clap::ValueEnum;

#[derive(Clone, Copy, ValueEnum)]
pub enum Variant {
    Standard,
    UrlSafe,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum EncodePadding {
    Padded,
    Unpadded,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum DecodePadding {
    Auto,
    Padded,
    Unpadded,
}

fn engine(variant: Variant, padded: bool, decode: DecodePaddingMode) -> GeneralPurpose {
    let alphabet = match variant {
        Variant::Standard => &alphabet::STANDARD,
        Variant::UrlSafe => &alphabet::URL_SAFE,
    };
    GeneralPurpose::new(
        alphabet,
        GeneralPurposeConfig::new()
            .with_encode_padding(padded)
            .with_decode_padding_mode(decode)
            .with_decode_allow_trailing_bits(false),
    )
}

pub fn encode(input: &[u8], variant: Variant, padding: EncodePadding) -> String {
    engine(
        variant,
        matches!(padding, EncodePadding::Padded),
        DecodePaddingMode::RequireCanonical,
    )
    .encode(input)
}

pub fn decode(
    input: &[u8],
    variant: Variant,
    padding: DecodePadding,
    ignore_whitespace: bool,
) -> Result<Vec<u8>> {
    let filtered;
    let input = if ignore_whitespace {
        // 仅允许文档列明的四种 ASCII 空白；垂直 TAB、换页及 Unicode 空白仍报错。
        filtered = input
            .iter()
            .copied()
            .filter(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
            .collect::<Vec<_>>();
        filtered.as_slice()
    } else {
        input
    };
    // auto 在两个严格模式间选择，而非使用 Indifferent，避免接受少一个 '=' 的坏填充。
    let mode = match padding {
        DecodePadding::Padded => DecodePaddingMode::RequireCanonical,
        DecodePadding::Unpadded => DecodePaddingMode::RequireNone,
        DecodePadding::Auto if input.ends_with(b"=") => DecodePaddingMode::RequireCanonical,
        DecodePadding::Auto => DecodePaddingMode::RequireNone,
    };
    engine(variant, true, mode)
        .decode(input)
        .context("Base64 输入不符合所选字母表、填充或尾部位规则")
}
