//! live 测试（要联网、要下几百 MB、要真跑 Java 子进程）的公用工具。
//!
//! 放在 `tests/live/` 而不是 `tests/common/`：后者是契约门用来解析源码与 TypeScript
//! 注释的，两类东西混在一个模块里，读的人就分不清哪条 helper 是给谁用的。
//! `tests/` 下的子目录不会被单独编成测试二进制，所以这里不会凭空多出一个 0 测试的目标。
//!
//! 每个 helper 只被部分 live 测试用到，所以整体 allow dead_code（同 `tests/common/`）。
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use compass_lib::api::ApiClient;
use compass_lib::install::Installer;
use compass_lib::meta::MetaClient;
use compass_lib::net::Downloader;
use compass_lib::paths::Paths;

/// 数据目录。给了 `COMPASS_*_DIR` 就复用上一次下载的东西（几百 MB 的库与资源），
/// 没给就用临时目录，测完即弃。
///
/// 持有 `TempDir` 是为了让它在测试结束前不被删掉——返回 `PathBuf` 的话目录会在
/// 测试还在跑的时候就消失，症状是一串莫名其妙的 IO 错误。
pub enum DataDir {
    Temp(tempfile::TempDir),
    Reused(PathBuf),
}

impl DataDir {
    pub fn path(&self) -> &Path {
        match self {
            DataDir::Temp(t) => t.path(),
            DataDir::Reused(p) => p,
        }
    }
}

pub fn data_dir(var: &str) -> DataDir {
    match std::env::var(var).ok() {
        Some(dir) if !dir.trim().is_empty() => DataDir::Reused(PathBuf::from(dir)),
        _ => DataDir::Temp(tempfile::tempdir().unwrap()),
    }
}

pub fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}

/// 用 lib 自己的 `http_client()`，不要在测试里 `reqwest::Client::new()`：
/// tauri 依赖 reqwest 0.13 而我们用 0.12，图里两份并存时测试目标里的
/// `reqwest::Client` 名字有歧义（CI 上就因此编不过），而且裸建的 client
/// 没有生产那份的超时与 User-Agent，等于没验到真正的配置。
pub fn http() -> reqwest::Client {
    compass_lib::api::http_client().unwrap()
}

pub fn api(http: &reqwest::Client) -> ApiClient {
    ApiClient::new(http.clone())
}

/// 不走镜像：live 测试要验的是官方源的真实形状，镜像同步滞后会把"格式变了"
/// 与"镜像旧了"混成一个分不清的失败。
pub fn installer(paths: &Paths, http: &reqwest::Client) -> Installer {
    Installer::new(
        MetaClient::new(http.clone(), String::new()),
        Downloader::new(http.clone(), 8, String::new()),
        paths.clone(),
    )
}

/// 找 Java。live 测试要真跑 processor 与游戏，`JAVA_HOME` 优先。
pub fn java_path() -> PathBuf {
    if let Ok(home) = std::env::var("JAVA_HOME")
        && !home.is_empty()
    {
        let p =
            PathBuf::from(&home)
                .join("bin")
                .join(if cfg!(windows) { "java.exe" } else { "java" });
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from(if cfg!(windows) { "java.exe" } else { "java" })
}

/// 列出一个目录树下所有文件名（不引 walkdir 依赖，测试里够用就行）。
pub fn walkdir_names(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else if let Some(n) = e.file_name().to_str() {
                out.push(n.to_string());
            }
        }
    }
    out.sort();
    out
}

/// 离线账号：live 测试不登录，但启动计划要有账号字段才能拼出来。
pub fn offline_account() -> compass_lib::account::Account {
    compass_lib::account::Account {
        player_name: "Tester".into(),
        player_uuid: "069a79f444e94726a5befca90e38aaf5".into(),
        access_token: "0".repeat(32),
        client_token: "0".repeat(32),
        api_url: "https://mc.example.com".into(),
        signed_in_at: 0,
        token_obtained_at: 0,
        kind: compass_lib::account::AccountKind::Yggdrasil,
    }
}

/// 一次真启动的结果。
pub struct RunOutcome {
    /// 进程退出码；被我们杀掉的（命中标记或超时）为 `None`。
    /// 用 `Option<Option<i32>>` 会更精确但没人需要——被杀掉时我们不关心码。
    pub exit: Option<i32>,
    pub lines: Vec<String>,
    pub elapsed: std::time::Duration,
    /// 命中的停止标记（若有）
    pub stopped_at: Option<String>,
}

impl RunOutcome {
    /// 末尾 n 行，用于失败信息（整份日志几千行，打印出来没人看）
    pub fn tail(&self, n: usize) -> String {
        let skip = self.lines.len().saturating_sub(n);
        self.lines[skip..].join("\n")
    }

    pub fn contains(&self, needle: &str) -> bool {
        self.lines.iter().any(|l| l.contains(needle))
    }

    /// 落盘一份完整日志，便于失败后离线看（CI 的 stdout 会被截断）
    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, self.lines.join("\n")).ok();
    }
}

/// 启动一个计划，边收输出边等：命中 `stop_at` 里任一标记、进程自己退出、或超过
/// `limit`，三者先到为准。之后一律杀进程。
///
/// **不能等进程自己退**：这台机器可能有显示服务，游戏会真开一个窗口一直跑下去。
/// stdout 与 stderr 是两个不同的类型，只能各自 spawn 一个 pump（合并进一个数组编不过）。
pub async fn run_and_capture(
    plan: &compass_lib::launch::LaunchPlan,
    limit: std::time::Duration,
    stop_at: &[&str],
) -> RunOutcome {
    use std::sync::{Arc, Mutex};

    let mut child = compass_lib::launch::spawn(plan).await.unwrap();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));
    if let Some(stream) = child.stdout.take() {
        let sink = Arc::clone(&log);
        tokio::spawn(pump_lines(stream, sink));
    }
    if let Some(stream) = child.stderr.take() {
        let sink = Arc::clone(&log);
        tokio::spawn(pump_lines(stream, sink));
    }

    let started = std::time::Instant::now();
    let mut stopped_at: Option<String> = None;
    let mut exit: Option<i32> = None;
    while started.elapsed() < limit {
        {
            let lines = log.lock().unwrap();
            if let Some(hit) = lines.iter().find_map(|l| {
                stop_at
                    .iter()
                    .find(|m| l.contains(*m))
                    .map(|m| m.to_string())
            }) {
                stopped_at = Some(hit);
            }
        }
        if stopped_at.is_some() {
            break;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                exit = status.code();
                break;
            }
            Ok(None) => {}
            Err(e) => panic!("wait failed: {e}"),
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    let _ = child.kill().await;
    // 杀完之后 pump 还会把管道里剩下的读出来，给它一点时间，否则最后几行
    // （往往正是崩溃原因）会丢
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    RunOutcome {
        exit,
        lines: log.lock().unwrap().clone(),
        elapsed: started.elapsed(),
        stopped_at,
    }
}

async fn pump_lines<R>(stream: R, sink: std::sync::Arc<std::sync::Mutex<Vec<String>>>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    use tokio::io::AsyncBufReadExt;
    let mut lines = tokio::io::BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        sink.lock().unwrap().push(line);
    }
}
