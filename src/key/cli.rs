use super::{process::OpenSsl, storage};
use anyhow::{Context, Result, bail};
use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

// OpenSSL 的 stdin 口令读取最多保留 1023 字节；超过时必须先拒绝，避免静默截断。
const MAX_PASSPHRASE_BYTES: usize = 1023;

#[derive(Args, Debug)]
#[command(
    about = "Kavi 卡维 · 密钥伙伴：生成、导出和检查 RSA / Ed25519 密钥",
    after_help = r#"使用案例:
  生成加密 RSA 密钥对（需预先准备 passphrase.txt，目标目录必须不存在）：
    kito key generate --out-dir ./keys/app --passphrase-file ./passphrase.txt

  从加密私钥导出公钥（输出文件必须不存在）：
    kito key public ./keys/app/private.pem --passphrase-file ./passphrase.txt --output ./exported-public.pem

  检查私钥并确认公钥配对，输出 JSON 报告：
    kito key check ./keys/app/private.pem --passphrase-file ./passphrase.txt --public ./keys/app/public.pem --json

  检查 OpenSSL 3.x 后端与算法能力：
    kito key doctor

  口令只从文件或 stdin 读取，不直接写在命令行参数中。"#
)]
pub struct KeyArgs {
    #[command(subcommand)]
    command: KeyCommand,
}
#[derive(Subcommand, Debug)]
enum KeyCommand {
    /// 生成 RSA 或用于签名的 Ed25519 密钥对
    Generate(GenerateArgs),
    /// 从 PKCS#8 私钥导出 SPKI 公钥
    Public(PublicArgs),
    /// 检查私钥一致性及可选公钥配对
    Check(CheckArgs),
    /// 检查可选 OpenSSL 3 后端及算法能力
    #[command(after_help = r#"使用案例:
  检查 OpenSSL 3.x 后端及 RSA、Ed25519、私钥加密能力：
    kito key doctor

  输出供脚本读取的 JSON 检查报告：
    kito key doctor --json"#)]
    Doctor {
        /// 以 JSON 输出后端与算法能力报告
        #[arg(long)]
        json: bool,
    },
}
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq)]
enum Algorithm {
    /// RSA，默认 3072 位，可用 --bits 指定长度
    Rsa,
    /// Ed25519，用于签名，参数固定且不接受 --bits
    Ed25519,
}
#[derive(Args, Debug)]
#[command(group(ArgGroup::new("protection").required(true).args(["passphrase_file", "passphrase_stdin", "no_passphrase"])))]
#[command(after_help = r#"使用案例:
  生成默认 3072 位的加密 RSA 密钥对（需预先准备 passphrase.txt）：
    kito key generate --out-dir ./keys/app --passphrase-file ./passphrase.txt

  生成 4096 位加密 RSA 密钥对并输出 JSON 报告：
    kito key generate --algorithm rsa --bits 4096 --out-dir ./keys/rsa-4096 --passphrase-file ./passphrase.txt --json

  生成用于签名的加密 Ed25519 密钥对：
    kito key generate --algorithm ed25519 --out-dir ./keys/signing --passphrase-file ./passphrase.txt

  明确生成不设口令的本地测试密钥，私钥以未加密形式保存：
    kito key generate --algorithm ed25519 --out-dir ./keys/local-test --no-passphrase

  每个目标目录必须不存在；成功后包含 private.pem 和 public.pem，不覆盖已有文件。
  必须且只能选择一种保护方式；口令只从文件或 stdin 读取，不直接写在命令行参数中。"#)]
