//! 实例的游戏日志与崩溃报告：列出来、读出来、送去诊断。
//!
//! 三处输入都是纯文本，共用一套解析：游戏进程实时的 stdout（见 `logconfig` 为什么
//! 把控制台布局换成纯文本）、滚动归档 `logs/latest.log` 与 `logs/*.log.gz`、以及
//! `crash-reports/*.txt`。
//!
//! **两个尺寸上限是刻意的**，而且分别服务不同目的：
//! - 给 UI 看的那份只取**末尾** `MAX_VIEW_BYTES`。一个跑了几小时的 `latest.log`
//!   可以有几十 MB，整份塞进 IPC 再渲染会让界面卡死，而用户要看的几乎总在末尾。
//! - 给诊断的那份取 `MAX_DIAGNOSE_BYTES`，超了也是取末尾并标记 `partial`。
//!   代价是：一份超过上限的日志里，**早期**的告警（例如"某个 mod 被跳过了"）会漏掉。
//!   这是取舍而不是疏忽——把整份读进来的替代方案是在 pathological 情况下拖垮启动器。
//!
//! 文件名一律来自前端，所以读取走 `paths::safe_join`：一个 `../../compass.json`
//! 不该因为"日志查看器"这个入口就能读到实例目录之外的东西。

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::diagnose::{self, Finding};
use crate::error::Error;
use crate::instance;
use crate::paths::{Paths, safe_join};

/// 给 UI 的读取上限（取末尾）。
pub const MAX_VIEW_BYTES: u64 = 512 * 1024;
/// 诊断的读取上限（超了取末尾）。
pub const MAX_DIAGNOSE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LogKind {
    /// `logs/latest.log` 与滚动出去的 `logs/*.log.gz`
    GameLog,
    /// `crash-reports/*.txt`
    CrashReport,
}

/// 一份归档的元数据。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogArchive {
    /// **相对实例游戏目录**的路径（如 `logs/latest.log`）。读取时用它，
    /// 而且只接受它——绝对路径会被 `safe_join` 拒掉。
    pub name: String,
    pub kind: LogKind,
    /// Unix 秒
    pub modified_at: u64,
    /// 磁盘上的字节数。`.gz` 是压缩后的大小，与读出来的文本长度不是一回事。
    pub size: u64,
    pub compressed: bool,
}

/// 读出来的一段日志。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogText {
    pub name: String,
    pub text: String,
    /// 文件比上限大，`text` 只是**末尾**那一段
    pub truncated: bool,
}

/// 一次诊断的结果。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    /// 诊断所依据的那份文件（相对路径）。没有日志时为空——那本身也是要告诉用户的：
    /// "没有日志"和"日志里没查出问题"是两件事。
    pub source: Option<String>,
    pub findings: Vec<Finding>,
    /// 文件超过诊断上限，只看了末尾
    pub partial: bool,
}

/// 列出这个实例的日志与崩溃报告，新的在前。
///
/// 目录不存在返回空列表而不是报错：刚建好的实例还没跑过，"没有日志"是正常状态，
/// 不该在 UI 上显示成一条错误。
pub fn list(paths: &Paths, slug: &str) -> Result<Vec<LogArchive>, Error> {
    let game = instance::game_dir(paths, slug)?;
    let mut out = Vec::new();
    collect(&game.join("logs"), LogKind::GameLog, &mut out);
    collect(&game.join("crash-reports"), LogKind::CrashReport, &mut out);
    out.sort_by_key(|a| std::cmp::Reverse(a.modified_at));
    Ok(out)
}

fn collect(dir: &Path, kind: LogKind, out: &mut Vec<LogArchive>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_string());
        let Some(file_name) = name else { continue };
        let compressed = file_name.ends_with(".gz");
        // 只收日志形状的文件：logs/ 下还会有别的（例如某些 mod 自己的输出），
        // 而崩溃报告目录里只认 .txt
        let wanted = match kind {
            LogKind::GameLog => {
                file_name.ends_with(".log") || file_name.ends_with(".txt") || compressed
            }
            LogKind::CrashReport => file_name.ends_with(".txt"),
        };
        if !wanted {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let dir_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(LogArchive {
            name: format!("{dir_name}/{file_name}"),
            kind,
            modified_at: modified,
            size: meta.len(),
            compressed,
        });
    }
}

