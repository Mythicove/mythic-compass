//! 我们自己写的 log4j2 客户端配置，替代 Mojang 的 `logging.client.file`。
//!
//! **为什么不用 Mojang 那个**：它的控制台 appender 是 `<LegacyXMLLayout />`，于是 stdout 是
//! `<log4j:Message><![CDATA[Setting user: …]]></log4j:Message>` 这种 XML；而它写
//! `logs/latest.log` 用的是纯文本 `PatternLayout`。同一个进程两种形状，意味着日志查看器
//! 与崩溃规则库要么写两套解析，要么先把 XML 转回文本——而 `crash-reports/*.txt`
//! 又始终是纯文本。统一成纯文本之后，**实时 stdout、归档 latest.log、崩溃报告三处
//! 共用一个解析器与一套规则**，这是本模块存在的唯一理由。
//!
//! **除控制台布局外，与 Mojang 的 `client-1.21.2.xml` 逐项一致**（下面每条都有测试盯着）：
//! - `File` appender 仍写 `logs/latest.log`，滚动到 `logs/%d{yyyy-MM-dd}-%i.log.gz`。
//!   动它就等于把"日志归档"这个功能悄悄关掉——游戏不再落盘，而我们的归档列表读的就是那里。
//! - `NETWORK_PACKETS` 标记仍被 DENY：少了它，开着数据包调试时日志会被刷爆。
//! - `Tracy` listener 保留：那是游戏自带的 profiler 钩子，删掉不会有症状，
//!   但也没理由替 Mojang 做这个决定。
//! - `%msg{nolookups}` 保留：这是 Log4Shell（CVE-2021-44228）的缓解，不是装饰。
//!
//! **只对声明了 `logging.client` 的版本生效**。更老的版本（1.12.2、1.7.10）没有这套机制，
//! 游戏用的是自己内置的配置，控制台本来就是纯文本；给它们塞一份带 `Tracy` 与
//! `RollingRandomAccessFile` 的配置，是在赌那些类在旧 log4j 里也存在。
//!
//! 控制台**不加** ANSI 颜色（`%highlight`/`%style`）：查看器按解析出来的 `[%level]` 上色，
//! 那是确定的；而 ANSI 会混进 latest.log，规则库就得先剥码再匹配。
//! 有些 mod 自己会吐 ANSI，所以查看器仍要能容忍并剥离——但那是防御，不是我们的输出。

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::version::VersionJson;

/// 版本目录里的文件名。固定名字而不是沿用 `logging.client.file.id`
/// （那个是 `client-1.21.2.xml` 这种带版本号的）：内容是我们生成的，
/// 与 Mojang 那份的 sha1 无关，沿用它的名字会让人以为这是官方文件。
pub const FILENAME: &str = "compass-log4j2.xml";

/// 控制台与文件用**同一个** pattern，这样两处的行形状一致，一个解析器够用。
const PATTERN: &str = "[%d{HH:mm:ss}] [%t/%level]: %msg{nolookups}%n";

pub fn client_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Configuration status="WARN">
    <Appenders>
        <Console name="SysOut" target="SYSTEM_OUT">
            <PatternLayout pattern="{PATTERN}" />
        </Console>
        <RollingRandomAccessFile name="File" fileName="logs/latest.log" filePattern="logs/%d{{yyyy-MM-dd}}-%i.log.gz">
            <PatternLayout pattern="{PATTERN}" />
            <Policies>
                <TimeBasedTriggeringPolicy />
                <OnStartupTriggeringPolicy />
            </Policies>
        </RollingRandomAccessFile>
        <Listener name="Tracy">
            <PatternLayout pattern="(%F:%L): %msg{{nolookups}}%n" />
        </Listener>
    </Appenders>
    <Loggers>
        <Root level="info">
            <filters>
                <MarkerFilter marker="NETWORK_PACKETS" onMatch="DENY" onMismatch="NEUTRAL" />
            </filters>
            <AppenderRef ref="SysOut"/>
            <AppenderRef ref="File"/>
            <AppenderRef ref="Tracy"/>
        </Root>
    </Loggers>
</Configuration>
"#
    )
}

