//! 人为制造真实崩溃，把输出与崩溃报告抓下来——崩溃规则库的**地面真值**。
//!
//! 规则库最容易写错的方式是照着印象编关键字：`ClassNotFoundException` 这种谁都想得到，
//! 但"缺 mod"在 Fabric 与 NeoForge 上是两段完全不同的文本，"内存不足"有时是
//! `OutOfMemoryError` 有时只是进程被杀、连一行日志都没有。编出来的规则会稳定地
//! 匹配不到任何东西，而测试如果也是照着规则写的，两边一起错、一起绿。
//!
//! 所以顺序是：**先造真崩溃、抓真文本，再照着文本写规则**。本文件负责前半段，
//! 断言规则命中的部分在 `diagnose` 模块的测试里（用这里抓下来的文本做夹具）。
//!
//! 五个场景都是能确定性复现的，不依赖显卡或运气：
//! 1. 混装 loader：Fabric 的 sodium 放进 NeoForge 实例
//! 2. mod 版本不匹配：1.20.1 的 JEI 放进 1.21.4 实例
//! 3. Java 版本不对：用 JDK 8 跑 1.21.4
//! 4. 内存不足：`-Xmx64m`
//! 5. 缺依赖：sodium 需要 fabric-api 提供的一堆 `fabric-*-api-v*`，实例里没有 Fabric API

mod live;

use std::path::{Path, PathBuf};
use std::time::Duration;

use compass_lib::instance::Instance;
use compass_lib::paths::Paths;
use compass_lib::runner::LaunchInput;
use compass_lib::version::OsInfo;

/// 真实 mod 样本目录（`COMPASS_CRASH_MODS` 指定）。里面要有：
/// `sodium-fabric-0.6.13+mc1.21.4.jar`、`jei-1.20.1-forge-15.74.0.230.jar`
const MODS: &str = "COMPASS_CRASH_MODS";

/// 一个 JDK 8（`COMPASS_CRASH_JAVA8`），用来造"Java 版本不对"
const JAVA8: &str = "COMPASS_CRASH_JAVA8";

struct Scenario {
    name: &'static str,
    version_id: String,
    /// 从 MODS 目录复制进去的真实 jar
    mods: Vec<String>,
    /// 现场生成的最小 Fabric mod（只含 `fabric.mod.json`）。
    ///
    /// 用合成 jar 不是造假：**措辞是真正的 Fabric Loader 生成的**，与 jar 里有没有代码
    /// 无关。之所以需要它，是因为手上正好没有一个"真的缺依赖"的 mod——sodium 把
    /// `fabric-api-base`、`fabric-block-view-api-v2` 等全都嵌进了自己的 jar（实测如此），
    /// 于是它单独放着也能跑起来。
    synthetic: Option<SyntheticMod>,
    memory_mb: Option<u32>,
    /// 覆盖启动用的 java（默认走 live::java_path()）
    java: Option<PathBuf>,
    /// 这个场景**期望**游戏正常跑起来（于是命中启动标记就停，不干等超时）。
    ///
    /// 实测现代 Forge/NeoForge 对"混装 loader"与"mod 版本不匹配"都**不崩溃**：
    /// NeoForge 打一条 `is a Fabric mod and cannot be loaded` 的 WARN 就跳过，
    /// Forge 打一条 `Failed to parse <jar> module-info` 的 ERROR 也跳过，两者随后
    /// 正常进游戏。所以用户的症状是"我的 mod 没生效"而不是"游戏崩了"——
    /// 这类反而更需要诊断，因为界面上什么都没有。
    expect_boot: bool,
    /// 这个场景**必须**在输出里留下的诊断线索。规则库要匹配的就是它；
    /// 断言它存在，等于把地面真值钉住——将来某个 loader 改了措辞，这里会红。
    must_contain: &'static str,
}

struct SyntheticMod {
    file: &'static str,
    id: &'static str,
    /// `fabric.mod.json` 的 `depends` 内容（不含外层花括号）
    depends: &'static str,
}

/// "游戏已经跑起来了"的标记。两个都取：不同 loader 走到这里的最后一步不同。
const BOOTED: [&str; 2] = ["Backend library: LWJGL", "Setting user:"];

