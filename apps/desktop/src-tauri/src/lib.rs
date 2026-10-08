//! byte-git 桌面端（Tauri v2）。
//!
//! 前端为 git-mate 移植的 Vue 3 应用，通过 `src/wailsjs/go/main/ByteGitApp.ts`
//! 适配层调用本 crate 暴露的 Tauri 命令（方法名 snake_case 化）。

mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::version,
            commands::info_summary,
            commands::git_status,
            commands::branch_list,
            commands::branch_clean,
            commands::stash_list,
            commands::stash_save,
            commands::stash_pop,
            commands::license_list,
            commands::license_create,
            commands::ignore_list,
            commands::ignore_add,
            commands::config_get,
            commands::config_set,
        ])
        .run(tauri::generate_context!())
        .expect("byte-git 桌面端启动失败");
}
