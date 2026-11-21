//! Cross-product acceptance: consume the exact .mrpack published by a real Isles
//! process. Run with Isles scripts/compass-pack-live.mjs; requires network,
//! a desktop/OpenGL session and a real isolated Mirror process.
//! Downloads the game's official Java runtime.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use compass_lib::{
    api, config, instance, javaget, launch, meta, modpack, net, paths::Paths, runner,
};
use sha1::{Digest, Sha1};
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::test]
#[ignore = "run with real Isles publication, network and desktop/OpenGL"]
async fn published_pack_installs_and_initializes_a_real_client() {
    let pack =
        PathBuf::from(std::env::var_os("COMPASS_ISLES_PACK").expect("published .mrpack path"));
    let run = PathBuf::from(std::env::var_os("COMPASS_ISLES_RUN").expect("isolated run directory"));
    let origin = std::env::var("COMPASS_ISLES_ORIGIN").expect("pack source boundary origin");
    let mirror_origin = std::env::var("COMPASS_MIRROR_ORIGIN").expect("real Mirror origin");
    let player = std::env::var("COMPASS_MIRROR_PLAYER").expect("registered player");
    let password = std::env::var("COMPASS_MIRROR_PASSWORD").expect("launcher credential");
    let game_port = std::env::var("COMPASS_GAME_PORT")
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let skin_digest =
        std::env::var("COMPASS_SKIN_SHA256").expect("uploaded normalized skin digest");
    let paths = Paths::new(run.join("compass-data"));
    paths.ensure().unwrap();
    let http = api::http_client().unwrap();
    let api = api::ApiClient::new(http.clone());
    let installer = compass_lib::install::Installer::new(
        meta::MetaClient::new(http.clone(), String::new()),
        net::Downloader::new(http.clone(), 16, String::new(), &paths),
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

    // Real launcher authentication through the same transport used by commands.
    let authenticated = api
        .authenticate(&mirror_origin, &player, &password)
        .await
        .unwrap();
    assert_eq!(authenticated.available_profiles.len(), 1);
    let profile = authenticated.selected_profile.unwrap();
    assert_eq!(profile.name, player);
    let now = compass_lib::account::now_unix();
    let account = compass_lib::account::Account {
        player_name: profile.name,
        player_uuid: profile.id,
        access_token: authenticated.access_token,
        client_token: authenticated.client_token,
        api_url: mirror_origin.clone(),
        signed_in_at: now,
        token_obtained_at: now,
        kind: compass_lib::account::AccountKind::Yggdrasil,
    };
    compass_lib::account::save(&paths, &account).unwrap();
    let (stored, warning) = compass_lib::account::load(&paths);
    assert!(warning.is_none(), "account load warning: {warning:?}");
    let stored = stored.unwrap();
    assert_eq!(stored.access_token, account.access_token);
    api.validate(&mirror_origin, &stored.access_token, &stored.client_token)
        .await
        .unwrap();
    let mut inst = instance::load(&paths, "isles-pack").unwrap();
    inst.memory_mb = Some(2048);
    inst.resolution = Some(instance::Resolution {
        width: 854,
        height: 480,
    });
    let version = runner::read_installed_version(&paths, &inst.version_id).unwrap();
    let required_java = version.java_version.as_ref().unwrap();
    let runtime = javaget::install(
        &api,
        &net::Downloader::new(http.clone(), 16, String::new(), &paths),
        &paths,
        &compass_lib::version::OsInfo::current(),
        &required_java.component,
        None,
    )
    .await
    .unwrap();
    let java = PathBuf::from(&runtime.path);
    assert!(java.starts_with(paths.java()));
    let probed = compass_lib::java::probe(&java).await.unwrap();
    assert_eq!(probed.version, required_java.major_version);
    assert!(probed.is_64bit);
    inst.java_path = Some(java.clone());
    // This native-core scenario starts with Minecraft's first-run prompts accepted.
    // The desktop onboarding clicks are a separate UI scenario.
    std::fs::write(
        instance::game_dir(&paths, "isles-pack")
            .unwrap()
            .join("options.txt"),
        "onboardAccessibility:false\nskipMultiplayerWarning:true\n",
    )
    .unwrap();
    inst.server = Some(launch::ServerJoin {
        host: "127.0.0.1".into(),
        port: Some(game_port),
    });
    instance::save(&paths, &inst).unwrap();
    let cfg = config::Config {
        api_url: mirror_origin.clone(),
        ..config::Config::default()
    };
    let prepared = runner::prepare_launch(&paths, &http, &cfg, &inst, &account, None)
        .await
        .unwrap();
    assert_eq!(prepared.java_version, 21);
    assert_eq!(prepared.plan.program, java);
    assert!(
        prepared.injected,
        "authlib-injector missing from the launch plan"
    );
    // Run a real online-mode server with the same pinned injector and downloaded Java.
    let server_work = run.join("game-server");
    let server_log = std::fs::File::create(run.join("minecraft-server.log")).unwrap();
    let mut server = tokio::process::Command::new(&java)
        .args(["-Xms256M", "-Xmx1536M", "-Dauthlibinjector.side=server"])
        .arg(compass_lib::authlibinjector::agent_arg(
            &compass_lib::authlibinjector::jar_path(&paths),
            &mirror_origin,
        ))
        .args(["-jar", "server.jar", "--nogui"])
        .current_dir(&server_work)
        .stdout(server_log.try_clone().unwrap())
        .stderr(server_log)
        .stdin(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let server_deadline = Instant::now() + Duration::from_secs(240);
    while !server_work.join("server-ready.json").is_file() {
        assert!(
            server.try_wait().unwrap().is_none(),
            "server exited; see minecraft-server.log"
        );
        assert!(
            Instant::now() < server_deadline,
            "server startup timed out; see minecraft-server.log"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let ready: serde_json::Value =
        serde_json::from_slice(&std::fs::read(server_work.join("server-ready.json")).unwrap())
            .unwrap();
    assert_eq!(ready["onlineMode"], true);
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
            lines.push(
                line.replace(&account.access_token, "[redacted]")
                    .replace(&account.client_token, "[redacted]"),
            );
            if initialized && server_work.join("server-player.json").is_file() {
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
        lines.push(
            line.replace(&account.access_token, "[redacted]")
                .replace(&account.client_token, "[redacted]"),
        );
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
    assert!(
        lines.iter().any(|line| line.contains("authlib-injector")),
        "client did not report authlib-injector"
    );
    let observed: serde_json::Value = serde_json::from_slice(
        &std::fs::read(server_work.join("server-player.json"))
            .expect("client did not join; see client/server logs"),
    )
    .unwrap();
    assert_eq!(observed["onlineMode"], true);
    assert_eq!(observed["name"], player);
    assert_eq!(
        observed["uuid"].as_str().unwrap().replace('-', ""),
        account.player_uuid.replace('-', "")
    );
    assert_eq!(observed["skinModel"], "SLIM");
    assert_eq!(observed["skinSigned"], true);
    let skin_url = observed["skinUrl"].as_str().unwrap();
    assert!(skin_url.starts_with(&format!("{mirror_origin}/textures/")));
    let skin = http
        .get(skin_url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .bytes()
        .await
        .unwrap();
    let downloaded_skin = run.join("server-observed-skin.png");
    std::fs::write(&downloaded_skin, &skin).unwrap();
    assert_eq!(
        net::sha256_of_file(&downloaded_skin).await.unwrap(),
        skin_digest
    );
    use tokio::io::AsyncWriteExt;
    server
        .stdin
        .take()
        .unwrap()
        .write_all(b"stop\n")
        .await
        .unwrap();
    if tokio::time::timeout(Duration::from_secs(30), server.wait())
        .await
        .is_err()
    {
        server.kill().await.unwrap();
        server.wait().await.unwrap();
    }
    api.invalidate(&mirror_origin, &account.access_token, &account.client_token)
        .await
        .unwrap();
    assert!(
        api.validate(&mirror_origin, &account.access_token, &account.client_token)
            .await
            .is_err()
    );
    // Change only the external download boundary after publication. Same size,
    // wrong bytes: an installer accepting size alone must fail this scenario.
    assert!(
        http.post(format!("{origin}/fixture/corrupt-optional"))
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
        "authentication":"real-mirror-player-credential",
        "accountPersistence":"token-reloaded-and-validated",
        "logout":"token-invalidated", "authlibInjector":"injected-and-reported-by-client",
        "javaSource":"compass-downloaded-official-runtime",
        "javaComponent":runtime.component, "javaMajor":prepared.java_version,
        "serverJoin":"online-mode-authenticated", "serverUuid":"matches-mirror-profile",
        "serverSkin":"uploaded-slim-signed-texture-and-bytes-match",
        "scope":"native-import-launch-and-online-server-join-not-desktop-click"
    });
    std::fs::write(
        run.join("compass-result.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    println!("{manifest}");
}
