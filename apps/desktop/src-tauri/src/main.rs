// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 上游 03915e0 起钩子统一 shebang 直执行（git 原生机制），
    // 不再有 `<exe> hook exec` 的 wrapper 回调分发，exe 仅作为 GUI 启动器。
    byte_git_desktop_lib::run()
}
