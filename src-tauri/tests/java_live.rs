//! 真下载一份 Mojang 官方 JRE，并用 `java -version` 证明它跑得起来。
//!
//! 覆盖官方运行时安装入口；完整自动选择 Java 与游戏启动另行验收。
//! 纯离线的形状测试证明不了这件事——manifest 解析对了、下载计划对了，
//! 仍然可能因为漏了可执行位、路径拼错、或者 sha1 校验把正确的文件判成坏的而装不出一个能用的 JRE。
//!
//! Java 8 保留本地旧版本验收；Java 21 在三平台 CI 上验证现代整合包运行时。

mod live;

use compass_lib::javaget;
use compass_lib::paths::Paths;
use compass_lib::version::OsInfo;

#[tokio::test]
#[ignore = "needs network and downloads the official Java 8 runtime"]
async fn downloads_a_real_jre_and_it_actually_runs() {
    check_runtime("jre-legacy", 8).await;
}

#[tokio::test]
#[ignore = "needs network and downloads the official Java 21 runtime"]
async fn downloads_modern_java_and_repairs_its_installation() {
    check_runtime("java-runtime-delta", 21).await;
}

async fn check_runtime(component: &str, major: u32) {
    let data = live::data_dir("COMPASS_JAVA_DIR");
    let paths = Paths::new(data.path().to_path_buf());
    paths.ensure().unwrap();
    let http = live::http();
    let api = live::api(&http);
    let dl = compass_lib::net::Downloader::new(http.clone(), 8, String::new(), &paths);
    let os = OsInfo::current();

    // 这个平台得真有官方运行时，否则测的是"报错对不对"而不是"下载行不行"
    assert!(
        javaget::platform_runtime(
            &serde_json::from_str(
                &compass_lib::api::ApiClient::new(http.clone())
                    .get_text(javaget::ALL_JSON_URL)
                    .await
                    .unwrap()
            )
            .unwrap(),
            &os,
            component
        )
        .is_some(),
        "这个平台（{os:?}）没有官方 {component}，换一个组件或跳过这条测试"
    );

    let got = javaget::install(&api, &dl, &paths, &os, component, None)
        .await
        .expect("官方 JRE 应当能装下来");
    println!(
        "installed {} -> {} ({} files, {} MB)",
        got.component,
        got.path,
        got.download.downloaded + got.download.skipped,
        (got.download.bytes / 1024 / 1024)
    );

    let bin = std::path::PathBuf::from(&got.path);
    assert!(bin.is_file(), "{} 不在", got.path);
    assert_eq!(
        bin,
        javaget::java_binary(&javaget::component_dir(&paths, component).unwrap())
    );

    // 真跑一次 -version：这一步才证明它是个能用的 JRE，而不是一堆凑齐了字节的文件
    let probed = compass_lib::java::probe(&bin)
        .await
        .expect("下载下来的 JRE 应当能被探测到版本");
    assert_eq!(
        probed.version, major,
        "下到的是 Java {}，不是 {major}",
        probed.version
    );
    assert!(
        probed.is_64bit,
        "official runtime must match the 64-bit platform"
    );
    println!("probed: {:?}", probed);

    // 幂等：再装一次不该重新下载
    let again = javaget::install(&api, &dl, &paths, &os, component, None)
        .await
        .unwrap();
    assert_eq!(again.download.downloaded, 0, "{:?}", again.download);
    assert_eq!(again.path, got.path);

    // 中断安装可能留下 bin/java 却缺少其余文件：重装必须修复，而不是提前报成功。
    let release =
        std::fs::canonicalize(bin.parent().unwrap().parent().unwrap().join("release")).unwrap();
    std::fs::remove_file(&release).unwrap();
    let repaired = javaget::install(&api, &dl, &paths, &os, component, None)
        .await
        .unwrap();
    assert!(
        release.is_file(),
        "runtime with an existing Java binary was not repaired"
    );
    assert!(repaired.download.downloaded > 0);
    let repaired_runtime = compass_lib::java::probe(&bin).await.unwrap();
    assert_eq!(repaired_runtime.version, major);
    assert!(repaired_runtime.is_64bit);

    // 已装组件要能被列出来（Java 管理页就是读这个）
    assert_eq!(javaget::installed(&paths), vec![component.to_string()]);

    let acceptance = serde_json::json!({
        "component": component,
        "java_major": repaired_runtime.version,
        "is_64bit": repaired_runtime.is_64bit,
        "files": got.download.downloaded + got.download.skipped,
        "reinstall_downloaded": again.download.downloaded,
        "missing_release_repaired": release.is_file(),
        "scope": "official runtime download, executable probe, cache reuse and missing-file repair"
    });
    std::fs::write(
        paths.root().join(format!("{component}-acceptance.json")),
        serde_json::to_vec_pretty(&acceptance).unwrap(),
    )
    .unwrap();
}
