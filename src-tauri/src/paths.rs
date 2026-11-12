//! 数据目录布局（计划文档 P0 决策 3）：**全局缓存跨实例共享**（assets/libraries/versions/java
//! 按 sha1 或 Maven 坐标天然去重，多实例不重复下载），**实例只存自己的游戏目录**。
//!
//! root 由 Tauri 的 app_data_dir 给出（Windows %APPDATA%、macOS ~/Library/Application Support、
//! Linux $XDG_DATA_HOME），不自己拼平台路径；测试用 `Paths::new(临时目录)` 注入，
//! 因此本模块没有任何"只有测试会设"的字段。

use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::error::Error;

/// 便携模式的标记文件名：放在可执行文件旁边，内容不重要（空文件即可）。
pub const PORTABLE_MARKER: &str = "portable.txt";

/// 便携模式的数据根目录：标记文件存在时是它旁边的 `data/`，否则 `None`。
///
/// 用途是把整个启动器放进 U 盘或云同步目录里带着走。判断只看那个标记文件在不在，
/// **不看 exe 装在哪**：装在 Program Files 里也能开便携模式（虽然那样多半写不进去，
/// 而写入失败会明确报错，不该由这里预先替用户决定）。
///
/// 用标记文件而不是命令行参数，是因为"便携"是一个**部署属性**而不是每次启动的选择：
/// 双击图标、文件关联、开机自启这些入口都不会带参数，靠参数判断就会时灵时不灵。
pub fn portable_root(marker: &Path) -> Option<PathBuf> {
    if !marker.is_file() {
        return None;
    }
    Some(marker.parent()?.join("data"))
}

/// 把来自外部数据（版本 JSON、资源索引）的相对路径安全地拼到 root 下。
/// 空串、绝对路径与任何 `..` 都拒绝：远端 JSON 不该有能力把文件写到数据目录之外。
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, Error> {
    if rel.is_empty() {
        return Err(Error::Invalid("empty relative path".into()));
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(Error::Invalid(format!(
            "absolute path is not allowed under {}: {rel:?}",
            root.display()
        )));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(Error::Invalid(format!(
            "path escapes its root {}: {rel:?}",
            root.display()
        )));
    }
    Ok(root.join(path))
}

#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 全局设置。
    pub fn config_file(&self) -> PathBuf {
        self.root.join("compass.json")
    }

    /// 已登录账号（含令牌）。P4 会把令牌迁到系统钥匙串，此文件只留非敏感部分。
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }

    /// 令牌等机密的落点。**与账号列表分开存**：accounts.json 里只有身份与时间戳
    /// （那是要给前端看的），令牌单独一个文件，好收紧权限、也好在退出登录时单独删干净。
    pub fn secrets_file(&self) -> PathBuf {
        self.root.join("secrets.json")
    }

    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// Minecraft 资源对象（`objects/<aa>/<sha1>`），跨版本共享。
    pub fn assets(&self) -> PathBuf {
        self.cache().join("assets")
    }

    /// 库文件，按 Maven 路径布局（`group/path/artifact/version/…`），跨版本共享。
    pub fn libraries(&self) -> PathBuf {
        self.cache().join("libraries")
    }

    /// 版本 JSON 与 client jar（`versions/<id>/`），跨实例共享。
    pub fn versions(&self) -> PathBuf {
        self.cache().join("versions")
    }

    /// 自动下载的 JRE（P5）。
    pub fn java(&self) -> PathBuf {
        self.cache().join("java")
    }

    /// 我们自带并校验的工具 jar（authlib-injector 等）。
    pub fn tools(&self) -> PathBuf {
        self.cache().join("tools")
    }

    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }

    /// 建出 P0 需要的目录（幂等）。versions/assets 等按需再建，避免空目录噪音。
    pub fn ensure(&self) -> Result<(), Error> {
        for dir in [self.root(), &self.cache(), &self.instances()] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// 设置页展示的目录布局（让用户知道东西装在哪、便携化/备份要拷什么）。
    pub fn info(&self) -> PathsInfo {
        PathsInfo {
            root: self.root.display().to_string(),
            config_file: self.config_file().display().to_string(),
            accounts_file: self.accounts_file().display().to_string(),
            secrets_file: self.secrets_file().display().to_string(),
            cache: self.cache().display().to_string(),
            assets: self.assets().display().to_string(),
            libraries: self.libraries().display().to_string(),
            versions: self.versions().display().to_string(),
            java: self.java().display().to_string(),
            tools: self.tools().display().to_string(),
            instances: self.instances().display().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathsInfo {
    pub root: String,
    pub config_file: String,
    pub accounts_file: String,
    /// 令牌所在。**要显示给用户**：现在它是明文文件，藏着不说等于让人以为已经加密了。
    pub secrets_file: String,
    pub cache: String,
    pub assets: String,
    pub libraries: String,
    pub versions: String,
    pub java: String,
    pub tools: String,
    pub instances: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 便携模式由"可执行文件旁边有没有那个标记文件"决定，与 exe 装在哪无关。
    #[test]
    fn portable_mode_is_decided_by_a_marker_file_next_to_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(PORTABLE_MARKER);

        // 没有标记文件 → 不便携
        assert_eq!(portable_root(&marker), None);

        // 空文件就算（内容不重要）
        std::fs::write(&marker, b"").unwrap();
        assert_eq!(portable_root(&marker), Some(dir.path().join("data")));

        // 是个**目录**而不是文件时不算：一个叫 portable.txt 的目录几乎肯定是误操作，
        // 而据此改变数据目录会让用户以为数据丢了
        std::fs::remove_file(&marker).unwrap();
        std::fs::create_dir_all(&marker).unwrap();
        assert_eq!(portable_root(&marker), None);
    }

    #[test]
    fn layout_is_rooted_and_shared() {
        let p = Paths::new("/tmp/compass-test");
        assert_eq!(
            p.config_file(),
            PathBuf::from("/tmp/compass-test/compass.json")
        );
        // 缓存与实例是两棵子树：缓存全局共享，实例各自隔离
        assert!(p.assets().starts_with(p.cache()));
        assert!(p.libraries().starts_with(p.cache()));
        assert!(p.versions().starts_with(p.cache()));
        assert!(p.instances().starts_with(p.root()));
    }

    #[test]
    fn safe_join_rejects_escape_attempts() {
        let root = Path::new("/data/cache/libraries");
        assert_eq!(
            safe_join(root, "org/lwjgl/x.jar").unwrap(),
            PathBuf::from("/data/cache/libraries/org/lwjgl/x.jar")
        );
        assert!(safe_join(root, "../escape.jar").is_err());
        assert!(safe_join(root, "org/../../escape.jar").is_err());
        assert!(safe_join(root, "").is_err());
        #[cfg(windows)]
        assert!(safe_join(root, r"C:\Windows\system32\x.dll").is_err());
        #[cfg(not(windows))]
        assert!(safe_join(root, "/etc/passwd").is_err());
    }

    #[test]
    fn ensure_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let p = Paths::new(dir.path().join("data"));
        p.ensure().unwrap();
        p.ensure().unwrap();
        assert!(p.cache().is_dir() && p.instances().is_dir());
    }
}