/// 读一份日志的**末尾**（上限 `MAX_VIEW_BYTES`）。
pub fn read(paths: &Paths, slug: &str, name: &str) -> Result<LogText, Error> {
    read_bounded(paths, slug, name, MAX_VIEW_BYTES)
}

fn read_bounded(paths: &Paths, slug: &str, name: &str, max: u64) -> Result<LogText, Error> {
    let game = instance::game_dir(paths, slug)?;
    let file = safe_join(&game, name)?;
    if !file.is_file() {
        return Err(Error::NotFound(format!("no such log: {name}")));
    }
    let (text, truncated) = if name.ends_with(".gz") {
        read_gz_tail(&file, max)?
    } else {
        read_plain_tail(&file, max)?
    };
    Ok(LogText {
        name: name.to_string(),
        text,
        truncated,
    })
}

/// 纯文本取末尾。按字节切会切在多字节字符中间，所以先按字节取窗口，
/// 再用 `from_utf8` 的容错版本丢掉开头那半个字符。
fn read_plain_tail(file: &Path, max: u64) -> Result<(String, bool), Error> {
    let len = fs::metadata(file)?.len();
    let truncated = len > max;
    let bytes = if truncated {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = fs::File::open(file)?;
        f.seek(SeekFrom::End(-(max as i64)))?;
        let mut buf = Vec::with_capacity(max as usize);
        f.read_to_end(&mut buf)?;
        buf
    } else {
        fs::read(file)?
    };
    Ok((lossy_from_tail(bytes), truncated))
}

/// gzip 不能随机访问；流式解压，用有界队列仅保留最近的字节。
fn read_gz_tail(file: &Path, max: u64) -> Result<(String, bool), Error> {
    let f = fs::File::open(file)?;
    let mut decoder = flate2::read::GzDecoder::new(f);
    let mut buf = std::collections::VecDeque::new();
    let mut chunk = [0u8; 64 * 1024];
    let mut truncated = false;
    loop {
        match decoder.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend(&chunk[..n]);
                if buf.len() as u64 > max {
                    truncated = true;
                    buf.drain(..buf.len() - max as usize);
                }
            }
            // 半截的 .gz（游戏被强杀时正在滚动）也要把已解出来的部分给用户，
            // 报错会让"日志明明在那儿却打不开"变成一条看不懂的 IO 错误
            Err(_) => {
                truncated = true;
                break;
            }
        }
    }
    Ok((lossy_from_tail(buf.into_iter().collect()), truncated))
}

