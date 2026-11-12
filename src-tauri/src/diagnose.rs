//! 把游戏输出翻译成"发生了什么 + 该怎么办"。
//!
//! **每一条规则都对着 `tests/fixtures/crash/` 里的真实日志写**，那些日志是
//! `tests/crash_live.rs` 真跑出来的（真装版本、真放 mod、真启动 JVM）。
//! 规则库最容易坏的方式是照着印象编关键字——编出来的规则会稳定地匹配不到任何东西，
//! 而如果测试也是照着规则写的，两边一起错、一起绿。所以这里的测试是
//! "拿夹具喂进去，断言出哪条码"，夹具是地面真值，不是实现生成的。
//!
//! 实测中推翻掉的几个直觉：
//!
//! - **现代 Forge/NeoForge 对"混装 loader"与"mod 版本不符"根本不崩溃**：NeoForge 打一条
//!   `is a Fabric mod and cannot be loaded` 的 WARN 就跳过，Forge 打一条
//!   `Failed to parse <jar> module-info` 的 ERROR 也跳过，随后正常进游戏。
//!   用户的症状是"我的 mod 没生效"，界面上什么都没有——这类反而最需要诊断，
//!   所以规则库里有 `Warning` 这一档，不只找致命错误。
//! - **Fabric 的 mod 解析失败不退出**：它弹一个 GUI 错误框然后挂住（实测 241 秒后被测试
//!   杀掉，exit 是 `None`，也没有 crash-report）。所以"看退出码"这条路在这里根本不通，
//!   必须从日志流里认失败。
//! - **`Error: A JNI error has occurred, please check your installation and try again`
//!   同时出现在 OOM 和 Java 版本不对两种情况里**，还叫用户去"检查安装"。
//!   照它匹配必然给错建议，所以规则一律匹配它后面那行**具体的**异常。
//! - **六个场景没有一个写出 `crash-reports/`**：常见失败都发生在崩溃报告器之外
//!   （JVM 启动阶段、loader 的 mod 解析阶段）。所以本模块的主输入是 stdout 与
//!   `logs/latest.log`，crash-report 只是其中一种可能的输入。
//! - **`Failed to fetch user properties` + 401 在用第三方皮肤站时是预期的**：游戏在跟
//!   Mojang 的服务说话，而它不认我们镜像站的令牌，游戏照常继续跑。标成致命是错的。
//!
//! 计划里列的九类中，**显卡驱动/LWJGL 初始化失败、磁盘空间不足、杀毒软件锁文件这三类
//! 没有夹具**：前一类试过（删掉解压出来的 natives 并**不会**让 1.21.4 崩溃，现代 LWJGL
//! 从 classpath 的 jar 里自行解压本地库），后两类无法确定性复现。这三类**故意不写规则**
//! ——没有真样本的匹配串是猜的，猜错的规则比没有规则更糟（它会在别的情况下误报，
//! 而用户会照着它去修一个不存在的问题）。

use serde::Serialize;

/// 严重程度。`Warning` 不是"轻微"，而是"游戏还在跑，但有一件事没按你以为的方式发生"。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Fatal,
    Warning,
}

/// 一键动作。前端按 `kind` 决定点了去哪儿，**不解析文案**。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    /// 去 Java 管理页；`major` 是这个游戏版本需要的主版本号
    InstallJava { major: u32 },
    /// 去内容页搜这个 mod id（缺依赖时用）
    InstallMod { id: String },
    /// 去实例设置调内存
    AdjustMemory,
    /// 去内容页管理 mod（版本冲突、被跳过的 jar）
    OpenContent,
    /// 打开完整日志
    OpenLogs,
}

/// 一条诊断结果。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// 稳定码，前端据此本地化（与 `error.rs` 的码同一套思路）
    pub code: &'static str,
    pub severity: Severity,
    /// 命中的原文行。给用户看"我们是从哪句判断的"——一句能被核对的证据，
    /// 比一段我们自己的转述更可信，也让我们在规则错的时候容易被发现。
    pub evidence: String,
    pub action: Option<Action>,
}

