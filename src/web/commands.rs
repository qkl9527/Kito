//! Web 结构化请求到 CLI 的唯一边界：只接受声明过的能力，绝不接受路径或 argv。
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    pub tool: String,
    pub action: String,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    #[serde(default)]
    pub files: Vec<String>,
}
pub struct PreparedCommand {
    pub args: Vec<OsString>,
    pub stdin: Option<Vec<u8>>,
}

/// 参数读取同时登记消费项；最后统一拒绝遗漏项，防止未来误开放 CLI 参数。
struct Builder<'a> {
    params: BTreeMap<&'a str, &'a str>,
    args: Vec<OsString>,
    positional: Vec<OsString>,
    stdin: Option<Vec<u8>>,
    text: Option<Vec<u8>>,
}
impl<'a> Builder<'a> {
    fn take(&mut self, name: &str) -> Option<&'a str> {
        self.params.remove(name)
    }
    fn value(&mut self, name: &str, default: &'a str) -> &'a str {
        self.take(name).unwrap_or(default)
    }
    fn arg(&mut self, value: impl Into<OsString>) {
        self.args.push(value.into());
    }
    fn option(&mut self, name: &str, value: &str) {
        self.arg(format!("--{}={value}", name.replace('_', "-")));
    }
    fn choice(&mut self, name: &str, default: &'a str, choices: &[&str]) -> Result<&'a str> {
        let value = self.value(name, default);
        ensure!(choices.contains(&value), "参数 {name} 的取值不受支持");
        self.option(name, value);
        Ok(value)
    }
    fn number(
        &mut self,
        name: &str,
        default: Option<&'a str>,
        min: f64,
        max: f64,
        integer: bool,
    ) -> Result<()> {
        if let Some(value) = self.take(name).or(default) {
            let number: f64 = value
                .parse()
                .with_context(|| format!("参数 {name} 必须是数字"))?;
            ensure!(
                number.is_finite()
                    && (min..=max).contains(&number)
                    && (!integer || value.parse::<u64>().is_ok()),
                "参数 {name} 超出允许范围 {min}–{max}"
            );
            self.option(name, value);
        }
        Ok(())
    }
    fn flag(&mut self, name: &str) -> Result<()> {
        match self.take(name) {
            None | Some("false") => {}
            Some("true") => self.arg(format!("--{}", name.replace('_', "-"))),
            _ => bail!("参数 {name} 必须是 true 或 false"),
        };
        Ok(())
    }
    fn optional(&mut self, name: &str) {
        if let Some(value) = self.take(name) {
            self.option(name, value);
        }
    }
    fn required(&mut self, name: &str) -> Result<&'a str> {
        self.take(name)
            .filter(|v| !v.is_empty())
            .with_context(|| format!("缺少参数 {name}"))
    }
    fn output(&mut self, path: PathBuf) {
        self.arg("--output");
        self.arg(path.into_os_string());
    }
    fn source(&mut self, inputs: &[PathBuf]) -> Result<()> {
        match (self.take("text"), inputs) {
            (Some(text), []) => self.text = Some(text.as_bytes().to_vec()),
            (None, [path]) => {
                self.arg("--file");
                self.arg(path.as_os_str());
            }
            _ => bail!("请提供文本或一个上传文件，二者不能同时提供"),
        }
        Ok(())
    }
    fn encoding(&mut self, default: &'a str) -> Result<()> {
        self.choice("quality", default, &["high", "balanced", "small"])?;
        self.choice("preset", "medium", &["fast", "medium", "slow"])?;
        Ok(())
    }
}

