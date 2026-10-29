//! Codi 的 clap 适配；转换始终在输出前完整完成，错误不会留下部分结果。
use super::{base64, html, url};
use crate::io_args::{InputArgs, OutputArgs, write_stdout};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;

#[derive(Args)]
#[command(after_help = r#"使用案例:
  将文本编码为 Base64：
    kito codec base64 encode "你好，Kito"

  解析 URL，并展示重复参数、空值和可复制的 Query JSON：
    kito codec url parse "https://example.com/?tag=a&tag=b&empty=&flag"

  将 HTML 实体还原为文本：
    kito codec html decode "&lt;b&gt;你好&lt;/b&gt;""#)]
pub struct CodecArgs {
    #[command(subcommand)]
    command: CodecCommand,
}

#[derive(Subcommand)]
enum CodecCommand {
    #[command(
        about = "RFC 4648 Base64 编码/解码，支持 standard 与 url-safe 字母表",
        after_help = r#"使用案例:
  将图片文件编码后保存到新文件：
    kito codec base64 encode --file "image.png" -o "image.b64"

  将 Base64 文件解码为原始字节：
    kito codec base64 decode --file "image.b64" -o "restored.png""#
    )]
    Base64 {
        #[command(subcommand)]
        command: Base64Command,
    },
    #[command(
        about = "URL 参数解析，保留重复参数和次序",
        after_help = r#"使用案例:
  展示完整 URL 的组件、参数和可复制的 Query JSON：
    kito codec url parse "https://example.com/search?q=hello+world&tag=a&tag=b"

  将原始查询串解析为完整 JSON 报告：
    kito codec url parse "x=&flag&x=2" --query --json"#
    )]
    Url {
        #[command(subcommand)]
        command: UrlCommand,
    },
    #[command(
        about = "基于完整 HTML5 实体集合或 XML 1.0 的严格转换",
        after_help = r#"使用案例:
  将文本中的 HTML 特殊字符编码为实体：
    kito codec html encode "<b>你好 & Kito</b>" --context text

  按 XML 1.0 规则严格解码一层实体：
    kito codec html decode "&lt;tag&gt;Kito&lt;/tag&gt;" --standard xml"#
    )]
    Html {
        #[command(subcommand)]
        command: HtmlCommand,
    },
}

#[derive(Args)]
struct IoArgs {
    #[command(flatten)]
    input: InputArgs,
    #[command(flatten)]
    output: OutputArgs,
    /// 输入上限（MiB，1 MiB = 1048576 字节），读取过程中也检查。
    #[arg(long, default_value_t = 64)]
    max_input_mib: usize,
}
impl IoArgs {
    fn limit(&self) -> Result<usize> {
        self.max_input_mib
            .checked_mul(1024 * 1024)
            .context("--max-input-mib 过大")
    }
    fn read_bytes(&self) -> Result<Vec<u8>> {
        self.input.read_bytes(self.limit()?)
    }
    fn read_utf8(&self) -> Result<String> {
        self.input.read_utf8(self.limit()?)
    }
    fn write(&self, bytes: &[u8], guard: bool) -> Result<()> {
        self.output.write(&self.input, bytes, guard)
    }
}

#[derive(Subcommand)]
enum Base64Command {
    #[command(
        about = "将原始字节编码为 Base64，不自动增加换行",
        after_help = r#"使用案例:
  使用默认 standard 字母表和 padded 填充编码文本：
    kito codec base64 encode "你好，Kito"

  使用 URL-safe 字母表并明确省略末尾的 = 填充：
    kito codec base64 encode "hello" --variant url-safe --padding unpadded

  编码文件原始字节并保存结果：
    kito codec base64 encode --file "image.png" -o "image.b64""#
    )]
    Encode {
        #[command(flatten)]
        io: IoArgs,
        /// 字母表：standard 使用 +/；url-safe 使用 -_；选择 url-safe 不会自动去掉填充。
        #[arg(long, value_enum, default_value = "standard")]
        variant: base64::Variant,
        /// 编码填充：padded 按需补齐末尾 =；unpadded 省略末尾 =。
        #[arg(long, value_enum, default_value = "padded")]
        padding: base64::EncodePadding,
    },
    #[command(
        about = "严格解码为原始字节；二进制请保存到文件或重定向",
        after_help = r#"使用案例:
  解码 standard 字母表的文本，默认自动接受规范的有填充或无填充形式：
    kito codec base64 decode "aGVsbG8="

  明确按 URL-safe 字母表和无填充规则解码：
    kito codec base64 decode "aGVsbG8" --variant url-safe --padding unpadded

  忽略文件中的 ASCII 空白，将二进制结果保存到新文件：
    kito codec base64 decode --file "image.b64" --ignore-whitespace -o "restored.png""#
    )]
    Decode {
        #[command(flatten)]
        io: IoArgs,
        /// 输入字母表：standard 使用 +/；url-safe 使用 -_；不自动猜测或混用。
        #[arg(long, value_enum, default_value = "standard")]
        variant: base64::Variant,
        /// 解码填充：auto 接受规范的有/无填充形式；padded 要求规范填充；unpadded 禁止 =。
        #[arg(long, value_enum, default_value = "auto")]
        padding: base64::DecodePadding,
        /// 仅忽略 ASCII 空格、TAB、CR、LF。
        #[arg(long)]
        ignore_whitespace: bool,
    },
}