/// 失败标记。**必须靠它们停下，不能等退出码**：实测 Fabric 的 mod 解析失败会弹一个
/// GUI 错误框然后**挂住不退**（241 秒后被测试杀掉，exit 是 None，也没有 crash-report）——
/// 也就是说"退出码 1"这种失败形态在最常见的场景里根本不存在，只看进程退不退出
/// 就会一直等到超时。
const FAILED: [&str; 8] = [
    "Mod resolution failed",
    "Incompatible mods found!",
    "Failed to start the minecraft client",
    "---- Minecraft Crash Report ----",
    "Crash report saved",
    "UnsupportedClassVersionError",
    "OutOfMemoryError",
    "Error occurred during initialization",
];

#[tokio::test]
#[ignore = "needs network, a full 1.21.4 install, real mod jars and Java 8 (see MODS/JAVA8 env vars)"]
async fn manufactures_real_crashes_and_captures_them() {
    let data = live::data_dir("COMPASS_CRASH_DIR");
    let paths = Paths::new(data.path().to_path_buf());
    paths.ensure().unwrap();
    let out_dir = data.path().join("crashes");
    std::fs::create_dir_all(&out_dir).unwrap();

    let mods_dir = PathBuf::from(
        std::env::var(MODS).unwrap_or_else(|_| "D:/Temp/opencode/realmods".to_string()),
    );
    assert!(
        mods_dir.is_dir(),
        "{MODS} 要指向放着真实 mod jar 的目录，现在是 {mods_dir:?}"
    );
    let java8 = std::env::var(JAVA8).ok().map(PathBuf::from);

    // "缺依赖"那个场景要一个 Fabric 实例，而数据目录里未必有：没有就装一个。
    // loader 生成的 id 是 `fabric-loader-<ver>-<game>` 这种形状，不能写死。
    let fabric_id = {
        let mut installed: Vec<String> = std::fs::read_dir(paths.versions())
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .filter(|id| {
                        paths
                            .versions()
                            .join(id)
                            .join(format!("{id}.json"))
                            .is_file()
                    })
                    .collect()
            })
            .unwrap_or_default();
        installed.sort();
        match installed
            .into_iter()
            .find(|id| id.starts_with("fabric-loader-"))
        {
            Some(id) => id,
            None => {
                let http = live::http();
                let api = live::api(&http);
                let installer = live::installer(&paths, &http);
                let versions = compass_lib::loader::versions(
                    &api,
                    compass_lib::loader::LoaderKind::Fabric,
                    "1.21.4",
                )
                .await
                .unwrap();
                let installed = compass_lib::loader::install(
                    &installer,
                    &api,
                    compass_lib::loader::LoaderKind::Fabric,
                    "1.21.4",
                    &versions[0].version,
                    None,
                )
                .await
                .unwrap();
                installed.id
            }
        }
    };
    println!("fabric version for the missing-dependency scenario: {fabric_id}");

    let scenarios = vec![
        // 1. 混装 loader：NeoForge 只 WARN 然后跳过，游戏照常起来
        Scenario {
            name: "mixed-loader",
            version_id: "neoforge-21.4.161".into(),
            mods: vec!["sodium-fabric-0.6.13+mc1.21.4.jar".into()],
            synthetic: None,
            memory_mb: Some(2048),
            java: None,
            expect_boot: true,
            must_contain: "is a Fabric mod and cannot be loaded",
        },
        // 2. Forge 对版本不符的 mod：ERROR + 跳过，游戏照常起来
        Scenario {
            name: "forge-skips-broken-mod",
            version_id: "1.21.4-forge-54.1.22".into(),
            mods: vec!["jei-1.20.1-forge-15.74.0.230.jar".into()],
            synthetic: None,
            memory_mb: Some(2048),
            java: None,
            expect_boot: true,
            must_contain: "Failed to parse",
        },
        // 3. Fabric 对版本不符的 mod：这个会真崩（与 Forge 的行为相反）
        Scenario {
            name: "fabric-version-mismatch",
            version_id: fabric_id.clone(),
            mods: vec!["lithium-fabric-mc1.20.1-0.11.4.jar".into()],
            synthetic: None,
            memory_mb: Some(2048),
            java: None,
            expect_boot: false,
            must_contain: "Mod resolution failed",
        },
        // 4. 缺依赖：合成一个声明了不存在的 mod 的最小 Fabric mod
        Scenario {
            name: "fabric-missing-dependency",
            version_id: fabric_id.clone(),
            mods: vec![],
            synthetic: Some(SyntheticMod {
                file: "compasstest-missing-dep.jar",
                id: "compasstest_missing_dep",
                depends: r#""fabricloader":">=0.16.0","minecraft":"~1.21.4","some-mod-that-is-not-installed":"*""#,
            }),
            memory_mb: Some(2048),
            java: None,
            expect_boot: false,
            must_contain: "",
        },
        // 5. 内存不足
        Scenario {
            name: "out-of-memory",
            version_id: "1.21.4".into(),
            mods: vec![],
            synthetic: None,
            memory_mb: Some(64),
            java: None,
            expect_boot: false,
            must_contain: "",
        },
        // 6. Java 版本不对：1.21.4 要 Java 21，用 JDK 8 跑
        Scenario {
            name: "wrong-java",
            version_id: "1.21.4".into(),
            mods: vec![],
            synthetic: None,
            memory_mb: Some(2048),
            java: java8.clone(),
            expect_boot: false,
            must_contain: "",
        },
    ];

    for s in &scenarios {
        let java = s.java.clone().unwrap_or_else(live::java_path);
        if s.java.is_some() && java8.is_none() {
            println!("skip {}: {JAVA8} 没给", s.name);
            continue;
        }
        if s.java.is_some() && !java.is_file() {
            println!("skip {}: java 不存在 {}", s.name, java.display());
            continue;
        }

        // 每个场景一个独立实例：mods/ 与 game dir 都按 slug 隔离，
        // 否则上一个场景留下的 jar 会污染下一个
        let slug = format!("crash-{}", s.name);
        let inst = Instance {
            slug: slug.clone(),
            name: format!("crash {s}", s = s.name),
            version_id: s.version_id.clone(),
            memory_mb: s.memory_mb,
            ..Default::default()
        };
        compass_lib::instance::save(&paths, &inst).unwrap();

        let game = compass_lib::instance::game_dir(&paths, &slug).unwrap();
        let mods_target = game.join("mods");
        // 清空再放：幂等地重跑这个测试时不该叠加上一次的 jar
        std::fs::remove_dir_all(&mods_target).ok();
        std::fs::create_dir_all(&mods_target).unwrap();
        for m in &s.mods {
            let src = mods_dir.join(m);
            assert!(src.is_file(), "样本 mod 不在：{}", src.display());
            std::fs::copy(&src, mods_target.join(m)).unwrap();
        }
        if let Some(syn) = &s.synthetic {
            write_synthetic_mod(&mods_target, syn);
        }
        // 崩溃报告也要清掉，否则读到的是上一次的
        std::fs::remove_dir_all(game.join("crash-reports")).ok();
        std::fs::remove_file(game.join("logs").join("latest.log")).ok();

        let version = compass_lib::runner::read_installed_version(&paths, &s.version_id)
            .unwrap_or_else(|e| panic!("版本 {} 没装好：{e}", s.version_id));
        let account = live::offline_account();
        let os = OsInfo::current();
        let input = LaunchInput {
            instance: &inst,
            account: &account,
            version: &version,
            os: &os,
            paths: &paths,
            default_memory_mb: 2048,
            api_url: "https://mc.example.com",
            java,
            authlib_jar: None,
            prefetched_meta: None,
        };
        let plan = compass_lib::runner::build(&input).unwrap();
        let natives = compass_lib::runner::natives_to_extract(&input).unwrap();
        let natives_dir = compass_lib::instance::natives_dir(&paths, &slug).unwrap();
        compass_lib::launch::extract_natives(&natives, &natives_dir).unwrap();

        // 成功与失败标记都当停止条件，之后由 stopped_at 区分是哪一种：
        // 失败的场景里有的会挂住不退（Fabric 弹 GUI 框），等退出码会一直等到超时
        let mut stop: Vec<&str> = BOOTED.to_vec();
        stop.extend(FAILED.iter().copied());
        let out = live::run_and_capture(&plan, Duration::from_secs(240), &stop).await;
        let log_path = out_dir.join(format!("{}.log", s.name));
        out.save(&log_path);

        // 崩溃报告单独存一份：它是纯文本、结构与 stdout 不同，规则库要分开对待
        let report = newest_crash_report(&game);
        if let Some(r) = &report {
            let dest = out_dir.join(format!("{}.crash.txt", s.name));
            std::fs::copy(r, &dest).ok();
        }

        if !s.must_contain.is_empty() {
            assert!(
                out.contains(s.must_contain),
                "{} 的输出里没有 {:?}——loader 改了措辞，或场景没造出预期的情况；末尾：\n{}",
                s.name,
                s.must_contain,
                out.tail(30)
            );
        }
        // 控制台输出必须是纯文本。Mojang 的 `client.xml` 用 `<LegacyXMLLayout />`，
        // 换成我们自己那份配置之前，原版与 Fabric 的 stdout 是
        // `<log4j:Message><![CDATA[…]]></log4j:Message>`——那样查看器与崩溃规则库
        // 就得为"实时输出"与"归档/崩溃报告"各写一套解析。这条是那次替换的回归门。
        assert!(
            !out.contains("log4j:Message") && !out.contains("CDATA"),
            "{} 的控制台输出又变回 XML 了（logconfig 没生效？）；开头：\n{}",
            s.name,
            out.lines.first().map(String::as_str).unwrap_or("")
        );
        println!(
            "{}: {} lines in {:.0}s exit={:?} log={} crash_report={}",
            s.name,
            out.lines.len(),
            out.elapsed.as_secs_f64(),
            out.exit,
            log_path.display(),
            report.is_some()
        );
        // 场景必须按预期结束：抓下来一份与预期不符的日志，规则库就会照着它写错，
        // 而且错法是"匹配不到"——静默的
        let hit = |list: &[&str]| out.stopped_at.as_deref().is_some_and(|m| list.contains(&m));
        if s.expect_boot {
            assert!(
                hit(&BOOTED) && !hit(&FAILED),
                "{} 预期能跑起来，实际 stopped_at={:?} exit={:?}；末尾：\n{}",
                s.name,
                out.stopped_at,
                out.exit,
                out.tail(20)
            );
        } else {
            assert!(
                hit(&FAILED) || out.exit.is_some_and(|c| c != 0),
                "{} 预期失败，实际 stopped_at={:?} exit={:?}；末尾：\n{}",
                s.name,
                out.stopped_at,
                out.exit,
                out.tail(20)
            );
        }
    }

    println!("captured into {}", out_dir.display());
}

