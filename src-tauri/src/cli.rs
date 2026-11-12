//! 命令行参数与便携模式：这两件事都是"启动器是怎么被叫起来的"。
//!
//! 参数解析**不静默丢弃无法识别的东西**。用户打了 `--lauch`（少个 n）却看到启动器正常
//! 起来，会以为自己那条命令生效了——那比报错更糟，因为他接下来会去查为什么游戏没启动。
//! 所以 `Intent.unknown` 会一路带到 UI 上显示出来。
//!
//! 还有一个容易漏掉的入口：`.mrpack` 的文件关联（见 tauri.conf.json 的
//! `bundle.fileAssociations`）会让 Windows 用 `compass.exe "C:\…\pack.mrpack"` 启动我们，
//! 也就是**第一个位置参数可能是一个整合包路径**。只认 `--flag` 的解析器会让"双击整合包"
//! 变成一个什么都不做的正常启动。

use serde::Serialize;

/// 解析出来的启动意图。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Intent {
    /// `--instance <slug>`：要操作的实例
    pub instance: Option<String>,
    /// `--launch`：直接启动 `instance` 指定的那个实例
    pub launch: bool,
    /// 双击 `.mrpack`（或 `--import <path>`）：要导入的整合包
    pub import_modpack: Option<String>,
    /// 没认出来的参数，原样保留。UI 要显示出来，见模块头。
    pub unknown: Vec<String>,
}

/// 解析 argv（**包含** argv[0]，与 `std::env::args()` 一致）。
pub fn parse<I, S>(args: I) -> Intent
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = Intent::default();
    let mut iter = args.into_iter().skip(1); // argv[0] 是程序自己
    while let Some(arg) = iter.next() {
        let a = arg.as_ref();
        // 同时接受 `--instance foo` 与 `--instance=foo`：两种写法都有人用，
        // 只认一种的话另一种会掉进 unknown，而用户看不出区别
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (a, None),
        };
        match flag {
            "--instance" | "-i" => {
                let value = inline.or_else(|| iter.next().map(|s| s.as_ref().to_string()));
                match value {
                    Some(v) if !v.trim().is_empty() => out.instance = Some(v),
                    // `--instance` 后面什么都没有：这本身就是个错误，别当成"没指定"
                    _ => out.unknown.push(a.to_string()),
                }
            }
            "--launch" | "-l" => out.launch = true,
            "--import" => {
                let value = inline.or_else(|| iter.next().map(|s| s.as_ref().to_string()));
                match value {
                    Some(v) if !v.trim().is_empty() => out.import_modpack = Some(v),
                    _ => out.unknown.push(a.to_string()),
                }
            }
            // 位置参数：只认整合包路径（文件关联那条路进来的），其余进 unknown
            _ if !a.starts_with('-') => {
                if is_modpack(a) {
                    out.import_modpack = Some(a.to_string());
                } else {
                    out.unknown.push(a.to_string());
                }
            }
            _ => out.unknown.push(a.to_string()),
        }
    }
    out
}

/// 进程 argv（给 setup 用）。单独一个函数是为了能在测试里替换掉。
pub fn from_env() -> Intent {
    parse(std::env::args())
}

/// 是不是一个整合包文件。只按扩展名判断：`.mrpack` 是 Modrinth 的格式，
/// 也是 Hoard 导出的格式（见 modpack 模块）。
///
/// 这里**不**去猜 `.zip`：一个 zip 可能是整合包、也可能是一个 mod、一个资源包、
/// 一个存档。猜错的后果是把用户的存档当整合包解开，那是会毁数据的。
pub fn is_modpack(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    // Windows 上双击关联文件时路径可能带引号
    let trimmed = lower.trim_matches('"');
    trimmed.ends_with(".mrpack")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Intent {
        let mut v = vec!["compass".to_string()];
        v.extend(args.iter().map(|s| s.to_string()));
        parse(v)
    }

    #[test]
    fn no_arguments_is_an_empty_intent() {
        assert_eq!(
            argv(&[]),
            Intent {
                instance: None,
                launch: false,
                import_modpack: None,
                unknown: vec![]
            }
        );
    }

    #[test]
    fn instance_and_launch_in_both_spellings() {
        for args in [
            vec!["--instance", "main", "--launch"],
            vec!["--instance=main", "--launch"],
            vec!["-i", "main", "-l"],
            // 顺序不该有影响
            vec!["--launch", "--instance", "main"],
        ] {
            let got = argv(&args);
            assert_eq!(got.instance.as_deref(), Some("main"), "{args:?}");
            assert!(got.launch, "{args:?}");
            assert!(got.unknown.is_empty(), "{args:?} -> {:?}", got.unknown);
        }
    }

    /// 打错的参数必须被看见，不能被静默忽略：否则用户以为命令生效了，
    /// 然后去查"为什么游戏没启动"。
    #[test]
    fn unrecognized_arguments_are_reported_not_dropped() {
        let got = argv(&["--lauch", "--instance"]);
        assert_eq!(
            got.unknown,
            vec!["--lauch".to_string(), "--instance".to_string()]
        );
        assert!(!got.launch, "拼错的 --lauch 不该被当成 --launch");
        // `--instance` 后面没有值：也是错误，不是"没指定"
        assert_eq!(got.instance, None);
    }

    /// 双击 `.mrpack`：Windows 会把路径作为第一个位置参数传进来。
    /// 这是文件关联那条路，只认 `--flag` 的解析器会让它变成一个什么都不做的正常启动。
    #[test]
    fn a_modpack_path_arrives_as_a_positional_argument() {
        for p in [
            r"C:\Users\me\Downloads\pack.mrpack",
            "/home/me/pack.mrpack",
            // 带空格的路径，Windows 会加引号
            r#""C:\My Packs\big pack.mrpack""#,
            // 大小写
            "/home/me/Pack.MRPACK",
        ] {
            let got = argv(&[p]);
            assert!(
                got.import_modpack.is_some(),
                "{p:?} 应当被认成整合包，结果是 {got:?}"
            );
            assert!(got.unknown.is_empty(), "{p:?} -> {:?}", got.unknown);
        }
        // 显式写法也认
        assert_eq!(
            argv(&["--import", "/tmp/a.mrpack"])
                .import_modpack
                .as_deref(),
            Some("/tmp/a.mrpack")
        );
    }

    /// 其它位置参数不该被当成整合包。尤其是 `.zip`：一个 zip 可能是整合包、
    /// 也可能是一个 mod、一个资源包或一个存档，猜错的后果是把存档当整合包解开。
    #[test]
    fn other_positional_arguments_are_not_treated_as_modpacks() {
        for p in [
            "main",
            r"C:\Users\me\world.zip",
            "/home/me/some-mod.jar",
            "/home/me/pack.zip",
        ] {
            let got = argv(&[p]);
            assert_eq!(got.import_modpack, None, "{p:?} 不该被当成整合包：{got:?}");
            assert_eq!(got.unknown, vec![p.to_string()], "{p:?}");
        }
    }

    /// 值里带 `=` 的情况（路径里有等号是合法的），以及空的 `--instance=`
    #[test]
    fn values_containing_equals_are_kept_whole() {
        assert_eq!(
            argv(&["--import=/tmp/a=b.mrpack"])
                .import_modpack
                .as_deref(),
            Some("/tmp/a=b.mrpack")
        );
        let got = argv(&["--instance="]);
        assert_eq!(got.instance, None, "空的 --instance= 不该被当成指定了实例");
        assert_eq!(got.unknown, vec!["--instance=".to_string()]);
    }
}