/// 该版本是否需要外部日志配置：只有 Mojang 声明了 `logging.client` 的才需要。
pub fn wanted(version: &VersionJson) -> bool {
    version.logging.as_ref().is_some_and(|l| l.client.is_some())
}

/// 把配置写进版本目录（幂等：内容固定，重写没有副作用）。
/// 返回写入的路径；该版本不需要时返回 `None` 且不写文件。
pub fn write(version_dir: &Path, version: &VersionJson) -> Result<Option<PathBuf>, Error> {
    if !wanted(version) {
        return Ok(None);
    }
    fs::create_dir_all(version_dir)?;
    let dest = version_dir.join(FILENAME);
    fs::write(&dest, client_xml())?;
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 这些断言盯的是"我们改的只有控制台布局"。
    /// 每一项都对应一个静默失效的功能，而症状都在很远的地方：
    /// File appender 没了 → 归档列表永远是空的；标记过滤没了 → 日志被刷爆；
    /// nolookups 没了 → Log4Shell 缓解失效（而且不会有任何可见症状）。
    #[test]
    fn only_the_console_layout_differs_from_mojangs_config() {
        let xml = client_xml();
        assert!(
            !xml.contains("LegacyXMLLayout"),
            "控制台必须不是 XML 布局，否则 stdout 又是 CDATA：{xml}"
        );
        assert!(
            xml.contains(r#"fileName="logs/latest.log""#),
            "游戏仍须把日志落到 logs/latest.log（归档功能读的就是它）：{xml}"
        );
        assert!(
            xml.contains("logs/%d{yyyy-MM-dd}-%i.log.gz"),
            "滚动策略要保留，否则 latest.log 会无限增长：{xml}"
        );
        assert!(
            xml.contains(r#"marker="NETWORK_PACKETS""#) && xml.contains(r#"onMatch="DENY""#),
            "数据包标记仍要被丢弃：{xml}"
        );
        assert!(
            xml.contains("Tracy"),
            "游戏自带的 profiler 钩子不该被我们删掉：{xml}"
        );
        assert_eq!(
            xml.matches("%msg{nolookups}%").count(),
            3,
            "三个 appender 都要带 Log4Shell 缓解：{xml}"
        );
        // 控制台与文件必须同形状，否则"一个解析器"这个前提就不成立
        assert_eq!(
            xml.matches(PATTERN).count(),
            2,
            "控制台与文件应当用同一个 pattern：{xml}"
        );
    }

    /// 只对声明了 `logging.client` 的版本写：给 1.12.2 塞一份带 Tracy 与
    /// RollingRandomAccessFile 的配置，是在赌旧 log4j 里有这些类。
    #[test]
    fn versions_without_a_logging_section_get_no_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let legacy: VersionJson = serde_json::from_str(r#"{"id":"1.12.2"}"#).unwrap();
        assert!(!wanted(&legacy));
        assert!(write(dir.path(), &legacy).unwrap().is_none());
        assert!(
            !dir.path().join(FILENAME).exists(),
            "不该为没有 logging 段的版本写配置文件"
        );

        let modern: VersionJson = serde_json::from_str(
            r#"{"id":"1.21.4","logging":{"client":{
                "argument":"-Dlog4j.configurationFile=${path}",
                "type":"log4j2-xml",
                "file":{"id":"client-1.21.2.xml","sha1":"x","size":1,"url":"https://e/x"}
            }}}"#,
        )
        .unwrap();
        assert!(wanted(&modern));
        let path = write(dir.path(), &modern).unwrap().expect("应当写入");
        assert_eq!(path.file_name().unwrap().to_string_lossy(), FILENAME);
        assert!(path.is_file());
        // 幂等：再写一次内容不变（安装是可重跑的，不能第二次就出错或写坏）
        write(dir.path(), &modern).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), client_xml());
    }

    /// 文件名不沿用 Mojang 的 `logging.client.file.id`：内容是我们生成的，
    /// 与官方那份的 sha1 无关，沿用它的名字会让人以为那是官方文件（也会让人
    /// 以为它可以被 sha1 校验）。
    #[test]
    fn the_file_is_named_after_us_not_after_mojang() {
        assert_eq!(FILENAME, "compass-log4j2.xml");
        assert_ne!(FILENAME, "client-1.21.2.xml");
    }
}
