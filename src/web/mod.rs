//! 浏览器入口复用既有 CLI 领域能力；网络层不能接受任意本机路径或命令。
mod assets;
pub mod commands;
mod media_guard;
mod process;
mod server;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
#[command(after_help = r#"使用案例:
  在本机启动并自动打开浏览器；端口自动分配
    kito web
  指定本机端口，不自动打开浏览器
    kito web --port 4174 --no-open
  让系统分配空闲端口，复制终端中的完整地址访问
    kito web --port 0 --no-open

文件只在本机临时目录处理；下载需要的结果后，按 Ctrl+C 停止并清理。
仅监听 127.0.0.1，不提供外网或局域网访问。"#)]
pub struct WebArgs {
    /// 本机端口，0–65535；默认 0 让系统自动选择空闲端口。
    #[arg(long, default_value_t = 0)]
    pub port: u16,
    /// 不自动打开浏览器，只打印含会话凭证的本机地址。
    #[arg(long)]
    pub no_open: bool,
}

pub fn run(args: WebArgs) -> Result<bool> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?
        .block_on(server::serve(args.port, args.no_open))?;
    Ok(true)
}

fn open_browser(url: &str) {
    // URL 由监听地址和随机十六进制 token 生成；无 shell 拼接，也不接收用户程序名。
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    match result {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(_) => eprintln!("未能自动打开浏览器，请复制上方完整地址。"),
    }
}
