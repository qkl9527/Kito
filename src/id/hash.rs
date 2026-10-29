//! 使用标准 Digest 实现按块读取；不把整个文件载入内存，也不改变输入字节。

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use clap::ValueEnum;
use serde::Serialize;
use sha2::Digest;
use std::io::{ErrorKind, Read};

#[derive(Clone, Copy, Debug, Default, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha224,
    #[default]
    Sha256,
    Sha384,
    Sha512,
}

#[derive(Clone, Copy, Debug, Default, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Hex,
    Base64,
}

#[derive(Serialize)]
pub struct Report {
    pub algorithm: Algorithm,
    pub format: Format,
    pub digest: String,
    pub input_bytes: String,
    pub source_kind: &'static str,
}

pub fn calculate(
    reader: &mut dyn Read,
    source_kind: &'static str,
    algorithm: Algorithm,
    format: Format,
) -> Result<Report> {
    let (digest, input_bytes) = match algorithm {
        Algorithm::Md5 => read_digest::<md5::Md5>(reader),
        Algorithm::Sha1 => read_digest::<sha1::Sha1>(reader),
        Algorithm::Sha224 => read_digest::<sha2::Sha224>(reader),
        Algorithm::Sha256 => read_digest::<sha2::Sha256>(reader),
        Algorithm::Sha384 => read_digest::<sha2::Sha384>(reader),
        Algorithm::Sha512 => read_digest::<sha2::Sha512>(reader),
    }?;
    let digest = match format {
        Format::Hex => {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let mut result = String::with_capacity(digest.len() * 2);
            for byte in digest {
                result.push(HEX[(byte >> 4) as usize] as char);
                result.push(HEX[(byte & 0x0f) as usize] as char);
            }
            result
        }
        Format::Base64 => STANDARD.encode(digest),
    };
    Ok(Report {
        algorithm,
        format,
        digest,
        input_bytes: input_bytes.to_string(),
        source_kind,
    })
}

fn read_digest<D: Digest>(reader: &mut dyn Read) -> Result<(Vec<u8>, u128)> {
    let mut digest = D::new();
    let mut total = 0_u128;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            // Read 契约允许被信号中断；重试时不重复计入任何已读取字节。
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("读取 Hash 输入失败"),
        };
        total = total
            .checked_add(count as u128)
            .context("输入字节数超出计数范围")?;
        digest.update(&buffer[..count]);
    }
    Ok((digest.finalize().to_vec(), total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_failure_is_propagated_instead_of_hashing_partial_content() {
        struct FailingReader(bool);
        impl Read for FailingReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.0 {
                    Err(std::io::Error::other("测试读取失败"))
                } else {
                    self.0 = true;
                    buffer[0] = b'a';
                    Ok(1)
                }
            }
        }
        let error = calculate(
            &mut FailingReader(false),
            "file",
            Algorithm::Sha256,
            Format::Hex,
        )
        .err()
        .unwrap();
        assert!(format!("{error:#}").contains("测试读取失败"));
    }

    #[test]
    fn interrupted_reads_retry_and_large_inputs_cross_buffer_boundaries() {
        struct InterruptedReader {
            interrupted: bool,
            inner: std::io::Cursor<Vec<u8>>,
        }
        impl Read for InterruptedReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(std::io::Error::from(ErrorKind::Interrupted));
                }
                self.inner.read(buffer)
            }
        }
        let mut reader = InterruptedReader {
            interrupted: false,
            inner: std::io::Cursor::new(vec![b'a'; 1_000_000]),
        };
        let report = calculate(&mut reader, "file", Algorithm::Sha256, Format::Hex).unwrap();
        assert_eq!(report.input_bytes, "1000000");
        assert_eq!(
            report.digest,
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