#[derive(Args)]
struct UrlArgs {
    #[command(flatten)]
    io: IoArgs,
    /// 明确将输入当作原始 query（可带开头 ?），不猜测 URL。
    #[arg(long)]
    query: bool,
    /// form 将 + 转为空格；percent 保留 +；两者均严格解码一次。
    #[arg(long, value_enum, default_value = "form")]
    mode: url::Mode,
    /// 输出完整 JSON 报告（含有序 parameters 数组）；默认展示另附 Query JSON 对象，与 --output 互斥。
    #[arg(long, conflicts_with = "output")]
    json: bool,
}
#[derive(Subcommand)]
enum UrlCommand {
    #[command(
        about = "展示规范化 URL 组件、有序参数和可复制的 Query JSON 对象",
        after_help = r#"使用案例:
  解析完整 URL，保留重复参数、空值和无等号参数：
    kito codec url parse "https://example.com/?tag=a&tag=b&empty=&flag"

  按原始查询串解析，使用 percent 模式保留 +：
    kito codec url parse "?q=a+b&empty=" --query --mode percent

  解析空查询串，默认 Query JSON 为 {}：
    kito codec url parse "" --query

  从 UTF-8 文件读取 URL（文件内容不带结尾换行），输出完整报告：
    kito codec url parse --file "url.txt" --json"#
    )]
    Parse {
        #[command(flatten)]
        args: UrlArgs,
    },
}

#[derive(Args)]
struct HtmlArgs {
    #[command(flatten)]
    io: IoArgs,
    /// 实体与字符规则：html5 使用完整 HTML5 实体集合；xml 使用 XML 1.0 五个预定义实体及数字引用。
    #[arg(long, value_enum, default_value = "html5")]
    standard: html::Standard,
}
#[derive(Subcommand)]
enum HtmlCommand {
    #[command(
        about = "编码实体；style 只选择表示方式，--all 才编码全部字符",
        after_help = r#"使用案例:
  编码 HTML 文本中的 &、<、>，保留普通字符：
    kito codec html encode "<b>你好 & Kito</b>" --context text

  将全部字符编码为十六进制数字引用：
    kito codec html encode "你好 & Kito" --style hex --all

  按 XML 1.0 属性值规则编码 UTF-8 文件并保存：
    kito codec html encode --file "attribute.txt" --standard xml --context attribute -o "attribute.xml.txt""#
    )]
    Encode {
        #[command(flatten)]
        args: HtmlArgs,
        /// 实体表示：named 优先名称、无名称时用十进制；decimal 用十进制；hex 用十六进制。
        #[arg(long, value_enum, default_value = "named")]
        style: html::Style,
        /// 编码范围：text 处理 &、<、>；attribute 额外处理单双引号及 XML 属性中的 TAB/LF/CR。
        #[arg(long, value_enum, default_value = "attribute")]
        context: html::Context,
        /// 编码全部字符；未指定时只编码所选 context 要求的特殊字符。
        #[arg(long)]
        all: bool,
    },
    #[command(
        about = "严格解码一层，要求实体带分号，不处理 DTD 或外部实体",
        after_help = r#"使用案例:
  解码 HTML5 命名实体和数字引用：
    kito codec html decode "&lt;b&gt;你好&#33;&lt;/b&gt;"

  只解码一层，结果保留为 &lt;：
    kito codec html decode "&amp;lt;"

  按 XML 1.0 规则解码 UTF-8 文件并保存：
    kito codec html decode --file "entities.txt" --standard xml -o "decoded.txt""#
    )]
    Decode {
        #[command(flatten)]
        args: HtmlArgs,
    },
}

fn json<T: Serialize>(value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    write_stdout(&bytes)
}

pub fn run(args: CodecArgs) -> Result<bool> {
    match args.command {
        CodecCommand::Base64 { command } => match command {
            Base64Command::Encode {
                io,
                variant,
                padding,
            } => io.write(
                base64::encode(&io.read_bytes()?, variant, padding).as_bytes(),
                false,
            )?,
            Base64Command::Decode {
                io,
                variant,
                padding,
                ignore_whitespace,
            } => io.write(
                &base64::decode(&io.read_bytes()?, variant, padding, ignore_whitespace)?,
                true,
            )?,
        },
        CodecCommand::Html { command } => match command {
            HtmlCommand::Encode {
                args,
                style,
                context,
                all,
            } => args.io.write(
                html::encode(&args.io.read_utf8()?, args.standard, style, context, all)?.as_bytes(),
                false,
            )?,
            HtmlCommand::Decode { args } => args.io.write(
                html::decode(&args.io.read_utf8()?, args.standard)?.as_bytes(),
                false,
            )?,
        },
        CodecCommand::Url {
            command: UrlCommand::Parse { args },
        } => {
            let report = url::parse(&args.io.read_utf8()?, args.query, args.mode)?;
            if args.json {
                json(&report)?;
            } else {
                args.io.write(url::display(&report)?.as_bytes(), false)?;
            }
        }
    }
    Ok(true)
}
