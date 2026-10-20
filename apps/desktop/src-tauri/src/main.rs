// Windows 无控制台窗口：Tauri 模板默认仅 release 隐藏（debug 构建会带黑窗），
// byte-git 日常即以 debug 构建交付体验，故无条件 windows 子系统。
// 代价：eprintln 日志无处显示——需要排查时临时改回 cfg_attr 或挂文件日志。
#![windows_subsystem = "windows"]

fn main() {
    // 上游 03915e0 起钩子统一 shebang 直执行（git 原生机制），
    // 不再有 `<exe> hook exec` 的 wrapper 回调分发，exe 仅作为 GUI 启动器。
    byte_git_desktop_lib::run()
}
