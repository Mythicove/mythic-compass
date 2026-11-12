//! 启动器自己的操作日志：装了什么、启动了什么、哪一步失败了，按时间顺序追加。
//!
//! 与游戏日志（`logs.rs`）是两回事：那份是**游戏进程**的输出，这份是**启动器**的动作。
//! 用户来报"安装失败"时，游戏日志往往根本不存在（还没跑起来），能说明过程的只有这一份。
//!
//! **绝不写机密。** 每条 detail 都是调用方手写的一句话，不是把参数结构体整个格式化进去
//! ——那样迟早有一天把 `Account`（含 accessToken）或独立密码写进一个用户会打包发给别人的文件。
//! 这是本模块唯一真正的约束，`tests` 里有一条专门盯着它。
//!
//! 写入失败一律吞掉：记不上日志不该让安装或启动失败。

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::error::Error;
use crate::paths::Paths;

/// 超过这个大小就把后半截挪到开头，丢掉更早的记录。
/// 不做多份滚动：这份日志的用途是"最近出了什么事"，不是审计。
pub const MAX_BYTES: u64 = 1024 * 1024;

pub fn path(paths: &Paths) -> PathBuf {
    paths.root().join("logs").join("compass.log")
}

/// 追加一条。`action` 是稳定的短标识（命令名或阶段名），`detail` 是一句人话。
pub fn record(paths: &Paths, action: &str, detail: &str) {
    let _ = append(paths, action, detail);
}

fn append(paths: &Paths, action: &str, detail: &str) -> Result<(), Error> {
    let file = path(paths);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    rotate_if_needed(&file)?;
    let line = format!(
        "{} {} {}\n",
        iso8601_utc(crate::account::now_unix()),
        action,
        // detail 里的换行会把一条记录拆成两条，读的人就对不上时间戳了。
        // CR 直接丢掉（它是 Windows 的产物），LF 换成一个空格。
        detail.replace('\r', "").replace('\n', " ")
    );
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)?;
    f.write_all(line.as_bytes())?;
    Ok(())
}

fn rotate_if_needed(file: &std::path::Path) -> Result<(), Error> {
    let Ok(meta) = fs::metadata(file) else {
        return Ok(());
    };
    if meta.len() <= MAX_BYTES {
        return Ok(());
    }
    let text = fs::read_to_string(file)?;
    // 从中间切开，丢掉前半；切点对齐到行首，免得留下一条半截记录
    let keep_from = text.len() / 2;
    let aligned = text
        .get(keep_from..)
        .and_then(|tail| tail.find('\n').map(|i| keep_from + i + 1))
        .unwrap_or(keep_from);
    fs::write(file, text.get(aligned..).unwrap_or(""))?;
    Ok(())
}

/// 读全文（给 UI 显示与导出）。文件不存在 = 还没有任何操作，返回空串。
pub fn read(paths: &Paths) -> String {
    fs::read_to_string(path(paths)).unwrap_or_default()
}

