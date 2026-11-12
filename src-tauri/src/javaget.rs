//! 下载 Mojang 官方的 Java runtime，落到 `cache/java/<component>/`。
//!
//! **产品名到 Java 版本的映射不写死在这里**：游戏版本 JSON 的 `javaVersion.component`
//! 就是产品名，`all.json` 把 (平台, 产品) 映射到 manifest。实测（2026-10）各产品当前是
//! `alpha`=16、`beta`=17、`gamma`=17、`delta`=21、`epsilon`=25、`jre-legacy`=8——
//! 注意 **gamma 是 17 而不是 21**，凭印象写一张映射表就会下错版本，而症状是
//! "装了 Java 但游戏还是说版本不对"。
//!
//! 两个实测约束：
//! - **Linux arm64 没有 Mojang runtime**（`all.json` 的 Linux 只有 `linux` 与 `linux-i386`）。
//!   同样地，mac-os-arm64 与 windows-arm64 上 `java-runtime-alpha`/`beta`/`jre-legacy` 是空的，
//!   也就是 Apple Silicon 上拿不到官方的 Java 8/16/17。这些情况下 `platform_runtime` 返回
//!   `None`，调用方应当明确说"这个平台没有官方运行时，请手动指定 Java"，
//!   而不是让用户盯着一个转不出结果的进度条。
//! - **只取 `downloads.raw`，不取 `downloads.lzma`。** lzma 那份小得多，但它是裸 LZMA1
//!   （不是 xz 容器），要引一个新的解码依赖；而体积大头 `lib/modules`（实测 61MB）
//!   **只有 raw**，所以省下来的比例有限。用一个新依赖加一种失败模式换不到一半的流量，不值。
//!
//! `all.json` 的 URL 里带一个哈希（Mojang 就是这么发布的，HMCL 同样把它写死）。
//! 万一哪天它被轮换，症状是所有平台的 Java 下载都报"取不到运行时清单"——
//! 那时更新这个常量即可；用户自己配的 Java 路径不受影响，所以这不是单点故障。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::api::ApiClient;
use crate::error::Error;
use crate::net::{Downloader, FileTask, ProgressSink, Summary};
use crate::paths::{Paths, safe_join};
use crate::version::{Arch, OsInfo, OsName};

/// 全部平台 × 全部产品的索引。URL 里的哈希是 Mojang 发布这份索引的方式（见模块头）。
pub const ALL_JSON_URL: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// 一次安装的结果。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledJava {
    /// `bin/java`（Windows 上是 `bin\java.exe`）的绝对路径
    pub path: String,
    /// 产品名（`java-runtime-delta` 等），也是 `cache/java/` 下的目录名
    pub component: String,
    /// 索引里写的版本号（如 `21.0.7`）。这是**来源自称**的版本，
    /// 与 `java -version` 探测出来的大版本号是两件事，两者都给出来便于排查。
    pub version_name: String,
    pub download: Summary,
}

// ---------------------------------------------------------------------------
// 线上形状
// ---------------------------------------------------------------------------

