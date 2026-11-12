//! Mythic Compass 的 Rust 核心。
//!
//! 分层（计划文档 P0 决策 2）：重逻辑全在这里——路径布局、配置、账号、后端 HTTP，
//! 以及后续几个阶段的版本解析、下载调度、启动参数构造与进程管理。
//! 前端只做视图，通过**具名命令**取数据；不提供"转发任意 URL"的命令。

pub mod account;
pub mod api;
pub mod archive;
pub mod assets;
pub mod authlibinjector;
pub mod cli;
pub mod commands;
pub mod config;
pub mod content;
pub mod diagnose;
pub mod error;
pub mod forge;
pub mod install;
pub mod instance;
pub mod java;
pub mod javaget;
pub mod launch;
pub mod loader;
pub mod logconfig;
pub mod logs;
pub mod meta;
pub mod modpack;
pub mod net;
pub mod oplog;
pub mod paths;
pub mod runner;
pub mod session;
pub mod state;
pub mod version;

use tauri::Manager;

use crate::api::ApiClient;
use crate::paths::Paths;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 原生文件选择器：导入 mod/资源包、指定 Java 路径都靠它。
        // 不用 HTML `<input type=file>`——webview 拿到的是不透明句柄，
        // 而复制文件需要真实的文件系统路径。
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // The updater is optional: an unsigned/local build has no plugin
            // configuration and must still open instead of failing at startup.
            if app
                .config()
                .plugins
                .0
                .get("updater")
                .and_then(|value| value.get("pubkey"))
                .and_then(|value| value.as_str())
                .is_some_and(|key| !key.trim().is_empty())
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            // 便携模式（exe 旁边有 portable.txt）优先，否则用平台的用户数据目录。
            // 取不到 exe 路径时退回后者：便携模式是可选便利，不该成为启动的前置条件。
            let root = std::env::current_exe()
                .ok()
                .map(|exe| exe.with_file_name(paths::PORTABLE_MARKER))
                .and_then(|marker| paths::portable_root(&marker))
                .map_or_else(|| app.path().app_data_dir(), Ok)?;
            let paths = Paths::new(root);
            paths.ensure().map_err(|e| e.to_string())?;
            let (account, account_warning) = account::load(&paths);

            // 配置损坏不阻止启动：用默认值起来，并把原因交给设置页显示一次。
            // 直接崩不该由一个坏 JSON 造成，静默回落又会让用户以为设置"莫名消失"。
            let (config, warning) = match config::load(&paths) {
                Ok(c) => (c, None),
                Err(e) => (
                    config::Config::default(),
                    Some(format!(
                        "{} 无法读取，已回落到默认设置：{e}",
                        paths.config_file().display()
                    )),
                ),
            };
            // accounts.json 损坏的警告走同一机制（文件本身保留，见 account::load）
            let warning = match (warning, account_warning) {
                (Some(a), Some(b)) => Some(format!("{a}\n{b}")),
                (a, b) => a.or(b),
            };

            let http = api::http_client().map_err(|e| e.to_string())?;
            let api = ApiClient::new(http.clone());
            app.manage(AppState::new(paths, api, http, config, account, warning));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::paths_info,
            commands::take_startup_warning,
            commands::auth_login,
            commands::auth_logout,
            commands::auth_check,
            commands::accounts_list,
            commands::account_add_offline,
            commands::account_set_active,
            commands::account_remove,
            commands::credentials_list,
            commands::credentials_create,
            commands::credentials_delete,
            commands::instance_logs,
            commands::instance_log_read,
            commands::instance_diagnose,
            commands::instance_logs_dir,
            commands::operation_log,
            commands::operation_log_path,
            commands::cli_intent,
            commands::update_configured,
            commands::java_overview,
            commands::java_download,
            commands::java_remove,
            commands::mirror_skin_data_url,
            commands::hoard_search,
            commands::hoard_versions,
            commands::hoard_install,
            commands::versions_list,
            commands::versions_installed,
            commands::install_version,
            commands::instance_list,
            commands::instance_create,
            commands::instance_delete,
            commands::instance_update,
            commands::instance_duplicate,
            commands::instance_export,
            commands::instance_inspect_archive,
            commands::instance_import,
            commands::modpack_inspect,
            commands::modpack_install,
            commands::modpack_export,
            commands::instance_java,
            commands::instance_server_status,
            commands::content_list,
            commands::content_set_enabled,
            commands::content_import,
            commands::content_remove,
            commands::content_check,
            commands::loader_versions,
            commands::loader_install,
            commands::loader_remove,
            commands::instance_launch,
            commands::instance_stop,
            commands::game_running
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
