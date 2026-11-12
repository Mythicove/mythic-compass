//! 版本清单与元数据获取。
//!
//! 解析是纯函数（对着真实清单样本做契约测试）；网络请求统一经 `net::rewrite_mirror`，
//! 于是"换镜像"对清单、版本 JSON、资源、库**全都**生效，不需要每个模块各写一遍。

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::api::api_error;
use crate::assets::AssetIndex;
use crate::error::Error;
use crate::net::rewrite_mirror;
use crate::version::VersionJson;

pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    pub release_time: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub compliance_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestEntry>,
}

impl VersionManifest {
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    pub fn by_id(&self, id: &str) -> Option<&ManifestEntry> {
        self.versions.iter().find(|v| v.id == id)
    }

    /// 正式版（安装界面的默认视图；快照要单独开关，否则列表里全是噪音）。
    pub fn releases(&self) -> impl Iterator<Item = &ManifestEntry> {
        self.versions.iter().filter(|v| v.kind == "release")
    }
}

/// 继承链推进**一步**：把父版本并进子版本，返回 `(合并结果, 下一个要解析的父 id)`。
///
/// 两件事必须同时成立，缺一个就出 bug（两个都真出过）：
///
/// 1. 合并结果**仍然要声明** `inheritsFrom`，指向刚并进来的那个父版本。
///    `VersionJson::jar_version()` 靠它定位 client jar——继承型版本自己不带 jar，
///    jar 在父版本的目录下。清掉它，装完的 Fabric 会去找
///    `versions/fabric-loader-…/fabric-loader-….jar`，症状是启动时"client jar missing"。
/// 2. 链的推进要用**父版本自己的** `inheritsFrom`（返回值里的第二个元素），
///    不能读合并结果的。`version::merge` 是"子覆盖父"，合并结果里的 `inheritsFrom`
///    永远是子版本原来那个值，读它就等于把同一个父版本反复解析，
///    一直撞到深度上限（真装 Fabric 时暴露的：原版没有 `inheritsFrom`，
///    这条循环在 P1 从没被真跑过）。
///
/// 抽成纯函数是为了能离线测：`resolve_from` 要联网，而这两个 bug 都在这一步里。
pub fn merge_step(parent: VersionJson, child: VersionJson) -> (VersionJson, Option<String>) {
    let next = parent.inherits_from.clone();
    let parent_id = parent.id.clone();
    let mut merged = crate::version::merge(parent, child);
    merged.inherits_from = Some(parent_id);
    (merged, next)
}

/// 缓存一份 HTTP 响应所需的条件请求校验子。
///
/// 两个都存：有的服务器只给 `ETag`，有的只给 `Last-Modified`，
/// 而只带一个的话另一台镜像上缓存就永远不命中。
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Validators {
    #[serde(default)]
    etag: String,
    #[serde(default)]
    last_modified: String,
}