/// 扫描一段游戏输出（stdout、`logs/latest.log` 或 `crash-reports/*.txt` 都行，
/// 三者都是纯文本；见 `logconfig` 模块头为什么统一成纯文本）。
///
/// 返回所有命中的规则，致命的排在前面。可能同时命中多条：一次失败常常既有
/// 直接原因（缺某个 mod）也有旁证（另一个 jar 被跳过）。
pub fn diagnose(text: &str) -> Vec<Finding> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Finding> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        if let Some(f) = java_version(line) {
            out.push(f);
        }
        if let Some(f) = heap(line) {
            out.push(f);
        }
        if let Some(f) = out_of_memory(line) {
            out.push(f);
        }
        if let Some(f) = fabric_resolution(&lines, i, line) {
            out.extend(f);
        }
        if let Some(f) = loader_mismatch(line) {
            out.push(f);
        }
        if let Some(f) = forge_skipped_jar(line) {
            out.push(f);
        }
        if let Some(f) = mojang_user_properties(&lines, i, line) {
            out.push(f);
        }
    }

    dedup(&mut out);
    out.sort_by_key(|f| match f.severity {
        Severity::Fatal => 0,
        Severity::Warning => 1,
    });
    out
}

/// 同一条码只留第一条：一份日志里同一个原因会出现很多次
/// （堆栈的每一行都可能重复关键类名），列七遍不会更有用。
fn dedup(findings: &mut Vec<Finding>) {
    let mut seen: Vec<&str> = Vec::new();
    findings.retain(|f| {
        if seen.contains(&f.code) {
            return false;
        }
        seen.push(f.code);
        true
    });
}

// ---------------------------------------------------------------------------
// 规则。每条都注明它对着哪个夹具。
// ---------------------------------------------------------------------------

/// Java 版本不对。夹具：`wrong-java.log`（JDK 8 跑 1.21.4）。
///
/// 原文里带着两个类文件版本号，正好能算出"需要几"与"手上有几"——
/// 这比一句"Java 版本不对"有用得多，因为用户通常装了好几个 JDK。
fn java_version(line: &str) -> Option<Finding> {
    if !line.contains("UnsupportedClassVersionError") {
        return None;
    }
    let needed = after_number(line, "class file version ")?;
    Some(Finding {
        code: "java_version_mismatch",
        severity: Severity::Fatal,
        evidence: line.trim().to_string(),
        action: Some(Action::InstallJava {
            major: java_major_of_class_file(needed),
        }),
    })
}

/// class file 版本号 → Java 主版本号（52→8、61→17、65→21）。
/// 45 以下是 1.1 之前的古董，用饱和减法而不是断言：日志里的数字不该让启动器 panic。
pub fn java_major_of_class_file(class_file: u32) -> u32 {
    class_file.saturating_sub(44)
}