/// `all.json`：`{ 平台: { 产品: [ {availability, manifest, version}, … ] } }`。
///
/// 两处形状都是实测出来的，凭印象写会错：
/// - **平台在外层**，产品在内层（反了就一个都找不到）。
/// - **产品对应的是一个数组**，不是单个对象；**空数组 = 这个平台没有该产品**
///   （实测 `gamecore` 下所有产品都是 `[]`，Apple Silicon 上的 `java-runtime-alpha`
///   与 `jre-legacy` 也是）。按对象解析会报
///   `invalid length 0, expected struct Product`，整个索引一个都读不出来。
///
/// 数组里可以有多个条目（不同的 availability group）。我们取**第一个带 manifest 的**：
/// 下载只需要一个可用的清单，选哪个 group 不影响拿到的 JRE 内容。
pub type AllJson = BTreeMap<String, BTreeMap<String, Vec<Product>>>;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Product {
    pub manifest: Option<ManifestRef>,
    #[serde(default)]
    pub version: ProductVersion,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ManifestRef {
    pub url: String,
    pub sha1: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ProductVersion {
    #[serde(default)]
    pub name: String,
}

/// 单个 JRE 的文件清单。
///
/// **外层有一个 `files` 键**，里面才是「相对路径 → 条目」的扁平 map（实测 windows-x64 的
/// java-runtime-delta 有 484 条）。按扁平 map 解析会一个条目都拿不到，然后"装好了"一个
/// 空目录——所以这一层不能省。
///
/// 目录也有条目（`{"type":"directory"}`），要跳过：它们没有下载物。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Manifest {
    pub files: BTreeMap<String, ManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ManifestEntry {
    Directory,
    File {
        /// 为真时要给可执行位（Unix 上的 `bin/java`、`bin/javaw` 等）
        #[serde(default)]
        executable: bool,
        downloads: BTreeMap<String, Artifact>,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Artifact {
    pub url: String,
    pub sha1: String,
    #[serde(default)]
    pub size: u64,
}

/// 这个平台上可下载的官方运行时。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaOffer {
    pub component: String,
    /// 索引里自称的版本号（如 `21.0.7`）
    pub version_name: String,
}

/// Java 管理页要的全部数据。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaOverview {
    /// 本机探测到的可用 JRE（`java -version` 真跑过的），含我们下载的那些
    pub runtimes: Vec<crate::java::JavaRuntime>,
    /// `cache/java/` 下已经装好的组件名
    pub downloaded: Vec<String>,
    /// 这个平台上可下载的官方运行时
    pub available: Vec<JavaOffer>,
    /// 取不到官方索引时的原因（离线、被墙、索引 URL 失效）。
    ///
    /// UI 必须区分两种"available 为空"：**这个平台没有官方运行时**（本字段为 None，
    /// 实测如 Linux arm64）要引导用户手动指定路径；**现在查不到**（本字段有值）
    /// 只需要重试。混成一句"没有可用的 Java"会让前者看起来像个临时故障。
    pub available_error: Option<String>,
}

// ---------------------------------------------------------------------------
// 纯函数（可离线测）
// ---------------------------------------------------------------------------

/// Mojang 的平台键。返回 `None` = 这个平台上没有官方运行时（见模块头：Linux arm64）。
///
/// 不猜、不回退到相邻架构：一个 x64 的 JRE 在 arm64 Linux 上跑不起来（除非有 qemu，
/// 而那不该由我们悄悄假设），给一个注定失败的下载比直接说"没有"更糟。
pub fn platform_key(os: &OsInfo) -> Option<&'static str> {
    match (os.name, os.arch) {
        (OsName::Windows, Arch::X64) => Some("windows-x64"),
        (OsName::Windows, Arch::X86) => Some("windows-x86"),
        (OsName::Windows, Arch::Arm64) => Some("windows-arm64"),
        (OsName::Osx, Arch::Arm64) => Some("mac-os-arm64"),
        // macOS 的 x64 键就叫 "mac-os"（不是 "mac-os-x64"）
        (OsName::Osx, Arch::X64) => Some("mac-os"),
        (OsName::Linux, Arch::X64) => Some("linux"),
        (OsName::Linux, Arch::X86) => Some("linux-i386"),
        // Linux arm64：清单里没有
        (OsName::Linux, Arch::Arm64) => None,
        // macOS 32 位不存在
        (OsName::Osx, Arch::X86) => None,
    }
}

/// 从索引里取出某个平台某个产品的 manifest 引用。
/// 平台不认识、产品不存在、或该产品在这个平台上不可用，都返回 `None`。
pub fn platform_runtime(all: &AllJson, os: &OsInfo, component: &str) -> Option<Product> {
    all.get(platform_key(os)?)?
        .get(component)?
        .iter()
        .find(|p| p.manifest.is_some())
        .cloned()
}

/// 这个平台上**可下载**的官方运行时：`(组件名, 版本名)`，按组件名排序。
///
/// 只列真有 manifest 的：空数组（这个平台没有该产品）不该出现在列表里，
/// 否则 UI 会渲染出一排点了必然失败的按钮——那比不显示更糟。
pub fn offers(all: &AllJson, os: &OsInfo) -> Vec<(String, String)> {
    let Some(platform) = platform_key(os).and_then(|k| all.get(k)) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = platform
        .iter()
        .filter_map(|(component, entries)| {
            let p = entries.iter().find(|p| p.manifest.is_some())?;
            Some((component.clone(), p.version.name.clone()))
        })
        .collect();
    out.sort();
    out
}

/// 把 manifest 变成下载任务与"需要可执行位"的相对路径列表。
///
/// 只取 `raw`（理由见模块头）。一个文件既没有 `raw` 也没有任何可下载物时**报错**，
/// 而不是跳过：少一个 `lib/modules` 的 JRE 看起来是装好了，跑起来才炸。
pub fn plan(manifest: &Manifest, dest: &Path) -> Result<(Vec<FileTask>, Vec<String>), Error> {
    let mut tasks = Vec::new();
    let mut executables = Vec::new();
    for (rel, entry) in &manifest.files {
        let ManifestEntry::File {
            executable,
            downloads,
        } = entry
        else {
            continue;
        };
        let art = downloads.get("raw").ok_or_else(|| {
            Error::Invalid(format!(
                "java runtime manifest entry has no raw download: {rel}"
            ))
        })?;
        tasks.push(FileTask {
            url: art.url.clone(),
            // manifest 的键是相对路径且来自 Mojang，但仍然过 safe_join：
            // 一个 `../../` 就能把文件写到 cache/java 之外
            dest: safe_join(dest, rel)?,
            sha1: art.sha1.clone(),
            size: art.size,
        });
        if *executable {
            executables.push(rel.clone());
        }
    }
    Ok((tasks, executables))
}

/// 下载后给可执行位。Windows 上没有这个概念，直接返回。
///
/// **只在 Unix 上做**，而且失败要报出来：少了 +x 的 `bin/java` 在 Linux/macOS 上
/// 表现为"权限不够"，而用户看到的是启动失败，看不出是下载环节漏了一步。
#[cfg(unix)]
pub fn mark_executable(dest: &Path, rel: &[String]) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    for r in rel {
        let p = safe_join(dest, r)?;
        if !p.is_file() {
            continue;
        }
        let mut perms = std::fs::metadata(&p)?.permissions();
        // 0o755：所有者可读写执行，其余可读可执行。JRE 的 bin/ 下都是这个惯例
        perms.set_mode(0o755);
        std::fs::set_permissions(&p, perms)?;
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn mark_executable(_dest: &Path, _rel: &[String]) -> Result<(), Error> {
    Ok(())
}

/// 清单本身的 sha1（索引里给了）。用 `get_text` 拿到的字符串再算，
/// 而不是拿原始字节：清单是 JSON 文本，两种读法结果一致，而这一处不值得为它开一条二进制路径。
fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// `bin/java` 的落点（Windows 上是 `bin\java.exe`）。
pub fn java_binary(dest: &Path) -> PathBuf {
    dest.join("bin")
        .join(if cfg!(windows) { "java.exe" } else { "java" })
}

/// 某个组件的本地安装目录。
pub fn component_dir(paths: &Paths, component: &str) -> Result<PathBuf, Error> {
    // component 来自 Mojang 的版本 JSON，但它会被当成目录名，所以照样校验
    if component.is_empty()
        || component.contains(['/', '\\', ':'])
        || component.split('\0').count() > 1
        || component == "."
        || component == ".."
    {
        return Err(Error::Invalid(format!(
            "unusable java runtime component: {component:?}"
        )));
    }
    Ok(paths.java().join(component))
}

/// 已经下载好的组件（`bin/java` 在位即算）。启动前先看这里，避免重复下载。
pub fn installed(paths: &Paths) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(paths.java()) else {
        return out;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if java_binary(&e.path()).is_file() {
            out.push(name);
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------------------
// 联网部分
// ---------------------------------------------------------------------------

/// 下载并安装一个官方 Java runtime。
///
/// 幂等由下载器的逐文件 sha1 校验保证；bin/java 存在不能证明其他文件已就位。
/// 重装会跳过正确文件，并修复中断安装留下的缺失或损坏文件。
pub async fn install(
    api: &ApiClient,
    dl: &Downloader,
    paths: &Paths,
    os: &OsInfo,
    component: &str,
    progress: Option<ProgressSink>,
) -> Result<InstalledJava, Error> {
    let dest = component_dir(paths, component)?;

    let all: AllJson = api.get_json(ALL_JSON_URL).await?;
    let product = platform_runtime(&all, os, component).ok_or_else(|| {
        Error::NotFound(format!(
            "no official Java runtime for this platform/component: {:?} / {component}",
            platform_key(os)
        ))
    })?;
    let manifest_ref = product.manifest.ok_or_else(|| {
        Error::NotFound(format!(
            "component {component} is not available on this platform"
        ))
    })?;
    let version_name = product.version.name.clone();

    // 清单本身也有 sha1（索引里给的）：先核对再用，否则一个被篡改或截断的清单
    // 会让我们照着它下一堆东西，最后才发现不对
    let manifest_text = api.get_text(&manifest_ref.url).await?;
    let got = sha1_hex(manifest_text.as_bytes());
    if !manifest_ref.sha1.is_empty() && got != manifest_ref.sha1 {
        return Err(Error::Checksum(format!(
            "java runtime manifest: expected {} got {got}",
            manifest_ref.sha1
        )));
    }
    let manifest: Manifest = serde_json::from_str(&manifest_text)
        .map_err(|e| Error::Config(format!("decode java runtime manifest: {e}")))?;

    let (tasks, executables) = plan(&manifest, &dest)?;
    let summary = dl.download_all(tasks, progress).await;
    if !summary.failed.is_empty() {
        return Err(Error::Config(format!(
            "failed to download {} java runtime file(s): {}",
            summary.failed.len(),
            crate::net::format_failures(&summary.failed)
        )));
    }
    mark_executable(&dest, &executables)?;

    let bin = java_binary(&dest);
    if !bin.is_file() {
        // 清单说该有、下载也没报错，但文件不在：这种情况必须报错，
        // 否则会把一个不存在的 java 路径写进实例设置，症状是启动时"找不到 java"
        return Err(Error::Config(format!(
            "java runtime installed but {} is missing",
            bin.display()
        )));
    }
    Ok(InstalledJava {
        path: bin.to_string_lossy().into_owned(),
        component: component.to_string(),
        version_name,
        download: summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::{Arch, OsInfo, OsName};

    fn fixture(name: &str) -> String {
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/").to_string() + name;
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("夹具 {p} 读不到：{e}"))
    }

    /// 真索引（`all.json` 原样存进仓库，13KB）。用它而不是手写的样本，
    /// 是因为"哪个平台有哪个产品"这件事我猜错过：gamma 不是 21，Linux arm64 压根没有。
    fn all() -> AllJson {
        serde_json::from_str(&fixture("java-runtime-all.json")).unwrap()
    }

    fn win64() -> OsInfo {
        OsInfo::new(OsName::Windows, "10.0", Arch::X64)
    }

    /// 产品名 → 当前版本。**这条断言的价值在于它是实测值而不是记忆**：
    /// gamma 是 17（不是 21），delta 才是 21。写一张凭印象的映射表就会下错版本，
    /// 而症状是"装了 Java 但游戏还说版本不对"，离原因很远。
    /// 这条会随 Mojang 发布新版本而变红——那时该改的是这里，不是实现。
    #[test]
    fn the_index_maps_components_to_the_versions_it_actually_ships() {
        let a = all();
        for (component, prefix) in [
            ("jre-legacy", "8u"),
            ("java-runtime-alpha", "16."),
            ("java-runtime-beta", "17."),
            ("java-runtime-gamma", "17."),
            ("java-runtime-delta", "21."),
            ("java-runtime-epsilon", "25."),
        ] {
            // 走 platform_runtime 而不是直接翻 map：这条断言要验的是"调用方拿到的是什么"，
            // 而调用方拿到的正是 platform_runtime 的返回值
            let got = &platform_runtime(&a, &win64(), component)
                .unwrap_or_else(|| panic!("{component} 在 windows-x64 上不可用"))
                .version
                .name;
            assert!(
                got.starts_with(prefix),
                "{component} 现在是 {got}，不再是 {prefix}*：更新这条断言（并检查是否要改文档）"
            );
        }
    }

    /// 平台键。`None` 的那些是**真的没有**官方运行时，不是"我们还没实现"：
    /// Linux arm64 整个平台不在索引里，Apple Silicon 上没有 alpha/beta/jre-legacy。
    /// 这两种情况必须明确告诉用户"请手动指定 Java"，而不是给一个转不出结果的进度条。
    #[test]
    fn platform_keys_match_what_the_index_actually_publishes() {
        let a = all();
        let cases = [
            (
                OsInfo::new(OsName::Windows, "10.0", Arch::X64),
                Some("windows-x64"),
            ),
            (
                OsInfo::new(OsName::Windows, "10.0", Arch::X86),
                Some("windows-x86"),
            ),
            (
                OsInfo::new(OsName::Windows, "11.0", Arch::Arm64),
                Some("windows-arm64"),
            ),
            (OsInfo::new(OsName::Osx, "14.0", Arch::X64), Some("mac-os")),
            (
                OsInfo::new(OsName::Osx, "14.0", Arch::Arm64),
                Some("mac-os-arm64"),
            ),
            (OsInfo::new(OsName::Linux, "6.1", Arch::X64), Some("linux")),
            (
                OsInfo::new(OsName::Linux, "6.1", Arch::X86),
                Some("linux-i386"),
            ),
            // 索引里 Linux 只有 linux 与 linux-i386
            (OsInfo::new(OsName::Linux, "6.1", Arch::Arm64), None),
            (OsInfo::new(OsName::Osx, "14.0", Arch::X86), None),
        ];
        for (os, want) in cases {
            assert_eq!(platform_key(&os), want, "{os:?}");
            // 我们给出的每个键都必须真在索引里，否则"有键但取不到东西"
            if let Some(k) = want {
                assert!(a.contains_key(k), "索引里没有平台 {k}");
            }
        }
    }

    /// Apple Silicon 上确实拿不到 Java 8/16：产品条目在，但 manifest 是空的。
    /// 这个形状（条目在、manifest 为 None）必须与"平台不存在"一样被当成不可用，
    /// 否则会在 install 里走到一个没有 URL 的分支。
    #[test]
    fn components_that_exist_but_are_unavailable_report_no_manifest() {
        let a = all();
        let arm = a.get("mac-os-arm64").expect("mac-os-arm64 应当在索引里");
        // 不可用的形状是**空数组**，不是"有对象但 manifest 为空"
        let legacy = arm.get("jre-legacy").expect("条目应当在（哪怕不可用）");
        assert!(
            legacy.is_empty(),
            "Apple Silicon 上的 jre-legacy 实测是空数组：{legacy:?}"
        );
        assert!(
            platform_runtime(
                &a,
                &OsInfo::new(OsName::Osx, "14.0", Arch::Arm64),
                "jre-legacy"
            )
            .is_none(),
            "空数组必须被当成不可用"
        );
        // delta（Java 21）在 Apple Silicon 上是有的
        assert!(
            platform_runtime(
                &a,
                &OsInfo::new(OsName::Osx, "14.0", Arch::Arm64),
                "java-runtime-delta",
            )
            .and_then(|p| p.manifest)
            .is_some_and(|m| !m.url.is_empty()),
            "java-runtime-delta 应当在 mac-os-arm64 上可用"
        );
        // Linux arm64：整个平台都不在索引里，platform_runtime 必须给 None
        let linux_arm = OsInfo::new(OsName::Linux, "6.1", Arch::Arm64);
        assert!(platform_runtime(&a, &linux_arm, "java-runtime-delta").is_none());
    }

    /// 可下载列表只含有 manifest 的那些。实测 Apple Silicon 上 `java-runtime-alpha`
    /// 与 `jre-legacy` 是空数组——它们不该出现在列表里，否则 UI 会渲染出
    /// 一排点了必然失败的按钮。
    #[test]
    fn offers_list_only_what_this_platform_can_actually_download() {
        let a = all();
        let win = offers(&a, &win64());
        let names: Vec<&str> = win.iter().map(|(c, _)| c.as_str()).collect();
        for want in ["jre-legacy", "java-runtime-gamma", "java-runtime-delta"] {
            assert!(
                names.contains(&want),
                "windows-x64 应当能下 {want}：{names:?}"
            );
        }
        assert!(
            win.iter().all(|(_, v)| !v.is_empty()),
            "列出来的每一项都该有版本号：{win:?}"
        );
        // minecraft-java-exe 是启动器自己的 exe，不是 JRE；它在索引里，
        // 但我们不该把它当"可下载的 Java"给用户
        // （它确实有 manifest，所以会被列出来——UI 侧按 component 名过滤掉）
        let arm = offers(&a, &OsInfo::new(OsName::Osx, "14.0", Arch::Arm64));
        let arm_names: Vec<&str> = arm.iter().map(|(c, _)| c.as_str()).collect();
        assert!(
            !arm_names.contains(&"jre-legacy"),
            "Apple Silicon 上没有官方 Java 8，不该列出来：{arm_names:?}"
        );
        assert!(
            arm_names.contains(&"java-runtime-delta"),
            "Apple Silicon 上应当有 Java 21：{arm_names:?}"
        );
        // Linux arm64 整个平台不在索引里 → 空列表（而不是报错，也不是猜一个相邻架构）
        assert!(offers(&a, &OsInfo::new(OsName::Linux, "6.1", Arch::Arm64)).is_empty());
    }

    /// manifest 的形状：外层一个 `files` 键。按扁平 map 解析会拿到 0 个条目，
    /// 然后"成功安装"一个空目录——所以这条要盯着。
    #[test]
    fn the_manifest_has_a_files_wrapper_and_plan_reads_through_it() {
        let m: Manifest = serde_json::from_str(&fixture("java-runtime-manifest-trimmed.json"))
            .expect("真实 manifest（裁剪过）应当能解析");
        assert!(m.files.len() >= 5, "{}", m.files.len());

        let dir = tempfile::tempdir().unwrap();
        let (tasks, executables) = plan(&m, dir.path()).unwrap();

        // 目录条目不产生下载任务
        assert!(
            !tasks.iter().any(|t| t.dest.ends_with("legal")),
            "目录条目不该变成下载任务：{tasks:?}"
        );
        // 只取 raw，不取 lzma
        assert!(
            tasks
                .iter()
                .all(|t| !t.url.contains("b4d610f2cc9d12d2b9907622eecdc1833d441551")),
            "不该用 lzma 那份：{tasks:?}"
        );
        let java_exe = tasks
            .iter()
            .find(|t| t.dest.ends_with("bin/java.exe") || t.dest.ends_with("bin\\java.exe"))
            .expect("bin/java.exe 应当在计划里");
        assert_eq!(java_exe.sha1, "38dc560c6ff23b550a9df5b79a7de0d852e6f840");
        assert_eq!(java_exe.size, 39424);
        // 体积大头 lib/modules 只有 raw，这也正是"不引 lzma 解码"的代价上限
        let modules = tasks
            .iter()
            .find(|t| t.dest.ends_with("modules"))
            .expect("lib/modules");
        assert_eq!(modules.size, 61_880_399);
        // 可执行位清单
        assert!(
            executables.iter().any(|e| e == "bin/java.exe"),
            "{executables:?}"
        );
        assert!(
            !executables.iter().any(|e| e == "lib/modules"),
            "{executables:?}"
        );
    }

    /// 一个文件条目如果连 raw 都没有，必须报错而不是跳过：
    /// 少一个 `lib/modules` 的 JRE 看起来是装好了，跑起来才炸。
    #[test]
    fn a_file_without_a_raw_download_is_an_error_not_a_silent_skip() {
        let m: Manifest = serde_json::from_str(
            r#"{"files":{
                "bin/java":{"type":"file","executable":true,"downloads":{
                    "lzma":{"url":"https://e/l","sha1":"aa","size":1}
                }}
            }}"#,
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let err = plan(&m, dir.path()).unwrap_err();
        assert!(err.to_string().contains("no raw download"), "{err}");
    }

    /// manifest 的键会被当成相对路径拼进目标目录，所以要过 safe_join。
    #[test]
    fn manifest_paths_cannot_escape_the_component_directory() {
        let m: Manifest = serde_json::from_str(
            r#"{"files":{
                "../../evil":{"type":"file","downloads":{
                    "raw":{"url":"https://e/x","sha1":"aa","size":1}
                }}
            }}"#,
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        assert!(plan(&m, dir.path()).is_err(), "越界路径必须被拒");
    }

    /// component 也会被当成目录名，同样要挡。
    #[test]
    fn the_component_name_cannot_escape_the_java_cache() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("data"));
        paths.ensure().unwrap();
        for bad in ["..", ".", "../..", "a/b", "a\\b", "C:x", ""] {
            assert!(
                component_dir(&paths, bad).is_err(),
                "{bad:?} 不该被当成组件名"
            );
        }
        let ok = component_dir(&paths, "java-runtime-delta").unwrap();
        assert!(ok.starts_with(paths.java()), "{ok:?}");
        assert!(ok.ends_with("java-runtime-delta"));
    }

    /// `installed` 只认"bin/java 真在位"的目录：下了一半的、或空的目录不该被当成已安装，
    /// 否则启动时会拿一个不存在的路径去跑。
    #[test]
    fn only_a_component_with_a_java_binary_counts_as_installed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("data"));
        paths.ensure().unwrap();
        std::fs::create_dir_all(paths.java().join("half-done/bin")).unwrap();
        std::fs::create_dir_all(paths.java().join("java-runtime-delta/bin")).unwrap();
        let bin = if cfg!(windows) {
            "bin/java.exe"
        } else {
            "bin/java"
        };
        std::fs::write(paths.java().join("java-runtime-delta").join(bin), b"").unwrap();

        assert_eq!(installed(&paths), vec!["java-runtime-delta".to_string()]);
        assert_eq!(
            java_binary(&paths.java().join("java-runtime-delta"))
                .file_name()
                .unwrap()
                .to_string_lossy(),
            if cfg!(windows) { "java.exe" } else { "java" }
        );
    }

    /// Unix 上必须给可执行位，否则 `bin/java` 存在但跑不起来，
    /// 症状是"权限不够"，而用户看不出是下载环节漏了一步。
    #[cfg(unix)]
    #[test]
    fn executables_get_the_exec_bit_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::write(dir.path().join("bin/java"), b"").unwrap();
        std::fs::write(dir.path().join("release"), b"").unwrap();

        mark_executable(dir.path(), &["bin/java".to_string(), "release".to_string()]).unwrap();

        let mode = std::fs::metadata(dir.path().join("bin/java"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "bin/java 应当可执行：{mode:o}");
        // 没在清单里标 executable 的不该被改
        let other = std::fs::metadata(dir.path().join("release"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(other & 0o111, 0, "release 不该被标成可执行：{other:o}");
        // 清单里列了但文件不存在（例如 Windows 专属条目）不该报错
        mark_executable(dir.path(), &["bin/does-not-exist".to_string()]).unwrap();
    }
}