fn header(res: &reqwest::Response, name: &str) -> String {
    res.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

/// 解析成功之后才落盘，且落盘失败不影响返回结果（缓存是优化，写不进去不该让"列版本"失败）。
///
/// 顺序不能反：把一份解析不了的清单缓存下来，之后每次启动都会拿它当"没变"
/// （304 永远命中），症状是版本列表停在某一天，而且这个状态自己修不好。
fn store_manifest(
    body_file: &std::path::Path,
    validator_file: &std::path::Path,
    text: &str,
    validators: &Validators,
) -> Result<VersionManifest, Error> {
    let parsed: VersionManifest = serde_json::from_str(text)
        .map_err(|e| Error::Config(format!("decode {VERSION_MANIFEST_URL}: {e}")))?;
    if let Some(parent) = body_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(body_file, text);
    let _ = std::fs::write(
        validator_file,
        serde_json::to_vec(validators).unwrap_or_default(),
    );
    Ok(parsed)
}

/// 元数据客户端：默认只读 JSON、不落盘（缓存由调用方按 `paths` 布局决定）。
/// 唯一的例外是 `manifest_cached`——版本清单够大、变得够慢，值得自己管一份带
/// 条件请求的缓存；它要 `Paths`，所以不影响其余方法的无状态性。
#[derive(Clone)]
pub struct MetaClient {
    http: reqwest::Client,
    mirror: String,
}

impl MetaClient {
    pub fn new(http: reqwest::Client, mirror: impl Into<String>) -> Self {
        Self {
            http,
            mirror: mirror.into(),
        }
    }

    pub async fn json<T: DeserializeOwned>(&self, url: &str) -> Result<T, Error> {
        let text = self.text(url).await?;
        serde_json::from_str(&text).map_err(|e| Error::Config(format!("decode {url}: {e}")))
    }

    pub async fn text(&self, url: &str) -> Result<String, Error> {
        let res = self
            .http
            .get(rewrite_mirror(url, &self.mirror))
            .send()
            .await?;
        let status = res.status().as_u16();
        let body = res.text().await?;
        if !(200..300).contains(&status) {
            return Err(api_error(status, &body));
        }
        Ok(body)
    }

    pub async fn manifest(&self) -> Result<VersionManifest, Error> {
        self.json(VERSION_MANIFEST_URL).await
    }

    /// 带 ETag / Last-Modified 的版本清单。
    ///
    /// 清单 1MB 以上，而"列版本""装版本""装 loader"每次都要取一遍；它的内容几天才变一次。
    /// 条件请求命中 304 时一个字节正文都不用下——这对启动时间与弱网都有感。
    ///
    /// 三条退化路径都是刻意的，而且都退到"和没有缓存时一样"，不退到"用一个坏缓存"：
    /// - 服务端不给 ETag/Last-Modified（部分镜像如此）→ 每次全量取，行为与从前相同。
    /// - 304 但磁盘上那份读不出来或解析不了 → 重新全量取一次。缓存坏了不该让启动器不能用。
    /// - 200 但正文解析失败 → **不落盘**，直接报错。把一份坏清单缓存下来，
    ///   会让之后每次启动都拿它当"没变"，症状是版本列表停在某一天。
    ///
    /// 信任根不在这里：清单自身没有 sha1，但每个条目的版本 JSON 有，
    /// 而 `version_for` 会先对字节核 sha1 再解析（见它的注释）。
    pub async fn manifest_cached(
        &self,
        paths: &crate::paths::Paths,
    ) -> Result<VersionManifest, Error> {
        use std::fs;

        let body_file = paths.cache().join("version_manifest.json");
        let validator_file = paths.cache().join("version_manifest.validators.json");
        let validators = fs::read_to_string(&validator_file)
            .ok()
            .and_then(|t| serde_json::from_str::<Validators>(&t).ok())
            .unwrap_or_default();

        let mut req = self
            .http
            .get(rewrite_mirror(VERSION_MANIFEST_URL, &self.mirror));
        if !validators.etag.is_empty() {
            req = req.header("If-None-Match", &validators.etag);
        }
        if !validators.last_modified.is_empty() {
            req = req.header("If-Modified-Since", &validators.last_modified);
        }
        let res = req.send().await?;
        let status = res.status().as_u16();

        if status == 304 {
            if let Ok(text) = fs::read_to_string(&body_file)
                && let Ok(m) = serde_json::from_str::<VersionManifest>(&text)
            {
                return Ok(m);
            }
            // 服务端说没变，但我们手上那份用不了。全量取一次**并且把缓存修好**：
            // 只取不写的话，之后每次启动都会重复"白跑一次条件请求 + 一次全量下载"，
            // 而这个状态自己永远不会好起来。
            return self
                .fetch_and_store_manifest(&body_file, &validator_file)
                .await;
        }
        if !(200..300).contains(&status) {
            let body = res.text().await.unwrap_or_default();
            return Err(api_error(status, &body));
        }
        // 响应头必须在消费 body **之前**取：`res.text()` 之后 res 就没了
        let next = Validators {
            etag: header(&res, "etag"),
            last_modified: header(&res, "last-modified"),
        };
        let text = res.text().await?;
        store_manifest(&body_file, &validator_file, &text, &next)
    }

    /// 无条件全量取一次清单并落盘。
    async fn fetch_and_store_manifest(
        &self,
        body_file: &std::path::Path,
        validator_file: &std::path::Path,
    ) -> Result<VersionManifest, Error> {
        let res = self
            .http
            .get(rewrite_mirror(VERSION_MANIFEST_URL, &self.mirror))
            .send()
            .await?;
        let status = res.status().as_u16();
        let next = Validators {
            etag: header(&res, "etag"),
            last_modified: header(&res, "last-modified"),
        };
        let text = res.text().await?;
        if !(200..300).contains(&status) {
            return Err(api_error(status, &text));
        }
        store_manifest(body_file, validator_file, &text, &next)
    }

    /// 版本 JSON 可能是继承型的（Fabric/Forge），此时还要取父版本再合并。
    pub async fn version(&self, url: &str) -> Result<VersionJson, Error> {
        self.json(url).await
    }

    /// 取清单条目的版本 JSON，**先对字节做 sha1 校验再解析**：
    /// 清单给了 sha1 却不核对，等于允许镜像/中间人替换版本 JSON——它决定我们
    /// 下载什么、把什么放进 classpath，是整条安装链的信任根。
    pub async fn version_for(&self, entry: &ManifestEntry) -> Result<VersionJson, Error> {
        let url = rewrite_mirror(&entry.url, &self.mirror);
        let res = self.http.get(&url).send().await?;
        let status = res.status().as_u16();
        let bytes = res.bytes().await?;
        if !(200..300).contains(&status) {
            return Err(api_error(status, &String::from_utf8_lossy(&bytes)));
        }
        if !entry.sha1.is_empty() {
            let mut hasher = Sha1::new();
            hasher.update(&bytes);
            let actual = hex::encode(hasher.finalize());
            if actual != entry.sha1 {
                return Err(Error::Checksum(format!(
                    "{}: want {} got {actual}",
                    entry.url, entry.sha1
                )));
            }
        }
        serde_json::from_slice(&bytes)
            .map_err(|e| Error::Config(format!("decode {}: {e}", entry.url)))
    }

    /// 递归解析继承链：子版本在前，返回合并后的版本。
    /// 深度上限防环——loader 生成的 JSON 正常只有一层，但坏数据不该让启动器无限递归。
    pub async fn resolve_version(&self, entry: &ManifestEntry) -> Result<VersionJson, Error> {
        let child = self.version_for(entry).await?;
        self.resolve_from(child).await
    }

    /// 从一个**已经拿到的**子版本 JSON 开始解析继承链。
    ///
    /// loader 的 profile 不在官方清单里（没有清单条目可传、也没有 sha1 可核对），
    /// 所以要有这个入口；但父版本仍然从清单取并校验 sha1——信任根不变。
    /// 链的推进规则见 `merge_step`（那里记着两个真出过的 bug）。
    pub async fn resolve_from(&self, child: VersionJson) -> Result<VersionJson, Error> {
        const MAX_DEPTH: usize = 8;
        let root_id = child.id.clone();
        let mut merged = child;
        let mut next = merged.inherits_from.clone();
        // 清单在循环外取一次：它是一份 1MB+ 的文档，而继承链最多 8 层，
        // 放在循环里就是同一份文档连取 8 遍（装 Fabric/Quilt 时每次都走这条路径）
        let manifest = self.manifest().await?;
        for _ in 0..MAX_DEPTH {
            let Some(parent_id) = next.take() else {
                return Ok(merged);
            };
            let parent = manifest
                .by_id(&parent_id)
                .ok_or_else(|| Error::Invalid(format!("unknown parent version: {parent_id}")))?;
            let parent_json = self.version_for(parent).await?;
            (merged, next) = merge_step(parent_json, merged);
        }
        Err(Error::Invalid(format!(
            "version inheritance deeper than {MAX_DEPTH}: {root_id}"
        )))
    }

    pub async fn asset_index(&self, url: &str) -> Result<AssetIndex, Error> {
        self.json(url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trimmed() -> VersionManifest {
        VersionManifest::parse(include_str!(
            "../tests/fixtures/version_manifest_v2-trimmed.json"
        ))
        .unwrap()
    }

    #[test]
    fn parses_real_manifest() {
        let m = trimmed();
        assert!(!m.versions.is_empty());
        // latest 指向的版本必须真的在列表里（否则"安装最新版"会拿到 None）
        assert!(m.by_id(&m.latest.release).is_some());
        assert!(m.by_id(&m.latest.snapshot).is_some());
        let entry = m.by_id("1.21.4").unwrap();
        assert_eq!(entry.kind, "release");
        assert!(entry.url.ends_with("/1.21.4.json"));
        assert_eq!(entry.sha1.len(), 40);
        assert!(!entry.release_time.is_empty());
    }

    #[test]
    fn releases_filter_out_snapshots() {
        let m = trimmed();
        let releases: Vec<_> = m.releases().map(|v| v.id.clone()).collect();
        assert!(releases.contains(&"1.21.4".to_string()));
        assert!(releases.contains(&"1.7.10".to_string()));
        assert!(
            !releases.iter().any(|id| id.contains("snapshot")),
            "snapshots must not show up as releases: {releases:?}"
        );
        assert!(m.versions.len() > releases.len());
    }

    #[test]
    fn merge_step_keeps_the_pointer_to_the_version_that_provides_the_jar() {
        // 真实样本：Fabric 的 profile（inheritsFrom 1.21.4）+ 1.21.4 本身
        let child =
            VersionJson::parse(include_str!("../tests/fixtures/fabric-1.21.4.json")).unwrap();
        let parent = VersionJson::parse(include_str!("../tests/fixtures/1.21.4.json")).unwrap();
        assert_eq!(child.inherits_from.as_deref(), Some("1.21.4"));
        assert!(parent.inherits_from.is_none());

        let (merged, next) = merge_step(parent, child);
        // 链在这里走完：父版本是根，没有下一个
        assert_eq!(next, None, "根版本不该再往上指");
        // 合并结果仍然指向提供 client jar 的版本——jar_version() 靠它定位。
        // 清掉它的话，装完的 Fabric 会去 versions/fabric-loader-…/ 找 jar（不存在）。
        assert_eq!(merged.inherits_from.as_deref(), Some("1.21.4"));
        assert_eq!(
            merged.jar_version(),
            "1.21.4",
            "client jar 来自父版本（继承型版本自己不带 jar）"
        );
        // 合并本身没被弄坏：主类来自子版本，id 也是子版本的（装出来的目录名就是它）
        assert!(
            merged
                .main_class
                .as_deref()
                .is_some_and(|m| m.contains("fabricmc")),
            "{:?}",
            merged.main_class
        );
        // 断言形状而不是具体版本号：写死 "0.16.9" 的话，刷新 fixture 就会假失败
        assert!(merged.id.starts_with("fabric-loader-"), "{}", merged.id);
        assert!(merged.id.ends_with("-1.21.4"), "{}", merged.id);
    }

    #[test]
    fn merge_step_walks_a_multi_level_chain_one_step_at_a_time() {
        // A inherits B，B inherits C，C 是根。每调用一次只推进一层，
        // 推进的依据是**父版本自己的**指针（返回值第二项），不是合并结果里的。
        let a = VersionJson::parse(r#"{"id":"a","inheritsFrom":"b","mainClass":"A"}"#).unwrap();
        let b = VersionJson::parse(r#"{"id":"b","inheritsFrom":"c"}"#).unwrap();
        let c = VersionJson::parse(r#"{"id":"c","assets":"19"}"#).unwrap();

        let (after_b, next) = merge_step(b, a);
        assert_eq!(next.as_deref(), Some("c"), "下一步该去解析 c");
        assert_eq!(
            after_b.main_class.as_deref(),
            Some("A"),
            "子版本的字段要保住"
        );

        let (after_c, next) = merge_step(c, after_b);
        assert_eq!(next, None, "到根就该停下");
        assert_eq!(after_c.main_class.as_deref(), Some("A"));
        assert_eq!(
            after_c.assets.as_deref(),
            Some("19"),
            "根版本的字段要合并进来"
        );
        assert_eq!(after_c.jar_version(), "c", "jar 由链最上层的那个版本提供");
    }

    #[test]
    fn resolving_a_real_chain_terminates() {
        // 模拟 resolve_from 的循环（不联网）：真实的两层链必须一步收敛。
        // 之前读的是合并结果里的 inheritsFrom，于是原地打转跑满 MAX_DEPTH，
        // 报 "version inheritance deeper than 8"。
        let mut merged =
            VersionJson::parse(include_str!("../tests/fixtures/fabric-1.21.4.json")).unwrap();
        let parent = VersionJson::parse(include_str!("../tests/fixtures/1.21.4.json")).unwrap();
        let mut next = merged.inherits_from.clone();
        let mut steps = 0;
        while next.take().is_some() {
            (merged, next) = merge_step(parent.clone(), merged);
            steps += 1;
            assert!(steps <= 8, "继承链没收敛（跑满 8 步还在往上指）");
        }
        assert_eq!(steps, 1, "Fabric 的链只有一层，应当一步收敛");
        assert_eq!(merged.jar_version(), "1.21.4");
    }

    #[test]
    fn unknown_version_is_none() {
        assert!(trimmed().by_id("99.99.99").is_none());
    }

    // ---- manifest_cached：条件请求与缓存退化路径 ----

    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    #[derive(Clone, Copy)]
    enum Behavior {
        /// 带 If-None-Match 就回 304，否则 200 + ETag/Last-Modified
        Conditional,
        /// 从不给校验子（部分镜像就是这样）
        NoValidators,
        /// 200，但正文解析不了
        BadBody,
    }

    struct MetaServer {
        base: String,
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl MetaServer {
        async fn start(behavior: Behavior, body: String) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let seen = Arc::new(Mutex::new(Vec::<String>::new()));
            let sink = Arc::clone(&seen);
            tokio::spawn(async move {
                loop {
                    let Ok((stream, _)) = listener.accept().await else {
                        break;
                    };
                    let sink = Arc::clone(&sink);
                    let body = body.clone();
                    tokio::spawn(async move {
                        let _ = handle_meta(stream, behavior, body, sink).await;
                    });
                }
            });
            Self { base, seen }
        }

        fn requests(&self) -> Vec<String> {
            self.seen.lock().unwrap().clone()
        }
    }

    async fn handle_meta(
        mut stream: TcpStream,
        behavior: Behavior,
        body: String,
        seen: Arc<Mutex<Vec<String>>>,
    ) -> std::io::Result<()> {
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).await?;
        let head = String::from_utf8_lossy(&buf[..n]).to_string();
        seen.lock().unwrap().push(head.clone());

        let conditional = head.to_ascii_lowercase().contains("if-none-match:");
        let (status, headers, payload) = match behavior {
            Behavior::Conditional if conditional => ("304 Not Modified", vec![], String::new()),
            Behavior::Conditional => (
                "200 OK",
                vec![
                    ("ETag", "\"v1\"".to_string()),
                    ("Last-Modified", "Wed, 21 Oct 2026 07:28:00 GMT".to_string()),
                ],
                body,
            ),
            Behavior::NoValidators => ("200 OK", vec![], body),
            Behavior::BadBody => ("200 OK", vec![], "this is not json".to_string()),
        };
        let mut resp = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            payload.len()
        );
        for (k, v) in headers {
            resp.push_str(&format!("{k}: {v}\r\n"));
        }
        resp.push_str("\r\n");
        resp.push_str(&payload);
        stream.write_all(resp.as_bytes()).await?;
        stream.flush().await
    }

    fn paths() -> (tempfile::TempDir, crate::paths::Paths) {
        let dir = tempfile::tempdir().unwrap();
        let p = crate::paths::Paths::new(dir.path().join("data"));
        p.ensure().unwrap();
        (dir, p)
    }

    fn manifest_body() -> String {
        include_str!("../tests/fixtures/version_manifest_v2-trimmed.json").to_string()
    }

    /// 第二次请求必须带上第一次拿到的校验子，命中 304 时用磁盘上那份。
    /// 这是整个缓存存在的理由：清单 1MB+，而 304 的正文是 0 字节。
    #[tokio::test]
    async fn a_second_fetch_is_conditional_and_a_304_uses_the_cache() {
        let server = MetaServer::start(Behavior::Conditional, manifest_body()).await;
        let (_d, p) = paths();
        let meta = MetaClient::new(crate::api::http_client().unwrap(), server.base.clone());

        let first = meta.manifest_cached(&p).await.unwrap();
        assert!(!first.versions.is_empty());
        assert!(p.cache().join("version_manifest.json").is_file());
        assert!(p.cache().join("version_manifest.validators.json").is_file());

        let second = meta.manifest_cached(&p).await.unwrap();
        assert_eq!(second.versions.len(), first.versions.len());

        let reqs = server.requests();
        assert_eq!(reqs.len(), 2, "{reqs:?}");
        assert!(
            !reqs[0].to_ascii_lowercase().contains("if-none-match"),
            "第一次请求还没有校验子，不该带 If-None-Match：{}",
            reqs[0]
        );
        let lower = reqs[1].to_ascii_lowercase();
        assert!(
            lower.contains("if-none-match: \"v1\""),
            "第二次应当带上 ETag：{}",
            reqs[1]
        );
        assert!(
            lower.contains("if-modified-since: wed, 21 oct 2026 07:28:00 gmt"),
            "Last-Modified 也要带上（有的服务器只认这一个）：{}",
            reqs[1]
        );
    }

    /// 缓存坏了（读不出来、或存的是解析不了的东西）时必须退回全量取一次，
    /// 而不是把"缓存坏了"变成"版本列表永远是空的"。
    #[tokio::test]
    async fn a_broken_cache_falls_back_to_a_full_fetch() {
        let server = MetaServer::start(Behavior::Conditional, manifest_body()).await;
        let (_d, p) = paths();
        let meta = MetaClient::new(crate::api::http_client().unwrap(), server.base.clone());
        meta.manifest_cached(&p).await.unwrap();

        // 把正文换成垃圾，但保留校验子：服务端会说 304，而我们手上那份用不了
        std::fs::write(p.cache().join("version_manifest.json"), b"{ not json").unwrap();
        let got = meta.manifest_cached(&p).await.unwrap();
        assert!(
            !got.versions.is_empty(),
            "应当退回全量取，而不是报错或返回空"
        );

        // 第三次请求是无条件的（退回全量取），并且把缓存修好了
        let reqs = server.requests();
        assert!(
            !reqs[2].to_ascii_lowercase().contains("if-none-match"),
            "退回的那次不该带校验子：{}",
            reqs[2]
        );
        assert!(
            std::fs::read_to_string(p.cache().join("version_manifest.json"))
                .unwrap()
                .contains("versions")
        );
    }

    /// 200 但正文解析不了时**绝不落盘**：把一份坏清单缓存下来，会让之后每次启动
    /// 都拿它当"没变"，症状是版本列表停在某一天，而且再也修不好（304 永远命中）。
    #[tokio::test]
    async fn an_unparseable_body_is_never_cached() {
        let server = MetaServer::start(Behavior::BadBody, manifest_body()).await;
        let (_d, p) = paths();
        let meta = MetaClient::new(crate::api::http_client().unwrap(), server.base.clone());

        assert!(meta.manifest_cached(&p).await.is_err());
        assert!(
            !p.cache().join("version_manifest.json").exists(),
            "坏正文不该落盘"
        );
        assert!(
            !p.cache().join("version_manifest.validators.json").exists(),
            "校验子也不该落盘（否则下次会带着它发条件请求）"
        );
    }

    /// 服务端不给校验子（部分镜像）时，行为必须与没有缓存时完全一样：
    /// 每次全量取，不发条件头，也不写一个永远命中不了的校验子文件。
    #[tokio::test]
    async fn without_validators_every_fetch_is_full() {
        let server = MetaServer::start(Behavior::NoValidators, manifest_body()).await;
        let (_d, p) = paths();
        let meta = MetaClient::new(crate::api::http_client().unwrap(), server.base.clone());

        meta.manifest_cached(&p).await.unwrap();
        meta.manifest_cached(&p).await.unwrap();
        let reqs = server.requests();
        assert_eq!(reqs.len(), 2);
        assert!(
            reqs.iter()
                .all(|r| !r.to_ascii_lowercase().contains("if-none-match")),
            "没有 ETag 就不该发条件头：{reqs:?}"
        );
        // 正文仍然被缓存（省一次解析），但校验子是空的
        let v: Validators = serde_json::from_str(
            &std::fs::read_to_string(p.cache().join("version_manifest.validators.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(v, Validators::default());
    }
}
