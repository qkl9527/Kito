//! 密钥专用进程边界：口令只走 stdin；原始 stderr 不进入错误链。
use anyhow::{Context, Result, bail};
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MAX_OUTPUT: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(120);

pub struct OpenSsl {
    path: OsString,
    pub version: String,
}
impl OpenSsl {
    pub fn discover() -> Result<Self> {
        let path = std::env::var_os("KITO_OPENSSL").unwrap_or_else(|| "openssl".into());
        let output = execute(&path, &["version".into()], None, TIMEOUT)?;
        let version = String::from_utf8(output).context("OpenSSL 版本输出不是 UTF-8")?;
        let version = version.trim().to_owned();
        if !version.starts_with("OpenSSL 3.") {
            bail!("密钥伙伴要求 OpenSSL 3.x；当前后端不是受支持的 OpenSSL 3.x");
        }
        Ok(Self { path, version })
    }
    pub fn run(&self, args: &[OsString], passphrase: Option<&[u8]>) -> Result<Vec<u8>> {
        execute(&self.path, args, passphrase, TIMEOUT)
    }
    pub fn capabilities(&self) -> Result<()> {
        let algorithms = self.run(&["list".into(), "-public-key-algorithms".into()], None)?;
        let algorithms = String::from_utf8_lossy(&algorithms).to_ascii_uppercase();
        let ciphers = self.run(&["list".into(), "-cipher-algorithms".into()], None)?;
        if !algorithms.contains("RSA")
            || !algorithms.contains("ED25519")
            || !String::from_utf8_lossy(&ciphers)
                .to_ascii_uppercase()
                .contains("AES-256-CBC")
        {
            bail!("OpenSSL 缺少 RSA、Ed25519 或 AES-256-CBC 所需能力");
        }
        Ok(())
    }
}

fn reader(mut stream: impl Read + Send + 'static) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        // 两条管道并发读取，避免背压死锁；超限关闭管道，进程仍受总超时约束。
        let mut output = Vec::new();
        let mut buffer = [0u8; 8192];
        let result = (|| {
            loop {
                let count = stream.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                if output.len() + count <= MAX_OUTPUT {
                    output.extend_from_slice(&buffer[..count]);
                } else {
                    return Err(std::io::Error::other("OpenSSL 输出超过限制"));
                }
            }
            Ok(output)
        })();
        let _ = send.send(result);
    });
    receive
}
fn execute(
    path: &OsStr,
    args: &[OsString],
    passphrase: Option<&[u8]>,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let mut child = Command::new(path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("无法启动 OpenSSL；请安装 OpenSSL 3.x 或检查 KITO_OPENSSL")?;
    let started = Instant::now();
    let stdout = reader(child.stdout.take().context("OpenSSL stdout 管道不可用")?);
    let stderr = reader(child.stderr.take().context("OpenSSL stderr 管道不可用")?);
    // 最大口令为 1023 字节，小于匿名管道容量；不让后端等待终端交互。
    let input_result = if let Some(passphrase) = passphrase {
        let mut stdin = child.stdin.take().context("OpenSSL stdin 管道不可用")?;
        stdin
            .write_all(passphrase)
            .and_then(|()| stdin.write_all(b"\n"))
    } else {
        drop(child.stdin.take());
        Ok(())
    };
    loop {
        if let Some(status) = child.try_wait().context("无法获取 OpenSSL 进程状态")? {
            if !status.success() {
                bail!(
                    "OpenSSL 操作失败：请检查密钥格式、口令及后端能力（后端诊断已隐藏以保护秘密）"
                );
            }
            input_result.context("无法通过 stdin 向 OpenSSL 传递口令")?;
            let remaining = timeout.saturating_sub(started.elapsed());
            let output = stdout
                .recv_timeout(remaining)
                .context("OpenSSL 输出读取超时")??;
            let remaining = timeout.saturating_sub(started.elapsed());
            let _ = stderr
                .recv_timeout(remaining)
                .context("OpenSSL 诊断读取超时")??;
            return Ok(output);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("OpenSSL 操作超时，子进程已终止");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn stalled_child_is_killed_without_echoing_secrets() {
        let error = execute(
            OsStr::new("/bin/sh"),
            &["-c".into(), "while :; do :; done".into()],
            Some(b"test-secret"),
            Duration::from_millis(30),
        )
        .unwrap_err();
        assert!(error.to_string().contains("超时"));
        assert!(!error.to_string().contains("test-secret"));
    }
}