/// 从字节窗口还原文本：开头可能是被切断的多字节字符，丢掉它而不是显示成 U+FFFD。
fn lossy_from_tail(bytes: Vec<u8>) -> String {
    // 只丢掉窗口开头的 UTF-8 continuation bytes；内部坏字节以替代字符显示，
    // 不能因为后面的解码错误丢掉前面的正常日志。
    let start = bytes.iter().take_while(|b| **b & 0xc0 == 0x80).count();
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

/// 诊断这个实例：优先看最新的崩溃报告，没有就退到 `logs/latest.log`。
///
/// 顺序是有意的：崩溃报告是游戏自己写的"这次为什么死了"，比从几万行 stdout 里
/// 找线索可靠。但实测**常见失败根本不写崩溃报告**（JVM 启动阶段、loader 的 mod 解析
/// 阶段都在报告器之外），所以回退到 latest.log 不是权宜，是主路径之一。
pub fn diagnose_latest(paths: &Paths, slug: &str) -> Result<Diagnosis, Error> {
    let archives = list(paths, slug)?;
    let pick = archives
        .iter()
        .find(|a| a.kind == LogKind::CrashReport)
        .or_else(|| archives.iter().find(|a| a.name == "logs/latest.log"))
        .or_else(|| archives.iter().find(|a| a.kind == LogKind::GameLog));
    let Some(a) = pick else {
        return Ok(Diagnosis {
            source: None,
            findings: Vec::new(),
            partial: false,
        });
    };
    let got = read_bounded(paths, slug, &a.name, MAX_DIAGNOSE_BYTES)?;
    let mut findings = diagnose::diagnose(&got.text);
    // 崩溃报告本身是"发生了什么"的权威陈述，即使规则库一条都没匹配上，
    // 也要把它的 Description 行给用户看——否则面板会显示"没查出问题"，
    // 而事实上游戏明确写了一句原因
    if a.kind == LogKind::CrashReport
        && findings.is_empty()
        && let Some(desc) = crash_description(&got.text)
    {
        findings.push(Finding {
            code: "crash_report_unrecognized",
            severity: diagnose::Severity::Fatal,
            evidence: desc,
            action: Some(diagnose::Action::OpenLogs),
        });
    }
    Ok(Diagnosis {
        source: Some(a.name.clone()),
        findings,
        partial: got.truncated,
    })
}

/// 崩溃报告里的 `Description: <一句话>`。这是报告里唯一一行"人话摘要"。
fn crash_description(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("Description:"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 日志目录的绝对路径（"打开目录"按钮用）。
pub fn dir(paths: &Paths, slug: &str) -> Result<PathBuf, Error> {
    Ok(instance::game_dir(paths, slug)?.join("logs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Paths, String) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("data"));
        paths.ensure().unwrap();
        let slug = "logtest".to_string();
        let inst = crate::instance::Instance {
            slug: slug.clone(),
            name: "Log test".into(),
            version_id: "1.21.4".into(),
            ..Default::default()
        };
        crate::instance::save(&paths, &inst).unwrap();
        (dir, paths, slug)
    }

    fn game(paths: &Paths, slug: &str) -> PathBuf {
        instance::game_dir(paths, slug).unwrap()
    }

    /// 没跑过的实例是"没有日志"，不是"出错了"：UI 要能区分这两件事，
    /// 所以这里必须是空列表而不是 Err。
    #[test]
    fn an_instance_that_never_ran_has_no_logs_and_that_is_not_an_error() {
        let (_d, paths, slug) = setup();
        assert_eq!(list(&paths, &slug).unwrap(), vec![]);
        let diag = diagnose_latest(&paths, &slug).unwrap();
        assert_eq!(diag.source, None);
        assert!(diag.findings.is_empty());
    }

    #[test]
    fn lists_game_logs_and_crash_reports_newest_first() {
        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        fs::create_dir_all(g.join("crash-reports")).unwrap();
        fs::write(g.join("logs/latest.log"), b"[20:00:00] [main/INFO]: hi\n").unwrap();
        fs::write(g.join("logs/2026-10-28-1.log.gz"), b"\x1f\x8b").unwrap();
        fs::write(
            g.join("crash-reports/crash-2026-10-30_20.00.00-client.txt"),
            b"---- Minecraft Crash Report ----\nDescription: Initializing game\n",
        )
        .unwrap();
        // 不是日志的东西不该混进来
        fs::write(g.join("logs/some-mod-output.bin"), b"x").unwrap();

        let got = list(&paths, &slug).unwrap();
        let names: Vec<&str> = got.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names.len(), 3, "{names:?}");
        assert!(names.contains(&"logs/latest.log"));
        assert!(names.contains(&"logs/2026-10-28-1.log.gz"));
        assert!(
            names.contains(&"crash-reports/crash-2026-10-30_20.00.00-client.txt"),
            "{names:?}"
        );
        assert!(
            got.iter()
                .find(|a| a.name.ends_with(".gz"))
                .is_some_and(|a| a.compressed),
            ".gz 要标成压缩的（UI 显示的体积与读出来的文本长度不是一回事）"
        );
        // 名字必须是相对路径：读取靠它，而绝对路径会被 safe_join 拒掉
        assert!(
            got.iter()
                .all(|a| !a.name.starts_with('/') && !a.name.contains('\\'))
        );
        // 新的在前
        assert!(
            got.windows(2).all(|w| w[0].modified_at >= w[1].modified_at),
            "{got:?}"
        );
    }

    /// 文件名来自前端，所以必须是"这个实例目录内的相对路径"。
    /// 一个 `../../compass.json` 不该因为日志查看器这个入口就能读到别处。
    #[test]
    fn reading_escapes_neither_the_instance_nor_the_logs() {
        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        fs::write(g.join("logs/latest.log"), b"fine\n").unwrap();
        fs::write(paths.root().join("compass.json"), b"{\"secret\":true}").unwrap();

        for bad in [
            "../../compass.json",
            "../compass.json",
            "logs/../../compass.json",
            "/etc/passwd",
            "logs/nope.log",
        ] {
            let got = read(&paths, &slug, bad);
            assert!(
                got.is_err(),
                "{bad:?} 不该读得到，却拿到了 {:?}",
                got.unwrap().text
            );
        }
        assert_eq!(
            read(&paths, &slug, "logs/latest.log").unwrap().text,
            "fine\n"
        );
    }

    /// 超过上限只给末尾，并且**必须说清楚是截断的**：
    /// 用户看到一段没有开头的日志，如果不知道被截断了，会以为游戏就打了这么几句。
    #[test]
    fn a_huge_log_is_read_from_the_tail_and_reported_as_truncated() {
        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        let mut big = String::new();
        for i in 0..40_000 {
            big.push_str(&format!("[20:00:00] [main/INFO]: line {i}\n"));
        }
        fs::write(g.join("logs/latest.log"), big.as_bytes()).unwrap();
        assert!(big.len() as u64 > MAX_VIEW_BYTES);

        let got = read(&paths, &slug, "logs/latest.log").unwrap();
        assert!(got.truncated, "应当标成截断");
        assert!((got.text.len() as u64) <= MAX_VIEW_BYTES + 256);
        assert!(
            got.text.contains("line 39999"),
            "末尾那行必须在：{}",
            &got.text[..80]
        );
        assert!(
            !got.text.contains("line 0\n"),
            "开头不该在里面（取的是末尾）"
        );
        // 按字节切窗口会切在多字节字符中间：不该留下 U+FFFD
        assert!(!got.text.contains('\u{fffd}'), "开头被切坏了");
    }

    /// 多字节字符正好落在窗口边界上也不能出乱码。
    #[test]
    fn the_tail_window_never_starts_inside_a_multibyte_character() {
        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        // 每行都是中文，任何字节偏移都极可能落在字符中间
        let line = "[20:00:00] [main/INFO]: 中文日志内容测试\n";
        let big = line.repeat((MAX_VIEW_BYTES as usize / line.len()) + 100);
        fs::write(g.join("logs/latest.log"), big.as_bytes()).unwrap();

        let got = read(&paths, &slug, "logs/latest.log").unwrap();
        assert!(got.truncated);
        assert!(
            !got.text.contains('\u{fffd}'),
            "窗口开头切进了一个多字节字符：{:?}",
            got.text.chars().take(20).collect::<String>()
        );
        assert!(got.text.contains("中文日志内容测试"));
    }

    /// `.gz` 要能解压出来读。游戏滚动日志就是这个格式，
    /// 不支持的话"历史日志"这一整块功能是空的。
    #[test]
    fn gzipped_rolled_logs_are_decompressed() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        let body = "[20:00:00] [main/ERROR]: java.lang.OutOfMemoryError: Java heap space\n";
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(body.as_bytes()).unwrap();
        let gz = enc.finish().unwrap();
        fs::write(g.join("logs/2026-10-28-1.log.gz"), &gz).unwrap();

        let got = read(&paths, &slug, "logs/2026-10-28-1.log.gz").unwrap();
        assert_eq!(got.text, body);
        assert!(!got.truncated);

        // 诊断也能直接吃 .gz（用户往往只有滚动出去的那一份）
        let diag = diagnose_latest(&paths, &slug).unwrap();
        assert_eq!(diag.source.as_deref(), Some("logs/2026-10-28-1.log.gz"));
        assert_eq!(diag.findings[0].code, "out_of_memory");
    }

    #[test]
    fn large_compressed_logs_keep_the_last_line() {
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;
        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        let mut body = "old log entry\n".repeat(MAX_VIEW_BYTES as usize / 7);
        body.push_str("[20:00:00] [main/ERROR]: final failure\n");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(body.as_bytes()).unwrap();
        fs::write(g.join("logs/large.log.gz"), encoder.finish().unwrap()).unwrap();
        let got = read(&paths, &slug, "logs/large.log.gz").unwrap();
        assert!(got.truncated);
        assert!(got.text.ends_with("final failure\n"));
        assert!(got.text.len() as u64 <= MAX_VIEW_BYTES);
    }

    #[test]
    fn invalid_utf8_keeps_valid_text_before_and_after_it() {
        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        fs::write(
            g.join("logs/latest.log"),
            b"first useful line\n\xfflast useful line\n",
        )
        .unwrap();
        let got = read(&paths, &slug, "logs/latest.log").unwrap();
        assert!(got.text.starts_with("first useful line\n"));
        assert!(got.text.ends_with("last useful line\n"));
    }

    /// 半截的 .gz（游戏被强杀时正在滚动）要把已解出来的部分给出来，
    /// 而不是报一条看不懂的 IO 错误。
    #[test]
    fn a_truncated_gz_still_yields_what_could_be_decompressed() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(b"first part of the log\n").unwrap();
        let gz = enc.finish().unwrap();
        // 截掉尾部若干字节，模拟写一半被杀
        fs::write(g.join("logs/broken.log.gz"), &gz[..gz.len() - 3]).unwrap();

        let got = read(&paths, &slug, "logs/broken.log.gz");
        assert!(got.is_ok(), "半截 .gz 不该报错：{got:?}");
    }

    /// 崩溃报告优先于 latest.log：报告是游戏自己写的"这次为什么死了"。
    #[test]
    fn a_crash_report_outranks_the_game_log() {
        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        fs::create_dir_all(g.join("crash-reports")).unwrap();
        fs::write(
            g.join("logs/latest.log"),
            b"[20:00:00] [main/INFO]: all good\n",
        )
        .unwrap();
        fs::write(
            g.join("crash-reports/crash-2026-10-30_20.00.00-client.txt"),
            b"---- Minecraft Crash Report ----\nDescription: Ticking entity\n\njava.lang.NullPointerException\n",
        )
        .unwrap();

        let diag = diagnose_latest(&paths, &slug).unwrap();
        assert!(
            diag.source
                .as_deref()
                .is_some_and(|s| s.starts_with("crash-reports/")),
            "{diag:?}"
        );
        // 规则库一条都没匹配上时，也要把报告自己那句 Description 给用户，
        // 否则面板会说"没查出问题"，而游戏明明写了一句原因
        assert_eq!(diag.findings.len(), 1, "{diag:?}");
        assert_eq!(diag.findings[0].code, "crash_report_unrecognized");
        assert_eq!(diag.findings[0].evidence, "Ticking entity");
    }

    /// 没有崩溃报告时退回 latest.log，而且诊断确实是从真日志里认出来的。
    #[test]
    fn falls_back_to_the_game_log_and_diagnoses_it() {
        let (_d, paths, slug) = setup();
        let g = game(&paths, &slug);
        fs::create_dir_all(g.join("logs")).unwrap();
        fs::write(
            g.join("logs/latest.log"),
            b"[20:00:00] [main/INFO]: booting\nException in thread \"main\" java.lang.OutOfMemoryError: Java heap space\n",
        )
        .unwrap();

        let diag = diagnose_latest(&paths, &slug).unwrap();
        assert_eq!(diag.source.as_deref(), Some("logs/latest.log"));
        assert!(!diag.partial);
        assert_eq!(diag.findings[0].code, "out_of_memory");
    }
}