/// 取 `needle` 之后紧跟的那个整数。
fn after_number(line: &str, needle: &str) -> Option<u32> {
    let rest = line.split_once(needle)?.1;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// 堆配置非法。夹具：无（我们把 `-Xms` 夹住之后这条不该再出现），但用户自己填的
/// 额外 JVM 参数可以造出来，而它的报错完全看不出是参数问题。
fn heap(line: &str) -> Option<Finding> {
    if !line.contains("Initial heap size set to a larger value than the maximum heap size") {
        return None;
    }
    Some(Finding {
        code: "heap_misconfigured",
        severity: Severity::Fatal,
        evidence: line.trim().to_string(),
        action: Some(Action::AdjustMemory),
    })
}

/// 内存不足。夹具：`out-of-memory.log`。
///
/// **不要**匹配它上面那行 `Error: A JNI error has occurred, please check your
/// installation and try again`——那句在 Java 版本不对时也会出现，且建议是错的。
fn out_of_memory(line: &str) -> Option<Finding> {
    if !line.contains("OutOfMemoryError") {
        return None;
    }
    Some(Finding {
        code: "out_of_memory",
        severity: Severity::Fatal,
        evidence: line.trim().to_string(),
        action: Some(Action::AdjustMemory),
    })
}

/// Fabric 的 mod 解析失败。夹具：`fabric-missing-dependency.log`、`fabric-version-mismatch.log`。
///
/// Fabric 把机器可读的原因放在紧跟 `Mod resolution failed` 的几行里，而**区分成因要看
/// `Immediate reason:` 那一行**：
/// - 缺 mod：`Immediate reason: [HARD_DEP_NO_CANDIDATE <mod> <ver> {depends <id> @ [*]}, …]`
/// - 版本不符：`Immediate reason: [HARD_DEP_INCOMPATIBLE_PRESELECTED <mod> <ver> {depends minecraft @ [1.20.1]}, …]`
///
/// 两者的 `Reason:` 行都只写 `HARD_DEP`，照它判会把"缺 mod"误报成"版本冲突"
/// ——这个错第一版就犯了，是夹具把它抓出来的。
///
/// 取这几行而不是取后面的本地化文案：文案会随语言变（实测这台机器上是繁体中文），
/// 而 `Immediate reason:` / `Reason:` 是稳定的 ASCII。
fn fabric_resolution(lines: &[&str], i: usize, line: &str) -> Option<Vec<Finding>> {
    if !line.contains("Mod resolution failed") {
        return None;
    }
    let window = &lines[i..lines.len().min(i + 6)];
    let find = |needle: &str| window.iter().find(|l| l.contains(needle)).copied();
    let immediate = find("Immediate reason: [");
    let reason = find("Reason: [").unwrap_or(line);
    // 证据优先给 Immediate reason（它才说得清成因），没有就退回 Reason
    let evidence = immediate.unwrap_or(reason);

    if evidence.contains("HARD_DEP_NO_CANDIDATE") {
        // 缺 mod：两行的 `{depends <id> @ …}` 是同一段，任一行取到就行
        let id = fabric_dep_id(evidence).or_else(|| fabric_dep_id(reason));
        return Some(vec![Finding {
            code: "fabric_missing_mod",
            severity: Severity::Fatal,
            evidence: evidence.trim().to_string(),
            // 取不到 id 时退化成"去看日志"：给一个搜不了的动作比不给动作更糟
            action: Some(id.map_or(Action::OpenLogs, |id| Action::InstallMod { id })),
        }]);
    }
    if evidence.contains("HARD_DEP") {
        return Some(vec![Finding {
            code: "fabric_mod_version_conflict",
            severity: Severity::Fatal,
            evidence: evidence.trim().to_string(),
            action: Some(Action::OpenContent),
        }]);
    }
    // 认不出具体原因，但解析确实失败了：至少别让用户以为游戏是"随便崩的"
    Some(vec![Finding {
        code: "fabric_mod_resolution_failed",
        severity: Severity::Fatal,
        evidence: evidence.trim().to_string(),
        action: Some(Action::OpenLogs),
    }])
}

/// 从 `… {depends <id> @ […]}` 里取出被依赖的 mod id。
/// 不用正则：这段形状固定，手写切片反而更容易看出它在解析什么。
fn fabric_dep_id(line: &str) -> Option<String> {
    let depends = line.split_once("{depends ")?.1;
    let id: String = depends
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '}')
        .collect();
    (!id.is_empty()).then_some(id)
}

/// 混装 loader：别的 loader 的 mod 被跳过。夹具：`mixed-loader.log`（NeoForge 21.4.161）。
///
/// 原文：`Skipping jar. File mods\sodium-fabric-0.6.13+mc1.21.4.jar is a Fabric mod and cannot be loaded`
fn loader_mismatch(line: &str) -> Option<Finding> {
    if !line.contains("cannot be loaded") || !line.contains("Skipping jar") {
        return None;
    }
    Some(Finding {
        code: "loader_mismatch_skipped",
        severity: Severity::Warning,
        evidence: line.trim().to_string(),
        action: Some(Action::OpenContent),
    })
}

/// Forge 跳过一个读不了的 mod jar。夹具：`forge-skips-broken-mod.log`（1.20.1 的 JEI 进 1.21.4）。
///
/// 原文：`Failed to parse <path>\jei-1.20.1-forge-15.74.0.230.jar module-info`
/// 紧跟 `java.lang.module.InvalidModuleDescriptorException`。真实成因是版本不符，
/// 但 Forge 报的是"module-info 读不了"——所以码名按**它说了什么**取，不按我们猜的成因取。
fn forge_skipped_jar(line: &str) -> Option<Finding> {
    if !line.contains("Failed to parse") || !line.contains("module-info") {
        return None;
    }
    Some(Finding {
        code: "mod_jar_skipped",
        severity: Severity::Warning,
        evidence: line.trim().to_string(),
        action: Some(Action::OpenContent),
    })
}