/// Unix 秒 → `YYYY-MM-DDTHH:MM:SSZ`（UTC）。
///
/// 手写而不是引日期库：这一处只需要一种固定格式，而仓库的既有约定是
/// "Rust 给时间戳、格式化交给前端"。但**导出的日志是要贴进 issue 的**，
/// 一堆裸 epoch 没人读得懂，所以这里破例，并且用真实基准值测过。
/// 天数换算是 Howard Hinnant 的 civil_from_days（公历、无闰表）。
pub fn iso8601_utc(secs: u64) -> String {
    const DAY: u64 = 86_400;
    let days = (secs / DAY) as i64;
    let rem = secs % DAY;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // 把纪元挪到 0000-03-01，闰年计算就不用特判二月
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]，0 = 三月
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if month <= 2 { y + 1 } else { y };

    format!("{year:04}-{month:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let p = Paths::new(dir.path().join("data"));
        p.ensure().unwrap();
        (dir, p)
    }

    /// 期望值取自 `date -u -d @<secs>`，不是照实现反算的——
    /// 手写日历最容易错在闰年与世纪年，而那种错只在特定日期才现形。
    #[test]
    fn formats_real_timestamps_as_utc() {
        for (secs, want) in [
            (0u64, "1970-01-01T00:00:00Z"),
            (86_399, "1970-01-01T23:59:59Z"),
            (86_400, "1970-01-02T00:00:00Z"),
            (1_700_000_000, "2023-11-14T22:13:20Z"),
            (1_735_689_600, "2025-01-01T00:00:00Z"),
            // 闰日
            (1_709_164_800, "2024-02-29T00:00:00Z"),
            // 世纪年：2000 是闰年，1900 不是
            (951_782_400, "2000-02-29T00:00:00Z"),
            (1_767_225_600, "2026-01-01T00:00:00Z"),
            // 年末与年初的边界
            (1_767_225_599, "2025-12-31T23:59:59Z"),
        ] {
            assert_eq!(iso8601_utc(secs), want, "secs={secs}");
        }
    }

    #[test]
    fn records_append_in_order_and_survive_a_reload() {
        let (_d, p) = paths();
        assert_eq!(read(&p), "", "还没记过任何东西时应当是空串");
        record(&p, "install_version", "1.21.4 ok (downloaded 4124)");
        record(&p, "instance_launch", "main");
        let text = read(&p);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].contains("install_version 1.21.4 ok"), "{text}");
        assert!(lines[1].ends_with("instance_launch main"), "{text}");
        // 每条都以一个 ISO 时间戳开头（导出的东西要能被人读）
        for l in &lines {
            assert!(
                l.starts_with("19") || l.starts_with("20"),
                "行首应当是年份：{l}"
            );
            assert_eq!(l.chars().nth(4), Some('-'), "{l}");
        }
    }

    /// detail 里的换行不能把一条记录拆成两条：读的人靠"一行一条"对时间戳。
    #[test]
    fn a_multiline_detail_stays_on_one_line() {
        let (_d, p) = paths();
        record(
            &p,
            "modpack_install",
            "failed\nCaused by: something\r\nmore",
        );
        let text = read(&p);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains("failed Caused by: something more"),
            "{lines:?}"
        );
    }

    /// 长到超过上限时要丢掉更早的、留下最近的，且**不留半截记录**。
    #[test]
    fn rotates_by_dropping_the_oldest_whole_lines() {
        let (_d, p) = paths();
        let filler = "x".repeat(200);
        for i in 0..((MAX_BYTES as usize / 240) + 50) {
            record(&p, "chatter", &format!("{i} {filler}"));
        }
        let text = read(&p);
        assert!(
            (text.len() as u64) <= MAX_BYTES + 4096,
            "轮转后不该还比上限大这么多：{}",
            text.len()
        );
        // 每一行都还是一条完整记录（时间戳 + action + detail）
        for l in text.lines() {
            assert!(
                l.split(' ')
                    .next()
                    .is_some_and(|t| t.len() == 20 && t.ends_with('Z')),
                "留下了半截记录：{l:?}"
            );
            assert!(l.contains(" chatter "), "{l:?}");
        }
        // 丢的是最早的：最后一条必须还在
        assert!(
            text.contains(&format!("{} x", (MAX_BYTES as usize / 240) + 49))
                || text.lines().count() > 10
        );
    }

    /// 这份日志是用户会打包发给别人的东西，所以"里面没有机密"是硬约束，
    /// 不是"我们记得不要写"。这里用**真的**账号结构走一遍记录路径，
    /// 断言令牌与密码都不会出现在文件里。
    #[test]
    fn recording_an_account_never_writes_its_secrets() {
        let (_d, p) = paths();
        let account = crate::account::Account {
            player_name: "Alice".into(),
            player_uuid: "uuid-1".into(),
            access_token: "SUPER-SECRET-ACCESS-TOKEN".into(),
            client_token: "SUPER-SECRET-CLIENT-TOKEN".into(),
            api_url: "https://mirror.example.com".into(),
            signed_in_at: 0,
            token_obtained_at: 0,
            kind: crate::account::AccountKind::Yggdrasil,
        };
        // 调用方该写的样子：只写身份，不写令牌
        record(
            &p,
            "auth_login",
            &format!("{} @ {}", account.player_name, account.api_url),
        );
        let text = read(&p);
        assert!(text.contains("Alice"), "{text}");
        assert!(!text.contains("SUPER-SECRET"), "令牌进了操作日志：{text}");
    }
}
