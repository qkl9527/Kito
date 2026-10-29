//! 标识领域的 CLI 适配，复用共享 IO 的输入互斥和文件原子发布约束。

use super::{hash, uuid};
use crate::io_args::{InputArgs, OutputArgs, write_stdout};
use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
#[command(after_help = r#"使用案例:
  计算文本的 SHA-256 摘要：
    kito id hash "hello"

  批量生成带 Unix 毫秒时间字段的 UUID v7：
    kito id uuid --version v7 --count 10"#)]
pub struct IdArgs {
    #[command(subcommand)]
    command: IdCommand,
}

#[derive(Subcommand)]
enum IdCommand {
    #[command(
        about = "计算字符串和文件的 Hash；MD5/SHA-1 仅用于兼容，不适用于安全校验",
        after_help = r#"使用案例:
  计算 UTF-8 文本的 SHA-256 摘要，输入不额外加入换行：
    kito id hash "hello"

  按原始字节计算文件的 SHA-512，并保存摘要：
    kito id hash --file "archive.zip" --algorithm sha512 -o "archive.sha512"

  将摘要表示为带填充的 standard Base64：
    kito id hash "hello" --format base64

  输出包含算法、摘要、输入字节数和来源类型的 JSON 报告：
    kito id hash --file "archive.zip" --json"#
    )]
    Hash {
        #[command(flatten)]
        input: InputArgs,
        /// 摘要算法：sha224/sha256/sha384/sha512 属于 SHA-2；md5/sha1 仅用于兼容已有数据。
        #[arg(long, value_enum, default_value = "sha256")]
        algorithm: hash::Algorithm,
        /// 摘要表示：hex 为小写十六进制；base64 使用 standard 字母表并保留填充。
        #[arg(long, value_enum, default_value = "hex")]
        format: hash::Format,
        #[arg(
            long,
            conflicts_with = "output",
            help = "输出算法、格式、摘要、输入字节数和来源类型的 JSON 报告；不包含原始输入，与 --output 互斥"
        )]
        json: bool,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(
        about = "生成 UUID v4/v7；UUID 不是密钥或授权凭证，不保证绝对不重复",
        after_help = r#"使用案例:
  生成一个随机 UUID v4，默认小写并带连字符：
    kito id uuid

  生成十个含 Unix 毫秒时间字段的 UUID v7，每行一个：
    kito id uuid --version v7 --count 10

  生成无连字符的 UUID，并输出完整 JSON 报告：
    kito id uuid --version v4 --format simple --json

  将二十个带 urn:uuid: 前缀的 UUID 保存到新文件：
    kito id uuid --format urn --count 20 -o "ids.txt""#
    )]
    Uuid {
        /// UUID 版本：v4 使用随机位；v7 含 Unix 毫秒时间字段和随机位，不承诺严格递增。
        #[arg(long, value_enum, default_value = "v4")]
        version: uuid::Version,
        /// 表示形式：hyphenated 带连字符；simple 不带连字符；urn 增加 urn:uuid: 前缀。
        #[arg(long, value_enum, default_value = "hyphenated")]
        format: uuid::Format,
        /// 生成数量，允许 1–10000；普通输出每行一个，整批生成成功后才输出。
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..=10000))]
        count: u16,
        /// 输出 version、format 和 values 数组的 JSON 报告；单个 UUID 也使用数组，与 --output 互斥。
        #[arg(long, conflicts_with = "output")]
        json: bool,
        #[command(flatten)]
        output: OutputArgs,
    },
}

pub fn run(args: IdArgs) -> Result<bool> {
    match args.command {
        IdCommand::Hash {
            input,
            algorithm,
            format,
            json,
            output,
        } => {
            let (mut reader, source_kind) = input.open()?;
            let report = hash::calculate(reader.as_mut(), source_kind, algorithm, format)?;
            // Windows 上发布前关闭输入句柄；同文件保护仍由 OutputArgs 按路径身份执行。
            drop(reader);
            if json {
                write_report(&report)?;
            } else {
                output.write(&input, format!("{}\n", report.digest).as_bytes(), false)?;
            }
        }
        IdCommand::Uuid {
            version,
            format,
            count,
            json,
            output,
        } => {
            let report = uuid::generate(version, format, count)?;
            if json {
                write_report(&report)?;
            } else {
                let mut result = report.values.join("\n");
                result.push('\n');
                output.write(&InputArgs::default(), result.as_bytes(), false)?;
            }
        }
    }
    Ok(true)
}

fn write_report(report: &impl serde::Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    write_stdout(&bytes)
}