pub fn prepare(
    request: &RunRequest,
    inputs: &[PathBuf],
    output_dir: &Path,
) -> Result<PreparedCommand> {
    ensure!(
        request.params.len() <= 32
            && request
                .params
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
                <= 65536,
        "文本参数超过 64 KiB 或参数过多"
    );
    ensure!(inputs.len() <= 2, "上传文件数量过多");
    // 服务负责解析上传 ID；适配层只使用服务给出的私有绝对路径，拒绝符号链接。
    for path in inputs {
        ensure!(path.is_absolute(), "上传路径必须是服务器私有绝对路径");
        let meta = std::fs::symlink_metadata(path).context("无法读取上传文件")?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "上传必须是普通文件"
        );
    }
    let mut b = Builder {
        params: request
            .params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect(),
        args: vec![request.tool.clone().into()],
        positional: vec![],
        stdin: None,
        text: None,
    };
    let action = request.action.as_str();
    let one = || -> Result<&PathBuf> {
        ensure!(inputs.len() == 1, "此操作需要一个上传文件");
        Ok(&inputs[0])
    };
    let none = || -> Result<()> {
        ensure!(inputs.is_empty(), "此操作不接受上传文件");
        Ok(())
    };
    match request.tool.as_str() {
        "codec" => {
            let (family, operation) = action.split_once('-').context("不支持的编解码操作")?;
            ensure!(
                matches!(
                    (family, operation),
                    ("base64", "encode" | "decode")
                        | ("url", "parse")
                        | ("html", "encode" | "decode")
                ),
                "不支持的编解码操作"
            );
            b.arg(family);
            b.arg(operation);
            b.source(inputs)?;
            match family {
                "base64" => {
                    b.choice("variant", "standard", &["standard", "url-safe"])?;
                    if operation == "encode" {
                        b.choice("padding", "padded", &["padded", "unpadded"])?;
                    } else {
                        b.choice("padding", "auto", &["auto", "padded", "unpadded"])?;
                        b.flag("ignore_whitespace")?;
                    }
                }
                "url" => {
                    b.choice("mode", "form", &["form", "percent"])?;
                    b.flag("query")?;
                }
                _ => {
                    b.choice("standard", "html5", &["html5", "xml"])?;
                    if operation == "encode" {
                        b.choice("style", "named", &["named", "decimal", "hex"])?;
                        b.choice("context", "attribute", &["attribute", "text"])?;
                        b.flag("all")?;
                    }
                }
            }
            b.output(
                output_dir.join(if family == "base64" && operation == "decode" {
                    "decoded.bin"
                } else {
                    "result.txt"
                }),
            );
        }
        "text" => {
            ensure!(
                ["count", "dedupe", "case"].contains(&action),
                "不支持的文本操作"
            );
            b.arg(action);
            if action == "case" {
                let mode = b.value("mode", "lower");
                ensure!(
                    ["upper", "lower", "capitalize"].contains(&mode),
                    "不支持的大小写模式"
                );
                b.positional.push(mode.into());
                b.flag("words")?;
            }
            b.source(inputs)?;
            if action == "count" {
                b.flag("bytes")?;
            } else {
                b.output(output_dir.join("result.txt"));
            }
        }
        "id" => {
            b.arg(action);
            match action {
                "hash" => {
                    b.source(inputs)?;
                    b.choice(
                        "algorithm",
                        "sha256",
                        &["md5", "sha1", "sha224", "sha256", "sha384", "sha512"],
                    )?;
                    b.choice("format", "hex", &["hex", "base64"])?;
                    b.arg("--json");
                }
                "uuid" => {
                    none()?;
                    b.choice("version", "v4", &["v4", "v7"])?;
                    b.choice("format", "hyphenated", &["hyphenated", "simple", "urn"])?;
                    b.number("count", Some("1"), 1., 100., true)?;
                    b.arg("--json");
                }
                _ => bail!("不支持的标识操作"),
            }
        }
        "time" => {
            none()?;
            b.arg(action);
            b.arg("--json");
            match action {
                "now" => {
                    let tz = b.value("timezone", "UTC");
                    b.option("timezone", tz);
                    if b.params.contains_key("unit") {
                        b.choice("unit", "s", &["s", "ms", "us", "ns"])?;
                    }
                    b.optional("format");
                }
                "from-timestamp" => {
                    let value = b.required("value")?;
                    value.parse::<i128>().context("时间戳必须是整数")?;
                    b.positional.push(value.into());
                    b.choice("unit", "auto", &["auto", "s", "ms", "us", "ns"])?;
                    let tz = b.value("timezone", "UTC");
                    b.option("timezone", tz);
                    b.optional("format");
                }
                "to-timestamp" | "convert" => {
                    let value = b.required("datetime")?;
                    b.positional.push(value.into());
                    if action == "convert" {
                        b.optional("from_timezone");
                        let tz = b.value("to_timezone", "UTC");
                        b.option("to_timezone", tz);
                    } else {
                        b.optional("timezone");
                        b.choice("unit", "s", &["s", "ms", "us", "ns"])?;
                    }
                    b.optional("format");
                }
                "zones" => {
                    b.optional("at");
                    if let Some(filter) = b.take("filter") {
                        b.positional.push(filter.into());
                    }
                }
                _ => bail!("不支持的时间操作"),
            }
        }
        "key" => {
            b.arg(action);
            b.arg("--json");
            match action {
                "generate" => {
                    none()?;
                    let algorithm = b.choice("algorithm", "rsa", &["rsa", "ed25519"])?;
                    if algorithm == "rsa" {
                        b.choice("bits", "3072", &["2048", "3072", "4096"])?;
                    }
                    b.arg("--out-dir");
                    b.arg(output_dir.join("keys").into_os_string());
                }
                "public" | "check" => {
                    ensure!(
                        !inputs.is_empty() && inputs.len() <= if action == "check" { 2 } else { 1 },
                        "私钥操作需要上传私钥，可选上传配对公钥"
                    );
                    b.positional.push(inputs[0].as_os_str().to_owned());
                    if action == "public" {
                        b.output(output_dir.join("public.pem"));
                    }
                    if inputs.len() == 2 {
                        b.arg("--public");
                        b.arg(inputs[1].as_os_str());
                    }
                }
                _ => bail!("不支持的密钥操作"),
            }
            // 口令永远不进入 argv、工作文件或报告；默认无口令必须显式通知生成器。
            match b.take("passphrase") {
                Some(p) if !p.is_empty() => {
                    ensure!(
                        p.len() <= 1023 && !p.bytes().any(|x| matches!(x, 0 | b'\r' | b'\n')),
                        "口令必须是 1–1023 字节单行文本"
                    );
                    b.arg("--passphrase-stdin");
                    b.stdin = Some(p.as_bytes().to_vec());
                }
                _ if action == "generate" => b.arg("--no-passphrase"),
                _ => {}
            }
        }
        "image" => {
            let input = one()?;
            ensure!(
                ["compress", "convert", "resize", "info", "exif"].contains(&action),
                "不支持的图片操作"
            );
            b.arg(action);
            b.positional.push(input.as_os_str().to_owned());
            b.arg("--json");
            if action != "info" {
                b.option("memory_mib", "256");
            }
            match action {
                "compress" => {
                    b.choice("effort", "max", &["fast", "max"])?;
                    b.flag("strip_metadata")?;
                    b.optional("resize");
                    b.option("time_limit", "15");
                    b.output(output_dir.join("compressed.png"));
                }
                "convert" | "resize" => {
                    let to = b.value("to", "png");
                    ensure!(
                        ["png", "webp", "jpeg", "avif", "heic", "heif", "tiff", "bmp"]
                            .contains(&to),
                        "不支持的图片格式"
                    );
                    b.option("to", to);
                    if action == "resize" {
                        let size = b.required("resize")?;
                        b.option("resize", size);
                    } else {
                        b.optional("resize");
                    }
                    b.flag("allow_lossy")?;
                    b.number("quality", None, 1., 100., true)?;
                    b.option("time_limit", "30");
                    b.output(output_dir.join(format!("result.{to}")));
                }
                _ => {}
            }
        }
        "video" => {
            let input = one()?;
            ensure!(
                [
                    "convert",
                    "compress",
                    "trim",
                    "resize",
                    "mute",
                    "snapshot",
                    "gif",
                    "audio-extract",
                    "audio-convert"
                ]
                .contains(&action),
                "不支持的视频操作"
            );
            if let Some(sub) = action.strip_prefix("audio-") {
                b.arg("audio");
                b.arg(sub);
            } else {
                b.arg(action);
            }
            b.positional.push(input.as_os_str().to_owned());
            b.arg("--json");
            b.option("time_limit", "30");
            let extension = match action {
                "audio-extract" | "audio-convert" => b
                    .choice("to", "mp3", &["mp3", "m4a", "wav", "flac"])?
                    .to_string(),
                "snapshot" => {
                    b.optional("at");
                    "png".into()
                }
                "gif" => {
                    b.optional("start");
                    b.number("duration", Some("5"), 0.001, 60., false)?;
                    b.number("width", Some("480"), 2., 4096., true)?;
                    b.number("fps", Some("12"), 1., 50., true)?;
                    "gif".into()
                }
                _ => {
                    if action != "mute" {
                        b.encoding(if action == "compress" {
                            "small"
                        } else {
                            "balanced"
                        })?;
                    }
                    if action == "trim" {
                        b.optional("start");
                        b.optional("duration");
                        if !request.params.contains_key("duration") {
                            b.option("duration", "5");
                        }
                    }
                    if action == "resize" {
                        let size = b.value("size", "1280x720");
                        b.option("size", size);
                    }
                    if action == "convert" {
                        b.choice("to", "mp4", &["mp4", "mkv", "webm", "mov"])?
                            .into()
                    } else {
                        "mp4".into()
                    }
                }
            };
            b.output(output_dir.join(format!("result.{extension}")));
        }
        "sound" => {
            b.arg(action);
            b.arg("--json");
            if action == "presets" {
                none()?;
            } else {
                let mut midi = false;
                match action {
                    "sfx" => {
                        none()?;
                        let preset = b.value("preset", "success");
                        ensure!(
                            [
                                "notify",
                                "success",
                                "error",
                                "coin",
                                "jump",
                                "laser",
                                "explosion",
                                "whoosh"
                            ]
                            .contains(&preset),
                            "不支持的音效"
                        );
                        b.positional.push(preset.into());
                        b.number("duration", None, 0.05, 10., false)?;
                        b.number("pitch", Some("0"), -24., 24., false)?;
                    }
                    "tone" => {
                        none()?;
                        let pitch = b.value("pitch", "A4");
                        b.positional.push(pitch.into());
                        b.number("duration", Some("1"), 0.01, 120., false)?;
                        b.choice(
                            "instrument",
                            "sine",
                            &["sine", "soft", "bell", "pluck", "chip"],
                        )?;
                    }
                    "melody" => {
                        b.source(inputs)?;
                        let format = b.value("format", "wav");
                        ensure!(
                            ["wav", "mid", "midi"].contains(&format),
                            "旋律格式必须是 wav 或 mid/midi"
                        );
                        midi = format != "wav";
                        b.number("bpm", Some("120"), 20., 400., true)?;
                        b.number("repeat", Some("1"), 1., 64., true)?;
                        if !midi && b.params.contains_key("instrument") {
                            b.choice(
                                "instrument",
                                "soft",
                                &["sine", "soft", "bell", "pluck", "chip"],
                            )?;
                        }
                    }
                    "beat" => {
                        none()?;
                        let preset = b.value("preset", "basic");
                        ensure!(
                            ["basic", "house", "hiphop"].contains(&preset),
                            "不支持的鼓型"
                        );
                        b.positional.push(preset.into());
                        b.number("bpm", Some("120"), 20., 400., true)?;
                        b.number("bars", Some("4"), 1., 64., true)?;
                    }
                    _ => bail!("不支持的声音操作"),
                }
                if !midi {
                    if let Some(rate) = b.take("sample_rate") {
                        ensure!(
                            ["22050", "44100", "48000"].contains(&rate),
                            "不支持的采样率"
                        );
                        b.option("sample_rate", rate);
                    }
                    b.number("volume", None, 0., 1., false)?;
                    if let Some(seed) = b.take("seed") {
                        seed.parse::<u64>().context("seed 必须是 u64")?;
                        b.option("seed", seed);
                    }
                }
                b.output(output_dir.join(if midi { "melody.mid" } else { "sound.wav" }));
            }
        }
        _ => bail!("不支持的工具领域"),
    }
    ensure!(
        b.params.is_empty(),
        "存在未支持的参数：{}",
        b.params.keys().copied().collect::<Vec<_>>().join(", ")
    );
    // 必须通过所有白名单校验后才写输入。create_new 防止替换其他任务或已有文件。
    if let Some(text) = b.text {
        use std::io::Write;
        let mut file = tempfile::Builder::new()
            .prefix("input-text-")
            .tempfile_in(output_dir.parent().context("输出目录缺少父目录")?)
            .context("无法创建隔离文本输入")?;
        file.write_all(&text)?;
        let (_, path) = file.keep().context("无法保留隔离文本输入")?;
        b.args.push("--file".into());
        b.args.push(path.into_os_string());
    }
    // 所有自由位置参数放在 -- 后；--key=value 避免选项值被 clap 重新识别成选项。
    if !b.positional.is_empty() {
        b.args.push("--".into());
        b.args.extend(b.positional);
    }
    Ok(PreparedCommand {
        args: b.args,
        stdin: b.stdin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn req(tool: &str, action: &str, params: &[(&str, &str)]) -> RunRequest {
        RunRequest {
            tool: tool.into(),
            action: action.into(),
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            files: vec![],
        }
    }
    #[test]
    fn rejects_unknown_and_excessive_requests() {
        let dir = tempfile::tempdir().unwrap();
        for request in [
            req("shell", "run", &[]),
            req("id", "uuid", &[("output", "/tmp/no")]),
            req("id", "uuid", &[("count", "101")]),
            req("sound", "tone", &[("duration", "NaN")]),
        ] {
            assert!(prepare(&request, &[], dir.path()).is_err());
        }
    }
    /// 使用真实领域 clap 定义校验映射，避免 Web 表单与 CLI 选项逐渐漂移。
    #[test]
    fn generated_arguments_match_real_domain_parsers() {
        use clap::Parser;
        #[derive(clap::Parser)]
        struct CheckCli {
            #[command(subcommand)]
            tool: CheckTool,
        }
        #[derive(clap::Subcommand)]
        enum CheckTool {
            Codec(crate::codec::cli::CodecArgs),
            Id(crate::id::cli::IdArgs),
            Time(crate::time::cli::TimeArgs),
            Key(crate::key::cli::KeyArgs),
            Video(crate::video::cli::VideoArgs),
            Sound(crate::sound::cli::SoundArgs),
        }
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out");
        std::fs::create_dir(&output).unwrap();
        let upload = dir.path().join("upload");
        std::fs::write(&upload, b"input").unwrap();
        for (tool, action, params, needs_upload) in [
            ("codec", "base64-encode", vec![("text", "--help")], false),
            ("codec", "base64-decode", vec![("text", "AP8=")], false),
            ("codec", "html-encode", vec![("text", "<tag>")], false),
            ("codec", "html-decode", vec![("text", "&amp;")], false),
            (
                "codec",
                "url-parse",
                vec![("text", "https://example.com")],
                false,
            ),
            ("id", "hash", vec![("text", "abc")], false),
            ("id", "uuid", vec![], false),
            ("time", "now", vec![], false),
            ("time", "from-timestamp", vec![("value", "-1")], false),
            (
                "time",
                "to-timestamp",
                vec![("datetime", "2024-01-01T00:00:00Z")],
                false,
            ),
            (
                "time",
                "convert",
                vec![("datetime", "2024-01-01T00:00:00Z")],
                false,
            ),
            ("time", "zones", vec![("filter", "--help")], false),
            ("key", "generate", vec![], false),
            ("key", "public", vec![], true),
            ("key", "check", vec![], true),
            ("video", "convert", vec![], true),
            ("video", "compress", vec![], true),
            ("video", "trim", vec![], true),
            ("video", "resize", vec![], true),
            ("video", "mute", vec![], true),
            ("video", "snapshot", vec![], true),
            ("video", "gif", vec![], true),
            ("video", "audio-extract", vec![], true),
            ("video", "audio-convert", vec![], true),
            ("sound", "presets", vec![], false),
            ("sound", "sfx", vec![], false),
            ("sound", "tone", vec![], false),
            ("sound", "melody", vec![("text", "C4 D4 E4")], false),
            ("sound", "beat", vec![], false),
        ] {
            let inputs = if needs_upload {
                vec![upload.clone()]
            } else {
                vec![]
            };
            let command = prepare(&req(tool, action, &params), &inputs, &output).unwrap();
            let parsed = CheckCli::try_parse_from(
                std::iter::once(OsString::from("kito")).chain(command.args),
            );
            assert!(parsed.is_ok(), "{tool}/{action}: {:?}", parsed.err());
        }
    }
    #[test]
    fn rejects_large_unknown_conflicting_and_midi_options() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("upload");
        std::fs::write(&input, b"abc").unwrap();
        assert!(
            prepare(
                &req("codec", "base64-encode", &[("text", "abc")]),
                std::slice::from_ref(&input),
                dir.path()
            )
            .is_err()
        );
        assert!(
            prepare(
                &req("key", "generate", &[("passphrase", "secret\n")]),
                &[],
                dir.path()
            )
            .is_err()
        );
        assert!(
            prepare(
                &req(
                    "sound",
                    "melody",
                    &[("text", "C4"), ("format", "mid"), ("volume", "0.5")]
                ),
                &[],
                dir.path()
            )
            .is_err()
        );
        assert!(
            prepare(
                &req("text", "count", &[("text", &"a".repeat(65536))]),
                &[],
                dir.path()
            )
            .is_err()
        );
        assert!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .all(|entry| entry.unwrap().path() == input)
        );
        assert!(
            serde_json::from_str::<RunRequest>(r#"{"tool":"id","action":"uuid","argv":[]}"#)
                .is_err()
        );
    }
    #[test]
    fn text_is_byte_exact_and_never_an_option() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out");
        std::fs::create_dir(&output).unwrap();
        let text = "--help\0\u{feff}你好\n";
        let cmd = prepare(
            &req("codec", "base64-encode", &[("text", text)]),
            &[],
            &output,
        )
        .unwrap();
        assert!(!cmd.args.iter().any(|arg| arg == text));
        let index = cmd.args.iter().position(|arg| arg == "--file").unwrap();
        assert_eq!(
            std::fs::read(PathBuf::from(&cmd.args[index + 1])).unwrap(),
            text.as_bytes()
        );
        assert!(cmd.args.iter().any(|arg| arg == "--output"));
    }
    #[test]
    fn passphrase_only_enters_stdin_and_private_output_is_fixed() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = prepare(
            &req("key", "generate", &[("passphrase", "s3cret--help")]),
            &[],
            dir.path(),
        )
        .unwrap();
        assert_eq!(cmd.stdin.as_deref(), Some(b"s3cret--help".as_slice()));
        assert!(
            !cmd.args
                .iter()
                .any(|arg| arg.to_string_lossy().contains("s3cret"))
        );
        assert!(
            cmd.args
                .iter()
                .any(|arg| arg.as_os_str() == dir.path().join("keys").as_os_str())
        );
    }
    #[test]
    fn binary_decode_has_fixed_file_output_and_upload_counts_are_checked() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out");
        std::fs::create_dir(&output).unwrap();
        let cmd = prepare(
            &req("codec", "base64-decode", &[("text", "AP8=")]),
            &[],
            &output,
        )
        .unwrap();
        assert!(
            cmd.args
                .iter()
                .any(|arg| arg.as_os_str() == output.join("decoded.bin").as_os_str())
        );
        assert!(prepare(&req("image", "info", &[]), &[], dir.path()).is_err());
        assert!(prepare(&req("id", "uuid", &[]), &[dir.path().join("a")], dir.path()).is_err());
    }
}