struct GenerateArgs {
    /// 密钥算法：RSA 或用于签名的 Ed25519
    #[arg(long, value_enum, default_value = "rsa")]
    algorithm: Algorithm,
    /// RSA 位数：2048、3072 或 4096，默认 3072；Ed25519 不接受此参数
    #[arg(long, value_parser = parse_bits)]
    bits: Option<u32>,
    /// 保存 private.pem 与 public.pem 的新目录，必须不存在；缺少的父目录自动创建
    #[arg(long)]
    out_dir: PathBuf,
    #[command(flatten)]
    passphrase: PassphraseArgs,
    /// 明确允许生成未加密私钥，与两种口令来源互斥
    #[arg(long)]
    no_passphrase: bool,
    /// 以 JSON 输出生成报告，不包含私钥或口令
    #[arg(long)]
    json: bool,
}
fn parse_bits(value: &str) -> std::result::Result<u32, String> {
    match value {
        "2048" => Ok(2048),
        "3072" => Ok(3072),
        "4096" => Ok(4096),
        _ => Err("RSA --bits 仅支持 2048、3072 或 4096".into()),
    }
}
#[derive(Args, Debug)]
struct PassphraseArgs {
    /// 从预先准备的文件读取单行 UTF-8 口令（1–1023 字节），与 --passphrase-stdin 互斥
    #[arg(long, conflicts_with = "passphrase_stdin")]
    passphrase_file: Option<PathBuf>,
    /// 从 stdin 读取单行 UTF-8 口令（1–1023 字节），读至 EOF；不显示交互提示
    #[arg(long)]
    passphrase_stdin: bool,
}
impl PassphraseArgs {
    fn read(&self) -> Result<Option<Vec<u8>>> {
        let mut source: Box<dyn Read> = if let Some(path) = &self.passphrase_file {
            Box::new(fs::File::open(path).context("无法读取口令文件")?)
        } else if self.passphrase_stdin {
            Box::new(std::io::stdin())
        } else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        source
            .by_ref()
            // 两字节 CRLF 加一字节越界探测，确保过长输入不会被当作合法前缀。
            .take((MAX_PASSPHRASE_BYTES + 3) as u64)
            .read_to_end(&mut bytes)
            .context("读取口令失败")?;
        if bytes.ends_with(b"\r\n") {
            bytes.truncate(bytes.len() - 2);
        } else if bytes.ends_with(b"\n") {
            bytes.pop();
        }
        if bytes.is_empty()
            || bytes.len() > MAX_PASSPHRASE_BYTES
            || bytes.iter().any(|b| matches!(b, 0 | b'\n' | b'\r'))
            || std::str::from_utf8(&bytes).is_err()
        {
            bail!("口令必须为 1–1023 字节的 UTF-8 单行文本，不能包含 NUL 或内嵌换行");
        }
        Ok(Some(bytes))
    }
}
#[derive(Args, Debug)]
#[command(after_help = r#"使用案例:
  从加密 PKCS#8 私钥导出 SPKI 公钥（需预先准备 passphrase.txt）：
    kito key public ./keys/app/private.pem --passphrase-file ./passphrase.txt --output ./exported-public.pem

  导出公钥并输出 JSON 报告：
    kito key public ./keys/signing/private.pem --passphrase-file ./passphrase.txt -o ./signing-public.pem --json

  从未加密的本地测试私钥导出公钥，无需提供口令：
    kito key public ./keys/local-test/private.pem --output ./local-test-public.pem

  输出文件必须不存在；加密私钥的口令只从文件或 stdin 读取，不直接写在命令行参数中。"#)]
struct PublicArgs {
    /// 输入 PKCS#8 PEM 私钥路径；已加密时必须提供口令来源
    private: PathBuf,
    #[command(flatten)]
    passphrase: PassphraseArgs,
    /// 导出 SPKI PEM 公钥的文件路径，必须不存在，不覆盖已有文件或符号链接
    #[arg(short, long)]
    output: PathBuf,
    /// 以 JSON 输出导出报告，不包含私钥或口令
    #[arg(long)]
    json: bool,
}
#[derive(Args, Debug)]
#[command(after_help = r#"使用案例:
  检查加密 PKCS#8 私钥的内部一致性（需预先准备 passphrase.txt）：
    kito key check ./keys/app/private.pem --passphrase-file ./passphrase.txt

  同时检查公私钥配对，输出 JSON 报告：
    kito key check ./keys/app/private.pem --passphrase-file ./passphrase.txt --public ./keys/app/public.pem --json

  检查未加密的本地测试私钥，无需提供口令：
    kito key check ./keys/local-test/private.pem

  加密私钥的口令只从文件或 stdin 读取，不直接写在命令行参数中；公钥不匹配时退出码为 1。"#)]
