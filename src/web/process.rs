//! Web 任务独立运行同一 Kito 程序，进程、日志和输出都有明确预算。
//! 用文件承接 stdout/stderr，避免子进程管道填满；口令只经 stdin。
use super::commands::PreparedCommand;
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const OUTPUT_LIMIT: u64 = 256 * 1024 * 1024;
pub const TEXT_LIMIT: u64 = 2 * 1024 * 1024;
pub const TIME_LIMIT: u64 = 180;

pub struct Output {
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

struct RunningChild {
    child: Child,
    #[cfg(windows)]
    job: windows::Job,
    finished: bool,
}

impl RunningChild {
    fn stop(&mut self) {
        #[cfg(unix)]
        // 子进程建立独立 process group；超时也终止它启动的 FFmpeg 等后端。
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        #[cfg(windows)]
        self.job.stop();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for RunningChild {
    fn drop(&mut self) {
        if !self.finished {
            self.stop();
        }
    }
}

/// 检查运行中的输出，拒绝链接及特殊文件；文件恰在原子发布时消失可忽略。
pub fn output_size(directory: &Path) -> Result<u64> {
    let mut size = 0u64;
    for entry in walkdir::WalkDir::new(directory).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error)
                if error
                    .io_error()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        ensure!(
            !entry.file_type().is_symlink(),
            "输出包含符号链接，已拒绝发布"
        );
        if entry.file_type().is_dir() {
            continue;
        }
        ensure!(
            entry.file_type().is_file(),
            "输出包含非普通文件，已拒绝发布"
        );
        match entry.metadata() {
            Ok(metadata) => size = size.saturating_add(metadata.len()),
            Err(error)
                if error
                    .io_error()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        }
        ensure!(size <= OUTPUT_LIMIT, "任务输出超过 256 MiB 限制");
    }
    Ok(size)
}

pub fn run(
    executable: &Path,
    plan: PreparedCommand,
    directory: &Path,
    stopping: &AtomicBool,
) -> Result<Output> {
    let stdout_path = directory.join("stdout.log");
    let stderr_path = directory.join("stderr.log");
    let stdout = File::create(&stdout_path)?;
    let stderr = File::create(&stderr_path)?;
    let mut command = Command::new(executable);
    command
        .args(plan.args)
        .current_dir(directory)
        .stdin(if plan.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(stdout)
        .stderr(stderr);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().context("无法启动 Kito 处理任务")?;
    #[cfg(windows)]
    let job = match windows::Job::attach(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.context("无法隔离任务进程"));
        }
    };
    // stdin 的写入不能阻塞预算检查。线程只拥有本次输入，写完立即关闭管道。
    let writer = if let Some(bytes) = plan.stdin {
        let mut input = child.stdin.take().context("无法打开任务标准输入")?;
        Some(std::thread::spawn(move || input.write_all(&bytes)))
    } else {
        None
    };
    let mut child = RunningChild {
        child,
        #[cfg(windows)]
        job,
        finished: false,
    };
    let start = Instant::now();
    let status = loop {
        if stopping.load(Ordering::Relaxed) {
            bail!("服务正在停止，任务已取消");
        }
        ensure!(
            start.elapsed() < Duration::from_secs(TIME_LIMIT),
            "任务超过 180 秒限制，已停止"
        );
        ensure!(
            fs::metadata(&stdout_path)?.len() <= TEXT_LIMIT,
            "任务文本输出超过 2 MiB 限制"
        );
        ensure!(
            fs::metadata(&stderr_path)?.len() <= 256 * 1024,
            "任务诊断输出过大"
        );
        output_size(&directory.join("output"))?;
        if let Some(status) = child.child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    child.finished = true;
    if let Some(writer) = writer {
        // CLI 参数错误可能提前关闭 stdin，以实际退出状态为准，不泄露输入内容。
        if status.success() {
            writer
                .join()
                .map_err(|_| anyhow::anyhow!("任务输入线程失败"))?
                .context("无法完整传递任务输入")?;
        }
    }
    ensure!(
        fs::metadata(&stdout_path)?.len() <= TEXT_LIMIT,
        "任务文本输出超过 2 MiB 限制"
    );
    output_size(&directory.join("output"))?;
    let mut stderr = Vec::new();
    File::open(stderr_path)?
        .take(256 * 1024)
        .read_to_end(&mut stderr)?;
    Ok(Output {
        success: status.success(),
        stdout: fs::read(stdout_path)?,
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

#[cfg(windows)]
mod windows {
    use anyhow::{Context, Result, ensure};
    use std::{mem::size_of, os::windows::io::AsRawHandle, process::Child};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
    };
    pub struct Job(HANDLE);
    impl Job {
        pub fn attach(child: &Child) -> Result<Self> {
            // Job 的句柄只由当前阻塞工作线程持有，关闭时终止整个进程树。
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            ensure!(!handle.is_null(), "无法创建 Windows Job Object");
            let job = Self(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = unsafe {
                SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if configured == 0 {
                return Err(std::io::Error::last_os_error()).context("无法限制 Windows 任务");
            }
            let assigned = unsafe { AssignProcessToJobObject(handle, child.as_raw_handle()) };
            if assigned == 0 {
                return Err(std::io::Error::last_os_error()).context("无法关联 Windows 任务进程");
            }
            Ok(job)
        }
        pub fn stop(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_budget_rejects_oversize_without_reading_file_contents() {
        let dir = tempfile::tempdir().unwrap();
        File::create(dir.path().join("large"))
            .unwrap()
            .set_len(OUTPUT_LIMIT + 1)
            .unwrap();
        assert!(output_size(dir.path()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn output_never_follows_a_symbolic_link() {
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/passwd", dir.path().join("leak")).unwrap();
        assert!(output_size(dir.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_stops_descendants_before_they_can_publish_a_marker() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("output")).unwrap();
        let stopping = AtomicBool::new(false);
        // 后代先通知测试“已启动”，再等待 release。这样取消发生在 fork 后，
        // marker 是否出现真正验证整个 process group，而非仅验证父 shell 退出。
        let script = r#"
            sh -c '
                printf ready > descendant-ready
                while [ ! -e release ]; do sleep 0.01; done
                printf escaped > descendant-marker
            ' &
            wait
        "#;
        let result = std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                run(
                    Path::new("/bin/sh"),
                    PreparedCommand {
                        args: vec!["-c".into(), script.into()],
                        stdin: None,
                    },
                    directory.path(),
                    &stopping,
                )
            });
            let deadline = Instant::now() + Duration::from_secs(5);
            while !directory.path().join("descendant-ready").exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            // 即使启动同步失败也先取消并 join，断言失败不能遗留测试子进程。
            stopping.store(true, Ordering::Relaxed);
            worker.join().unwrap()
        });
        // 正常组终止后不会有进程读取 release；回归为“只杀父进程”时后代将写 marker。
        fs::write(directory.path().join("release"), b"go").unwrap();
        let deadline = Instant::now() + Duration::from_millis(300);
        while !directory.path().join("descendant-marker").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            directory.path().join("descendant-ready").exists(),
            "测试后代未就绪"
        );
        let error = result.err().expect("取消后任务必须返回错误");
        assert!(error.to_string().contains("取消"), "{error:#}");
        assert!(
            !directory.path().join("descendant-marker").exists(),
            "取消后后代仍在运行并写入结果"
        );
    }

    #[cfg(unix)]
    #[test]
    fn excessive_text_output_terminates_a_still_running_command() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("output")).unwrap();
        let start = Instant::now();
        let result = run(
            Path::new("/bin/sh"),
            PreparedCommand {
                // 输出固定 3 MiB 后保持运行：必须由预算检查终止，不能靠自然退出通过。
                args: vec![
                    "-c".into(),
                    "dd if=/dev/zero bs=1048576 count=3 2>/dev/null; while :; do sleep 1; done"
                        .into(),
                ],
                stdin: None,
            },
            directory.path(),
            &AtomicBool::new(false),
        );
        let error = result.err().expect("超限文本输出必须被拒绝");
        assert!(
            error.to_string().contains("文本输出超过 2 MiB"),
            "{error:#}"
        );
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(
            fs::metadata(directory.path().join("stdout.log"))
                .unwrap()
                .len()
                > TEXT_LIMIT
        );
    }
}
