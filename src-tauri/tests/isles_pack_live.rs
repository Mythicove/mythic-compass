//! Cross-product acceptance: consume the exact .mrpack published by a real Isles
//! process. Run with Isles scripts/compass-pack-live.mjs; requires network,
//! Java 21 and a desktop/OpenGL session. No replacement installer or fake JVM.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use compass_lib::{api, config, instance, launch, meta, modpack, net, paths::Paths, runner};
use sha1::{Digest, Sha1};
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::test]
#[ignore = "run with real Isles publication, network, Java 21 and desktop/OpenGL"]
async fn published_pack_installs_and_initializes_a_real_client() {
    let pack =
        PathBuf::from(std::env::var_os("COMPASS_ISLES_PACK").expect("published .mrpack path"));
    let run = PathBuf::from(std::env::var_os("COMPASS_ISLES_RUN").expect("isolated run directory"));
    let origin = std::env::var("COMPASS_ISLES_ORIGIN").expect("external identity boundary origin");
    let paths = Paths::new(run.join("compass-data"));
    paths.ensure().unwrap();
    let http = api::http_client().unwrap();
    let api = api::ApiClient::new(http.clone());
    let installer = compass_lib::install::Installer::new(
        meta::MetaClient::new(http.clone(), String::new()),
        net::Downloader::new(http.clone(), 16, String::new()),
        paths.clone(),
    );
    let info = modpack::inspect(&pack).unwrap();
    assert_eq!(info.minecraft.as_deref(), Some("1.21.1"));
    assert_eq!(info.loader_version.as_deref(), Some("0.16.14"));
    assert_eq!(
        (info.files_required, info.files_optional, info.files_skipped),
        (1, 1, 1)
    );
    assert!(info.warnings.is_empty(), "{:?}", info.warnings);
    println!("Installing the real Isles-published pack: {}", info.name);
    let installed = modpack::install(
        &paths,
        &api,
        &installer,
        &pack,
        "isles-pack",
        "Isles live",
        None,
    )
    .await
    .unwrap();
    assert!(installed.failed.is_empty(), "{:?}", installed.failed);
    assert!(installed.warnings.is_empty(), "{:?}", installed.warnings);
    assert_eq!(installed.downloaded, 2);
    assert_eq!(installed.optional_installed, 1);
    assert_eq!(installed.skipped_unsupported, 1);
    assert!(
        installed.deps.missing.is_empty(),
        "{:?}",
        installed.deps.missing
    );
    assert!(
        installed.deps.blocked.is_empty(),
        "{:?}",
        installed.deps.blocked
    );
    let game = instance::game_dir(&paths, "isles-pack").unwrap();
    let index = modpack::read_index(&pack).unwrap();
    for file in &index.files {
        let target = game.join(&file.path);
        if file
            .env
            .as_ref()
            .is_some_and(|env| env.client == "unsupported")
        {
            assert!(
                !target.exists(),
                "server-only file installed: {}",
                file.path
            );
        } else {
            let bytes = std::fs::read(&target).unwrap();
            assert_eq!(bytes.len() as u64, file.file_size);
            assert_eq!(hex::encode(Sha1::digest(&bytes)), file.hashes["sha1"]);
        }
    }
    assert_eq!(
        std::fs::read_to_string(game.join("config/mythic-isles.properties")).unwrap(),
        "community=Isles acceptance\n"
    );
    assert_eq!(
        std::fs::read_to_string(game.join("config/optional.txt")).unwrap(),
        "optional client file\n"
    );
    let duplicate = modpack::install(
        &paths,
        &api,
        &installer,
        &pack,
        "isles-pack",
        "duplicate",
        None,
    )
    .await;
    assert!(duplicate.is_err(), "existing instance was replaced");
    assert_eq!(
        instance::load(&paths, "isles-pack").unwrap().name,
        "Isles live"
    );

    // Launch the stored instance through the same preparation used by commands.
    // The account is a loopback protocol fixture, not an authenticated player.
    let account = serde_json::from_value(serde_json::json!({
        "playerName":"IslesTester", "playerUuid":"7c0f77ad48953c9aa5be05ce3d65be75",
        "accessToken":"00000000000000000000000000000000", "apiUrl":origin
    }))
    .unwrap();
    let mut inst = instance::load(&paths, "isles-pack").unwrap();
    inst.memory_mb = Some(2048);
    inst.resolution = Some(instance::Resolution {
        width: 854,
        height: 480,
    });
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        inst.java_path = Some(PathBuf::from(home).join("bin").join(if cfg!(windows) {
            "java.exe"
        } else {
            "java"
        }));
    }
    instance::save(&paths, &inst).unwrap();
    let cfg = config::Config {
        api_url: origin,
        ..config::Config::default()
    };
    let prepared = runner::prepare_launch(&paths, &http, &cfg, &inst, &account, None)
        .await
        .unwrap();
    assert_eq!(prepared.java_version, 21);
    println!("Starting actual Minecraft client: {}", inst.version_id);
    let mut child = launch::spawn(&prepared.plan).await.unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = tx.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = out.send(line);
        }
    });
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = tx.send(line);
        }
    });
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut lines = Vec::new();
    let mut initialized = false;
    while Instant::now() < deadline {
        if let Ok(Some(line)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            initialized |= line.contains("minecraft:textures/atlas/blocks.png-atlas");
            lines.push(line);
            if initialized {
                break;
            }
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
    stdout_task.await.unwrap();
    stderr_task.await.unwrap();
    while let Ok(line) = rx.try_recv() {
        lines.push(line);
    }
    std::fs::write(run.join("minecraft-client.log"), lines.join("\n")).unwrap();
    assert!(
        initialized,
        "client initialization not reached; see {}",
        run.join("minecraft-client.log").display()
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("fabric-api 0.116.18+1.21.1")),
        "real Fabric API not loaded"
    );
    // Change only the external download boundary after publication. Same size,
    // wrong bytes: an installer accepting size alone must fail this scenario.
    assert!(
        http.post(format!("{}/fixture/corrupt-optional", cfg.api_url))
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    let corrupt = modpack::install(
        &paths,
        &api,
        &installer,
        &pack,
        "isles-corrupt",
        "Corrupt download",
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        corrupt.failed.len(),
        1,
        "corrupt file was not reported: {:?}",
        corrupt.failed
    );
    let corrupt_file = instance::game_dir(&paths, "isles-corrupt")
        .unwrap()
        .join("config/optional.txt");
    assert!(
        !corrupt_file.exists(),
        "corrupt bytes were promoted into the game directory"
    );
    assert!(
        !net::part_path(&corrupt_file).exists(),
        "corrupt partial file retained"
    );
    let manifest = serde_json::json!({
        "minecraft":"1.21.1", "loader":"fabric", "loaderVersion":"0.16.14",
        "modrinthVersion":"5SwzqWUu", "instanceVersion":inst.version_id,
        "filesDownloaded":installed.downloaded, "optionalInstalled":installed.optional_installed,
        "serverFilesSkipped":installed.skipped_unsupported, "overridesApplied":installed.override_files,
        "fileSha1AndSizeVerified":true, "duplicateImport":"refused",
        "corruptDownload":"reported-and-not-promoted",
        "client":"block-atlas-initialized", "fabricApi":"loaded",
        "authentication":"loopback-fixture-not-online-account-validation",
        "scope":"native-import-and-launch-core-not-desktop-click-or-game-server-join"
    });
    std::fs::write(
        run.join("compass-result.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    println!("{manifest}");
}
