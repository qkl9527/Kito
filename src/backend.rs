//! 外部后端只接收参数数组；输出写临时文件，避免管道填满导致死锁。
use anyhow::{Result, bail, ensure};
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fmt, fs,
    io::{self, Read},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub struct ProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: String,
}

/// 仅包装 Command::spawn 的错误，避免把后端启动后的文件读取错误误判为程序缺失。
#[derive(Debug)]
struct ProcessSpawnError {
    program: OsString,
    source: io::Error,
}
impl fmt::Display for ProcessSpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "无法启动 {:?}", self.program)
    }
}
impl Error for ProcessSpawnError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Debug)]
struct OptionalBackendMissing(&'static str);
impl fmt::Display for OptionalBackendMissing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "未在 PATH 找到可选后端 {}，已跳过", self.0)
    }
}
impl Error for OptionalBackendMissing {}

pub fn is_optional_backend_missing(error: &anyhow::Error) -> bool {
    error.is::<OptionalBackendMissing>()
}

pub fn program(env: &str, default: &str) -> OsString {
    std::env::var_os(env).unwrap_or_else(|| default.into())
}

/// 新品牌变量优先，旧 CTK_* 仅作为未配置新变量时的后备。
/// 返回实际采用的变量名供诊断使用；Some("") 也是显式配置，不能继续回退。
fn configured_program<'a>(
    environment: &'a str,
    legacy_environment: &'a str,
    default: &str,
) -> (OsString, Option<&'a str>) {
    for name in [environment, legacy_environment] {
        if let Some(program) = std::env::var_os(name) {
            return (program, Some(name));
        }
    }
    (default.into(), None)
}

pub fn magick() -> OsString {
    configured_program("KITO_MAGICK", "CTK_MAGICK", "magick").0
}
pub fn zopflipng() -> OsString {
    configured_program("KITO_ZOPFLIPNG", "CTK_ZOPFLIPNG", "zopflipng").0
}

fn run_configured_process(
    environment: &str,
    legacy_environment: &str,
    default: &'static str,
    optional: bool,
    args: &[OsString],
    timeout: Option<Duration>,
) -> Result<ProcessOutput> {
    // 与公开 path helpers 共用解析规则；保留本次选中的来源，错误发生后不重新读环境。
    let (program, configured) = configured_program(environment, legacy_environment, default);
    run_process(&program, args, timeout).map_err(|error| {
        let Some(spawn) = error.downcast_ref::<ProcessSpawnError>() else {
            return error;
        };
        if optional && configured.is_none() && spawn.source.kind() == io::ErrorKind::NotFound {
            error.context(OptionalBackendMissing(default))
        } else if let Some(configured_environment) = configured {
            error.context(format!(
                "请检查 {configured_environment} 指定的程序路径和执行权限"
            ))
        } else {
            error.context(format!(
                "请确认 {default} 已安装且可从 PATH 执行，或设置 {environment}"
            ))
        }
    })
}

pub fn run_magick(args: &[OsString], timeout: Option<Duration>) -> Result<ProcessOutput> {
    run_configured_process("KITO_MAGICK", "CTK_MAGICK", "magick", false, args, timeout)
}

pub fn run_zopflipng(args: &[OsString], timeout: Option<Duration>) -> Result<ProcessOutput> {
    run_configured_process(
        "KITO_ZOPFLIPNG",
        "CTK_ZOPFLIPNG",
        "zopflipng",
        true,
        args,
        timeout,
    )
}

pub fn run_process(
    program: &OsStr,
    args: &[OsString],
    timeout: Option<Duration>,
) -> Result<ProcessOutput> {
    let out = tempfile::NamedTempFile::new()?;
    let err = tempfile::NamedTempFile::new()?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(out.reopen()?)
        .stderr(err.reopen()?)
        .spawn()
        .map_err(|source| ProcessSpawnError {
            program: program.to_owned(),
            source,
        })?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if timeout.is_some_and(|t| start.elapsed() >= t) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("后端执行超时");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stderr = String::new();
    err.reopen()?.take(256 * 1024).read_to_string(&mut stderr)?;
    ensure!(status.success(), "后端退出 {}：{}", status, stderr.trim());
    ensure!(
        out.as_file().metadata()?.len() <= 16 * 1024 * 1024,
        "后端文本输出异常过大"
    );
    Ok(ProcessOutput {
        stdout: fs::read(out.path())?,
        stderr,
    })
}
pub fn version(program: &OsStr, args: &[&str]) -> Result<String> {
    let out = run_process(
        program,
        &args.iter().map(OsString::from).collect::<Vec<_>>(),
        Some(Duration::from_secs(5)),
    )?;
    Ok(version_line(&out))
}

pub fn magick_version() -> Result<String> {
    let out = run_magick(&["-version".into()], Some(Duration::from_secs(5)))?;
    Ok(version_line(&out))
}

pub fn zopflipng_version() -> Result<String> {
    let out = run_zopflipng(&["-h".into()], Some(Duration::from_secs(5)))?;
    Ok(version_line(&out))
}

fn version_line(out: &ProcessOutput) -> String {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or(&out.stderr)
        .to_string()
}
pub fn safe_input(bytes: &[u8]) -> Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("source");
    fs::write(&path, bytes)?;
    Ok((dir, path))
}

/// 保留经过校验的扩展名，兼容 ICO/TGA 等需要扩展名辅助识别的格式。
pub fn safe_input_as(bytes: &[u8], hint: Option<&str>) -> Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let extension = hint
        .filter(|s| !s.is_empty() && s.len() <= 12 && s.bytes().all(|c| c.is_ascii_alphanumeric()));
    let name = extension
        .map(|s| format!("source.{}", s.to_ascii_lowercase()))
        .unwrap_or_else(|| "source".into());
    let path = dir.path().join(name);
    fs::write(&path, bytes)?;
    Ok((dir, path))
}