/// 造一个只含 `fabric.mod.json` 的最小 Fabric mod。
///
/// 里面没有任何代码，也不需要：目的是让**真的** Fabric Loader 走完它的依赖解析，
/// 然后把它的原文报错抓下来。措辞由 loader 生成，与 jar 的内容无关。
fn write_synthetic_mod(dir: &Path, m: &SyntheticMod) {
    use std::io::Write;
    let json = format!(
        r#"{{
  "schemaVersion": 1,
  "id": "{id}",
  "version": "1.0.0",
  "name": "{id}",
  "environment": "client",
  "depends": {{{depends}}}
}}"#,
        id = m.id,
        depends = m.depends
    );
    let file = std::fs::File::create(dir.join(m.file)).unwrap();
    let mut zw = zip::ZipWriter::new(file);
    zw.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zw.write_all(json.as_bytes()).unwrap();
    zw.finish().unwrap();
}

/// 最新的一份崩溃报告（游戏按 `crash-YYYY-MM-DD_HH.mm.ss.txt` 命名）
fn newest_crash_report(game_dir: &Path) -> Option<PathBuf> {
    let dir = game_dir.join("crash-reports");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in std::fs::read_dir(&dir).ok()?.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "txt")
            && let Ok(m) = e.metadata()
        {
            let t = m.modified().unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(bt, _)| t > *bt) {
                best = Some((t, p));
            }
        }
    }
    best.map(|(_, p)| p)
}
