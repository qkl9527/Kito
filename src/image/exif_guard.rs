//! TIFF 解析前的资源预检。不解码像素，也不替代 EXIF 库的标签语义解析。
//!
//! 多个字段可以指向同一段数据，仅限制文件大小不足以约束解析工作集。
//! 因此在库展开目录、复制字段前，累计每次引用的大小，并拒绝重复目录。
use anyhow::{Context, Result, ensure};
use std::collections::HashSet;

pub(super) fn validate(data: &[u8], memory_bytes: usize) -> Result<()> {
    ensure!(data.len() >= 8, "EXIF TIFF 头被截断");
    let little = match &data[..4] {
        b"II\x2a\0" => true,
        b"MM\0\x2a" => false,
        _ => anyhow::bail!("EXIF 不是支持的 TIFF 结构（不支持 BigTIFF）"),
    };
    let mut guard = Guard {
        data,
        little,
        visited: HashSet::new(),
        fields_left: 4000.min(memory_bytes / 1024),
        // 为 ASCII 分段、JSON 节点、格式化字符串预留空间。这是保守的
        // 解析预算，不是操作系统 RSS 硬上限，也不代表图片像素解码预算。
        bytes_left: memory_bytes / 256,
    };
    let mut offset = guard.u32(4)? as usize;
    let mut pages = 0;
    while offset != 0 {
        pages += 1;
        ensure!(pages <= 8, "EXIF 顶层 IFD 数量超过上限 8");
        offset = guard.directory(offset, Directory::Tiff)?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
enum Directory {
    Tiff,
    Exif,
    Gps,
    Interop,
}

struct Guard<'a> {
    data: &'a [u8],
    little: bool,
    visited: HashSet<usize>,
    fields_left: usize,
    bytes_left: usize,
}

impl Guard<'_> {
    fn bytes<const N: usize>(&self, offset: usize) -> Result<[u8; N]> {
        let end = offset.checked_add(N).context("EXIF 偏移溢出")?;
        Ok(self
            .data
            .get(offset..end)
            .context("EXIF 目录或字段被截断")?
            .try_into()?)
    }

    fn u16(&self, offset: usize) -> Result<u16> {
        let bytes = self.bytes(offset)?;
        Ok(if self.little {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    }

    fn u32(&self, offset: usize) -> Result<u32> {
        let bytes = self.bytes(offset)?;
        Ok(if self.little {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    }

    fn directory(&mut self, offset: usize, context: Directory) -> Result<usize> {
        ensure!(offset >= 8, "EXIF IFD 偏移进入 TIFF 头");
        ensure!(self.visited.insert(offset), "EXIF 包含重复或循环 IFD 指针");
        let count = usize::from(self.u16(offset)?);
        self.fields_left = self
            .fields_left
            .checked_sub(count)
            .context("EXIF 字段数量超过预算或上限 4000")?;
        let entries = offset.checked_add(2).context("EXIF 偏移溢出")?;
        let end = entries
            .checked_add(count * 12)
            .context("EXIF 目录长度溢出")?;
        let next = self.u32(end)? as usize;

        for index in 0..count {
            let entry = entries + index * 12;
            let tag = self.u16(entry)?;
            let typ = self.u16(entry + 2)?;
            let components = self.u32(entry + 4)? as usize;
            let unit: usize = match typ {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                // 未知类型由库作为 type/count/offset 描述，不展开数据。
                _ => 0,
            };
            let length = unit.checked_mul(components).context("EXIF 字段长度溢出")?;
            self.bytes_left = self
                .bytes_left
                .checked_sub(length)
                .context("EXIF 字段累计数据量超过内存预算（预算的 1/256）")?;
            if length > 4 {
                let start = self.u32(entry + 8)? as usize;
                ensure!(
                    start <= self.data.len() && length <= self.data.len() - start,
                    "EXIF 字段数据被截断"
                );
            }

            // 只跟随标准 EXIF/GPS/Interop 指针。MakerNote 和 TIFF SubIFDs
            // 等其他字段交给库作为普通值展示，不猜测厂商私有目录结构。
            let child = match (context, tag) {
                (Directory::Tiff, 0x8769) => Some(Directory::Exif),
                (Directory::Tiff, 0x8825) => Some(Directory::Gps),
                (Directory::Exif, 0xa005) => Some(Directory::Interop),
                _ => None,
            };
            if let Some(child) = child {
                ensure!(typ == 4 && components == 1, "EXIF IFD 指针必须是单个 LONG");
                let child_offset = self.u32(entry + 8)? as usize;
                ensure!(
                    self.directory(child_offset, child)? == 0,
                    "EXIF 子目录含非法 next IFD 指针"
                );
            }
        }
        Ok(next)
    }
}
