// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // CLI 分发：git 钩子 wrapper 会以 `<exe> hook exec <name> [args...]`
    // 调用本程序（core::hook::install 生成 wrapper 时用 current_exe()）。
    // 带该参数时执行钩子并以退出码返回 git，不启动 GUI。
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "hook" && args[2] == "exec" {
        let hook_name = args[3].clone();
        let rest: Vec<String> = args[4..].to_vec();
        let repo = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        match byte_git_core::hook::exec(&repo, &hook_name, &rest) {
            Ok(result) => {
                if !result.error.is_empty() {
                    eprint!("{}", result.error);
                }
                std::process::exit(result.exit_code);
            }
            Err(e) => {
                eprintln!("钩子执行失败: {e}");
                std::process::exit(1);
            }
        }
    }

    byte_git_desktop_lib::run()
}
