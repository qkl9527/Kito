fn main() {
    match kito::cli::run() {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("错误：{error:#}");
            std::process::exit(1);
        }
    }
}