/// 游戏向 Mojang 取用户属性失败（401）。夹具：`mojang-user-properties-401.log`。
///
/// **用第三方皮肤站时这是预期的**：令牌是我们镜像站发的，Mojang 不认，而游戏照常继续跑。
/// 所以是 Warning，而且不给动作——给一个"去重新登录"的按钮会把用户引到错的地方。
fn mojang_user_properties(lines: &[&str], i: usize, line: &str) -> Option<Finding> {
    if !line.contains("Failed to fetch user properties") {
        return None;
    }
    let rest = &lines[i..lines.len().min(i + 3)];
    if !rest.iter().any(|l| l.contains("Status: 401")) {
        return None;
    }
    Some(Finding {
        code: "mojang_user_properties_401",
        severity: Severity::Warning,
        evidence: line.trim().to_string(),
        action: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 夹具是 `tests/crash_live.rs` 真跑出来的日志，原样存进仓库。
    fn fixture(name: &str) -> String {
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/crash/").to_string() + name;
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("夹具 {p} 读不到：{e}"))
    }

    fn codes(text: &str) -> Vec<&'static str> {
        diagnose(text).iter().map(|f| f.code).collect()
    }

    /// 每条规则都要在**真实日志**上命中，并且命中在预期的那一档。
    /// 这不是"实现等于我对它的理解"那种测试：夹具是先于规则存在的地面真值，
    /// 规则写错了这里就红。
    #[test]
    fn every_rule_fires_on_its_real_capture() {
        assert_eq!(
            codes(&fixture("wrong-java.log")),
            ["java_version_mismatch"],
            "Java 版本不对应当被认出来"
        );
        assert_eq!(codes(&fixture("out-of-memory.log")), ["out_of_memory"]);
        assert_eq!(
            codes(&fixture("fabric-missing-dependency.log")),
            ["fabric_missing_mod"]
        );
        assert_eq!(
            codes(&fixture("fabric-version-mismatch.log")),
            ["fabric_mod_version_conflict"]
        );
        assert_eq!(
            codes(&fixture("mixed-loader.log")),
            ["loader_mismatch_skipped"]
        );
        assert_eq!(
            codes(&fixture("forge-skips-broken-mod.log")),
            ["mod_jar_skipped"]
        );
        assert_eq!(
            codes(&fixture("mojang-user-properties-401.log")),
            ["mojang_user_properties_401"]
        );
    }

    /// 严重程度分档：被跳过的 mod 与 401 都是 Warning（游戏还在跑），
    /// 启动不起来的那些是 Fatal。分错了 UI 就会用错的语气说话。
    #[test]
    fn severity_separates_still_running_from_not_starting() {
        for f in [
            ("mixed-loader.log", Severity::Warning),
            ("forge-skips-broken-mod.log", Severity::Warning),
            ("mojang-user-properties-401.log", Severity::Warning),
            ("wrong-java.log", Severity::Fatal),
            ("out-of-memory.log", Severity::Fatal),
            ("fabric-missing-dependency.log", Severity::Fatal),
        ] {
            let got = diagnose(&fixture(f.0));
            assert_eq!(got.len(), 1, "{} 应当只命中一条：{got:?}", f.0);
            assert_eq!(got[0].severity, f.1, "{}", f.0);
        }
    }

    /// Java 主版本号要算对，否则"去装 Java X"会把用户指到另一个错版本上。
    /// 52→8、61→17、65→21 是三个真实锚点（1.12.2 要 8，1.18–1.20.4 要 17，1.20.5+ 要 21）。
    #[test]
    fn class_file_version_maps_to_the_java_major() {
        assert_eq!(java_major_of_class_file(52), 8);
        assert_eq!(java_major_of_class_file(61), 17);
        assert_eq!(java_major_of_class_file(65), 21);
        // 日志里出现异常数字时也不该 panic
        assert_eq!(java_major_of_class_file(0), 0);
    }

    /// 真实那句报错里带着 `class file version 65.0`，动作应当是"装 Java 21"。
    #[test]
    fn the_java_action_carries_the_version_the_game_actually_needs() {
        let got = diagnose(&fixture("wrong-java.log"));
        assert_eq!(
            got[0].action,
            Some(Action::InstallJava { major: 21 }),
            "{got:?}"
        );
    }

    /// 缺依赖那条要能报出**缺的是谁**——只说"缺依赖"等于没说，
    /// 用户还是得自己去日志里翻。
    #[test]
    fn the_missing_dependency_action_names_the_mod() {
        let got = diagnose(&fixture("fabric-missing-dependency.log"));
        assert_eq!(
            got[0].action,
            Some(Action::InstallMod {
                id: "some-mod-that-is-not-installed".into()
            }),
            "{got:?}"
        );
    }

    /// `Error: A JNI error has occurred, please check your installation` 在 OOM 与
    /// Java 版本不对两种情况下**都会出现**，而且它的建议（检查安装）是错的。
    /// 规则库不能有自己的码由它触发。
    #[test]
    fn the_jni_preamble_does_not_produce_a_finding_of_its_own() {
        for name in ["out-of-memory.log", "wrong-java.log"] {
            let text = fixture(name);
            assert!(
                text.contains("A JNI error has occurred"),
                "{name} 里应当有那句 JNI 前导，否则这个测试没在测东西"
            );
            assert!(
                !codes(&text).contains(&"jni_error"),
                "{name}: {name} 不该由 JNI 前导句触发诊断"
            );
            assert_eq!(
                diagnose(&text).len(),
                1,
                "{name} 应当只有一条诊断（JNI 前导句不算）"
            );
        }
    }

    /// 证据必须是原文行：用户要能核对我们的判断，规则错了也要能被发现。
    #[test]
    fn evidence_is_a_verbatim_line_from_the_log() {
        for name in [
            "wrong-java.log",
            "out-of-memory.log",
            "fabric-missing-dependency.log",
            "mixed-loader.log",
        ] {
            let text = fixture(name);
            for f in diagnose(&text) {
                assert!(
                    text.lines().any(|l| l.trim() == f.evidence),
                    "{name} 的 {:?} 证据不是原文里的任何一行：{:?}",
                    f.code,
                    f.evidence
                );
            }
        }
    }

    /// 干净的日志不该报出任何东西。**误报比漏报更糟**：用户会照着去修一个
    /// 不存在的问题，而且从此不再相信这个面板。
    #[test]
    fn a_healthy_log_produces_nothing() {
        let healthy = "\
[20:22:38] [main/INFO]: Loading Minecraft 1.21.4 with Fabric Loader 0.20.1
[20:22:38] [main/INFO]: Loading 11 mods:
[20:22:39] [main/INFO]: Found graphics adapter: AdapterInfo{vendor=NVIDIA}
[20:22:40] [Datafixer Bootstrap/INFO]: 243 Datafixer optimizations took 389 milliseconds
[20:22:47] [Render thread/INFO]: Setting user: Tester
[20:22:47] [Render thread/INFO]: Backend library: LWJGL version 3.3.3+5
";
        assert_eq!(diagnose(healthy), vec![]);
        assert_eq!(diagnose(""), vec![]);
    }

    /// 同一个原因在一份日志里出现多次时只报一条：堆栈会把关键类名重复很多遍，
    /// 列七遍不会更有用，只会把真正不同的另一条挤到视野外。
    #[test]
    fn a_repeated_cause_is_reported_once() {
        let text = "java.lang.OutOfMemoryError: Java heap space\n\
                    at foo\njava.lang.OutOfMemoryError: Java heap space\n\
                    Caused by: java.lang.OutOfMemoryError: Java heap space\n";
        let got = diagnose(text);
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].code, "out_of_memory");
    }

    /// 致命排在警告前面：两者同时命中时（一个 mod 被跳过 + 另一个导致起不来），
    /// 用户要先看到的是"起不来"的原因。
    #[test]
    fn fatal_findings_come_before_warnings() {
        let text = "\
[20:06:14] [main/WARN] [loading.moddiscovery.ModDiscoverer/SCAN]: Skipping jar. File mods\\sodium-fabric.jar is a Fabric mod and cannot be loaded
Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space
";
        let got = diagnose(text);
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(got[0].severity, Severity::Fatal);
        assert_eq!(got[1].severity, Severity::Warning);
    }
}