struct CheckArgs {
    /// 待检查的 PKCS#8 PEM 私钥路径；已加密时必须提供口令来源
    private: PathBuf,
    #[command(flatten)]
    passphrase: PassphraseArgs,
    /// 可选的 SPKI PEM 公钥路径，用于确认是否与私钥配对
    #[arg(long)]
    public: Option<PathBuf>,
    /// 以 JSON 输出检查报告，包括 validity 与 match，不包含私钥或口令
    #[arg(long)]
    json: bool,
}
// serde_json 对不可表示为 UTF-8 的 Path 会返回错误，json!(PathBuf) 则会 panic。
// 仅限制需要进入报告的路径，并在任何文件写入之前显式校验。
fn report_path(path: &Path) -> Result<&str> {
    path.to_str()
        .context("密钥报告路径必须是有效 UTF-8；请使用 UTF-8 路径")
}
fn args(words: &[&str]) -> Vec<OsString> {
    words.iter().map(OsString::from).collect()
}
fn private_args(command: &[&str], private: &Path) -> Vec<OsString> {
    let mut result = args(command);
    result.extend([
        "-in".into(),
        private.as_os_str().to_owned(),
        "-passin".into(),
        "stdin".into(),
    ]);
    result
}
fn encrypted(private: &Path) -> Result<bool> {
    let mut file = fs::File::open(private).context("无法读取私钥")?;
    let mut header = [0u8; 64];
    let count = file.read(&mut header)?;
    let header = &header[..count];
    if header.starts_with(b"-----BEGIN ENCRYPTED PRIVATE KEY-----") {
        Ok(true)
    } else if header.starts_with(b"-----BEGIN PRIVATE KEY-----") {
        Ok(false)
    } else {
        bail!("仅支持 PKCS#8 PEM PRIVATE KEY 或 ENCRYPTED PRIVATE KEY")
    }
}
fn check_private(backend: &OpenSsl, private: &Path, pass: Option<&[u8]>) -> Result<Vec<u8>> {
    if encrypted(private)? && pass.is_none() {
        bail!("私钥已加密，请提供 --passphrase-file 或 --passphrase-stdin");
    }
    // 即使私钥未加密，OpenSSL 的 -passin stdin 也要求读到一行；空行禁止其交互询问。
    backend.run(
        &private_args(&["pkey", "-check", "-noout"], private),
        Some(pass.unwrap_or(b"")),
    )?;
    let der = backend.run(
        &private_args(&["pkey", "-pubout", "-outform", "DER"], private),
        Some(pass.unwrap_or(b"")),
    )?;
    // OpenSSL 能处理的算法多于 Kavi；必须在任何候选发布之前限制为 RSA / Ed25519。
    metadata(&der)?;
    Ok(der)
}
// SPKI 由 OpenSSL 规范化后再解析 AlgorithmIdentifier；不依赖本地化文本识别算法。
fn tlv<'a>(bytes: &mut &'a [u8], tag: u8) -> Result<&'a [u8]> {
    if bytes.len() < 2 || bytes[0] != tag {
        bail!("无效的 SPKI DER 结构");
    }
    let mut offset = 2;
    let mut size = usize::from(bytes[1]);
    if size & 128 != 0 {
        let count = size & 127;
        if count == 0 || count > 4 || bytes.len() < 2 + count {
            bail!("无效的 DER 长度");
        }
        size = 0;
        for byte in &bytes[2..2 + count] {
            size = size * 256 + usize::from(*byte);
        }
        offset += count;
    }
    if size > bytes.len() - offset {
        bail!("不完整的 DER 内容");
    }
    let value = &bytes[offset..offset + size];
    *bytes = &bytes[offset + size..];
    Ok(value)
}
fn metadata(der: &[u8]) -> Result<(&'static str, u32)> {
    let mut outer = der;
    let mut sequence = tlv(&mut outer, 0x30)?;
    let mut algorithm = tlv(&mut sequence, 0x30)?;
    let oid = tlv(&mut algorithm, 0x06)?;
    if oid == [0x2b, 0x65, 0x70] {
        return Ok(("ed25519", 256));
    }
    if oid != [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01] {
        bail!("仅支持 RSA 和 Ed25519 密钥");
    }
    let bit_string = tlv(&mut sequence, 0x03)?;
    if bit_string.first() != Some(&0) {
        bail!("无效的 RSA SPKI 位串");
    }
    let mut rsa = &bit_string[1..];
    let mut rsa = tlv(&mut rsa, 0x30)?;
    let modulus = tlv(&mut rsa, 0x02)?;
    let first = modulus
        .iter()
        .position(|v| *v != 0)
        .context("RSA 模数为空")?;
    let bits = (modulus.len() - first) * 8 - modulus[first].leading_zeros() as usize;
    Ok(("rsa", u32::try_from(bits)?))
}
fn report(backend: &OpenSsl, der: &[u8], protected: bool) -> Result<Value> {
    let (algorithm, bits) = metadata(der)?;
    Ok(
        json!({"algorithm":algorithm,"bits":bits,"private_format":"PKCS#8 PEM","public_format":"SPKI PEM","encrypted":protected,"openssl_version":backend.version,"fingerprint":format!("SHA256:{:x}", Sha256::digest(der))}),
    )
}
fn emit(value: &Value, as_json: bool) -> Result<()> {
    let bytes = if as_json {
        format!("{}\n", serde_json::to_string_pretty(value)?)
    } else {
        format!(
            "Kavi 卡维 · 密钥伙伴\n{}\n",
            serde_json::to_string_pretty(value)?
        )
    };
    match std::io::stdout().lock().write_all(bytes.as_bytes()) {
        Err(error) if error.kind() != std::io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}
pub fn run(input: KeyArgs) -> Result<bool> {
    match input.command {
        KeyCommand::Doctor { json: as_json } => {
            let backend = OpenSsl::discover()?;
            backend.capabilities()?;
            emit(
                &json!({"available":true,"openssl_version":backend.version,"algorithms":["rsa","ed25519"],"encryption":{"scheme":"PBES2","kdf":"PBKDF2-HMAC-SHA256","iterations":600000,"cipher":"AES-256-CBC"}}),
                as_json,
            )?;
        }
        KeyCommand::Generate(input) => generate(input)?,
        KeyCommand::Public(input) => export(input)?,
        KeyCommand::Check(input) => {
            let private_path = report_path(&input.private)?;
            let pass = input.passphrase.read()?;
            let backend = OpenSsl::discover()?;
            let der = check_private(&backend, &input.private, pass.as_deref())?;
            let mut value = report(&backend, &der, encrypted(&input.private)?)?;
            let matches = if let Some(public) = &input.public {
                let bytes = fs::read(public).context("无法读取待匹配公钥")?;
                if !bytes.starts_with(b"-----BEGIN PUBLIC KEY-----") {
                    bail!("待匹配公钥必须是 SPKI PEM PUBLIC KEY");
                }
                let mut command = args(&["pkey", "-pubin", "-outform", "DER", "-in"]);
                command.push(public.as_os_str().to_owned());
                Some(der == backend.run(&command, None)?)
            } else {
                None
            };
            value["validity"] = json!(true);
            value["match"] = json!(matches);
            value["private_path"] = json!(private_path);
            emit(&value, input.json)?;
            return Ok(matches != Some(false));
        }
    }
    Ok(true)
}
fn generate(input: GenerateArgs) -> Result<()> {
    report_path(&input.out_dir)?;
    let private_output = input.out_dir.join("private.pem");
    let public_output = input.out_dir.join("public.pem");
    let private_path = report_path(&private_output)?;
    let public_path = report_path(&public_output)?;
    if input.algorithm == Algorithm::Ed25519 && input.bits.is_some() {
        bail!("Ed25519 参数固定，不接受 --bits");
    }
    storage::reject_existing(&input.out_dir)?;
    let pass = input.passphrase.read()?;
    let backend = OpenSsl::discover()?;
    backend.capabilities()?;
    let mut directory = storage::Directory::create(&input.out_dir)?;
    let result = (|| {
        let raw = directory.candidate(".raw-private.pem")?;
        let candidate = directory.candidate(".candidate-private.pem")?;
        let public = directory.candidate(".candidate-public.pem")?;
        let mut command = args(&[
            "genpkey",
            "-algorithm",
            if input.algorithm == Algorithm::Rsa {
                "RSA"
            } else {
                "ED25519"
            },
            "-out",
        ]);
        command.push(raw.as_os_str().to_owned());
        if input.algorithm == Algorithm::Rsa {
            command.extend(args(&[
                "-pkeyopt",
                &format!("rsa_keygen_bits:{}", input.bits.unwrap_or(3072)),
                "-pkeyopt",
                "rsa_keygen_pubexp:65537",
                "-pkeyopt",
                "rsa_keygen_primes:2",
            ]));
        }
        backend.run(&command, None)?;
        let original_der = check_private(&backend, &raw, None)?;
        if let Some(pass) = pass.as_deref() {
            let mut command = args(&[
                "pkcs8",
                "-topk8",
                "-v2",
                "aes-256-cbc",
                "-v2prf",
                "hmacWithSHA256",
                "-iter",
                "600000",
                "-passout",
                "stdin",
                "-in",
            ]);
            command.extend([
                raw.as_os_str().to_owned(),
                "-out".into(),
                candidate.as_os_str().to_owned(),
            ]);
            backend.run(&command, Some(pass))?;
        } else {
            fs::copy(&raw, &candidate)?;
        }
        let der = check_private(&backend, &candidate, pass.as_deref())?;
        if der != original_der {
            bail!("私钥编码后公钥发生变化，拒绝发布");
        }
        let mut command = private_args(&["pkey", "-pubout"], &candidate);
        command.extend(["-out".into(), public.as_os_str().to_owned()]);
        backend.run(&command, Some(pass.as_deref().unwrap_or(b"")))?;
        let mut command = args(&["pkey", "-pubin", "-outform", "DER", "-in"]);
        command.push(public.as_os_str().to_owned());
        if backend.run(&command, None)? != der {
            bail!("候选公钥与私钥不匹配，拒绝发布");
        }
        let mut value = report(&backend, &der, pass.is_some())?;
        directory.publish(&candidate, "private.pem")?;
        directory.publish(&public, "public.pem")?;
        value["private_path"] = json!(private_path);
        value["public_path"] = json!(public_path);
        value["encryption"] = if pass.is_some() {
            json!({"scheme":"PBES2","kdf":"PBKDF2-HMAC-SHA256","iterations":600000,"cipher":"AES-256-CBC"})
        } else {
            Value::Null
        };
        Ok(value)
    })();
    match result {
        Ok(value) => {
            directory.finish().with_context(|| {
                format!("密钥目录发布后清理失败，请检查 {}", input.out_dir.display())
            })?;
            emit(&value, input.json)
        }
        Err(error) => {
            drop(directory);
            if input.out_dir.exists() {
                Err(error).with_context(|| {
                    format!("清理未完成，请检查保留路径 {}", input.out_dir.display())
                })
            } else {
                Err(error)
            }
        }
    }
}
fn export(input: PublicArgs) -> Result<()> {
    let public_path = report_path(&input.output)?;
    storage::reject_existing(&input.output)?;
    let pass = input.passphrase.read()?;
    let backend = OpenSsl::discover()?;
    let der = check_private(&backend, &input.private, pass.as_deref())?;
    // 报告所需的算法校验与私钥读取也可能失败，全部置于最终输出发布之前。
    let mut value = report(&backend, &der, encrypted(&input.private)?)?;
    value["public_path"] = json!(public_path);
    let parent = input
        .output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // 临时目录与目标同盘，确保排他 hard_link 发布；先生成唯一名称，再由安全目录 API 创建。
    let reservation = tempfile::Builder::new()
        .prefix(".kito-key-")
        .tempfile_in(parent)?;
    let path = reservation.path().to_owned();
    reservation.close()?;
    let mut directory = storage::Directory::create(&path)?;
    let candidate = directory.candidate(".public.pem")?;
    let mut command = private_args(&["pkey", "-pubout"], &input.private);
    command.extend(["-out".into(), candidate.as_os_str().to_owned()]);
    backend.run(&command, Some(pass.as_deref().unwrap_or(b"")))?;
    let mut verify_command = args(&["pkey", "-pubin", "-outform", "DER", "-in"]);
    verify_command.push(candidate.as_os_str().to_owned());
    if backend.run(&verify_command, None)? != der {
        bail!("候选公钥与已检查私钥不匹配，拒绝发布");
    }
    storage::publish(&candidate, &input.output)?;
    emit(&value, input.json)
}
