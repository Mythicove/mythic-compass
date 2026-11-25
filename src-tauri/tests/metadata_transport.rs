//! Metadata GETs must recover transient transport failures without replaying writes.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use compass_lib::api::{ApiClient, http_client};
use compass_lib::error::Error;
use compass_lib::meta::{ManifestEntry, MetaClient};
use compass_lib::net::{Downloader, FileTask};
use compass_lib::paths::Paths;
use sha1::{Digest, Sha1};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct Server {
    url: String,
    requests: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    async fn start(responses: Vec<String>) -> Self {
        Self::start_with_delay(responses, Duration::ZERO).await
    }

    async fn start_with_delay(responses: Vec<String>, delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mut head = Vec::new();
                while !head.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    if stream.read(&mut byte).await.unwrap_or(0) == 0 {
                        break;
                    }
                    head.push(byte[0]);
                }
                let index = observed.fetch_add(1, Ordering::SeqCst);
                let response = &responses[index.min(responses.len() - 1)];
                tokio::time::sleep(delay).await;
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });
        Self {
            url,
            requests,
            task,
        }
    }

    fn count(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

#[tokio::test]
async fn independent_downloaders_share_one_cache_writer() {
    let body = "shared Minecraft library bytes\n".repeat(8192);
    let server =
        Server::start_with_delay(vec![response(200, &body)], Duration::from_millis(100)).await;
    let directory = tempfile::tempdir().unwrap();
    let paths = Paths::new(directory.path());
    let game = directory.path().join("game");
    let destination = game.join("mods/shared.jar");
    let task = FileTask {
        url: server.url.clone(),
        dest: destination.clone(),
        sha1: hex::encode(Sha1::digest(body.as_bytes())),
        size: body.len() as u64,
    };
    let http = http_client().unwrap();
    let first = Downloader::new(http.clone(), 4, "", &paths);
    let second = Downloader::new(http, 4, "", &paths);
    let (a, b) = tokio::join!(
        first.download_all(vec![task.clone(); 4], None),
        second.download_all(vec![task.clone(); 4], None),
    );
    println!(
        "cache requests={} downloaded={} skipped={} failures={:?}/{:?}",
        server.count(),
        a.downloaded + b.downloaded,
        a.skipped + b.skipped,
        a.failed,
        b.failed
    );
    assert!(a.failed.is_empty() && b.failed.is_empty());
    assert_eq!(
        server.count(),
        1,
        "one cache destination must have one writer"
    );
    assert_eq!(a.downloaded + b.downloaded, 1);
    assert_eq!(a.skipped + b.skipped, 7);
    assert_eq!(
        tokio::fs::read(&destination).await.unwrap(),
        body.as_bytes()
    );
    assert!(!task.part_path().exists());
    assert_eq!(std::fs::read_dir(game.join("mods")).unwrap().count(), 1);
    assert_eq!(
        compass_lib::content::scan(&game, compass_lib::content::Kind::Mods).len(),
        1
    );
    let export = tempfile::tempdir().unwrap();
    assert_eq!(
        compass_lib::archive::zip_dir(&game, &export.path().join("instance.zip"), &[])
            .unwrap()
            .files,
        1
    );
}

#[tokio::test]
async fn cancelled_download_releases_cache_ownership() {
    let body = "library bytes after cancellation";
    let server =
        Server::start_with_delay(vec![response(200, body)], Duration::from_millis(100)).await;
    let directory = tempfile::tempdir().unwrap();
    let task = FileTask {
        url: server.url.clone(),
        dest: directory.path().join("library.jar"),
        sha1: hex::encode(Sha1::digest(body.as_bytes())),
        size: body.len() as u64,
    };
    let http = http_client().unwrap();
    let paths = Paths::new(directory.path());
    let first = Downloader::new(http.clone(), 1, "", &paths);
    let first_task = task.clone();
    let running = tokio::spawn(async move { first.download_all(vec![first_task], None).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        while server.count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    running.abort();
    assert!(running.await.unwrap_err().is_cancelled());
    let next = Downloader::new(http, 1, "", &paths);
    let summary = tokio::time::timeout(
        Duration::from_secs(5),
        next.download_all(vec![task.clone()], None),
    )
    .await
    .expect("cancelled owner retained the cache lock");
    assert!(summary.failed.is_empty(), "{:?}", summary.failed);
    assert_eq!(tokio::fs::read(&task.dest).await.unwrap(), body.as_bytes());
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn unknown_size_restart_reports_the_full_response_not_the_old_partial() {
    let body = include_bytes!("fixtures/1.21.4.json");
    let server = Server::start(vec![response(200, std::str::from_utf8(body).unwrap())]).await;
    let directory = tempfile::tempdir().unwrap();
    let task = FileTask {
        url: server.url.clone(),
        dest: directory.path().join("version.json"),
        sha1: hex::encode(Sha1::digest(body)),
        size: 0,
    };
    tokio::fs::write(task.part_path(), &body[..128])
        .await
        .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let summary = Downloader::new(http_client().unwrap(), 1, "", &Paths::new(directory.path()))
        .download_all(vec![task.clone()], Some(tx))
        .await;
    assert!(summary.failed.is_empty(), "{:?}", summary.failed);
    assert_eq!(tokio::fs::read(task.dest).await.unwrap(), body);
    let mut last = None;
    while let Ok(progress) = rx.try_recv() {
        assert_eq!(progress.total_bytes, body.len() as u64);
        last = Some(progress.done_bytes);
    }
    assert_eq!(last, Some(body.len() as u64));
}

#[tokio::test]
async fn range_rejection_does_not_certify_an_unverified_partial() {
    let body = include_str!("fixtures/1.21.4.json");
    let server = Server::start(vec![response(416, ""), response(200, body)]).await;
    let directory = tempfile::tempdir().unwrap();
    let task = FileTask {
        url: server.url.clone(),
        dest: directory.path().join("version.json"),
        sha1: String::new(),
        size: 0,
    };
    tokio::fs::write(task.part_path(), &body.as_bytes()[..128])
        .await
        .unwrap();
    let summary = Downloader::new(http_client().unwrap(), 1, "", &Paths::new(directory.path()))
        .download_all(vec![task.clone()], None)
        .await;
    assert!(summary.failed.is_empty(), "{:?}", summary.failed);
    assert_eq!(tokio::fs::read(&task.dest).await.unwrap(), body.as_bytes());
    assert_eq!(server.count(), 2);
    assert!(!task.part_path().exists());
}

#[tokio::test]
async fn interrupted_body_resumes_without_promoting_partial_bytes() {
    let body = include_str!("fixtures/1.21.4.json");
    let offset = 128;
    let server = Server::start(vec![
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(), &body[..offset]
        ),
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {offset}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len() - 1, body.len(), body.len() - offset, &body[offset..]
        ),
    ]).await;
    let directory = tempfile::tempdir().unwrap();
    let task = FileTask {
        url: server.url.clone(),
        dest: directory.path().join("version.json"),
        sha1: hex::encode(Sha1::digest(body.as_bytes())),
        size: body.len() as u64,
    };
    let summary = Downloader::new(http_client().unwrap(), 1, "", &Paths::new(directory.path()))
        .download_all(vec![task.clone()], None)
        .await;
    assert!(summary.failed.is_empty(), "{:?}", summary.failed);
    assert_eq!(tokio::fs::read(&task.dest).await.unwrap(), body.as_bytes());
    assert_eq!(server.count(), 2);
    assert!(!task.part_path().exists());
}

#[tokio::test]
async fn unavailable_origin_keeps_partial_and_later_install_recovers() {
    let body = include_str!("fixtures/1.21.4.json");
    let server = Server::start(vec![response(503, "offline")]).await;
    let directory = tempfile::tempdir().unwrap();
    let mut task = FileTask {
        url: server.url.clone(),
        dest: directory.path().join("version.json"),
        sha1: hex::encode(Sha1::digest(body.as_bytes())),
        size: body.len() as u64,
    };
    tokio::fs::write(task.part_path(), &body.as_bytes()[..128])
        .await
        .unwrap();
    let paths = Paths::new(directory.path());
    let downloader = Downloader::new(http_client().unwrap(), 1, "", &paths);
    let failed = downloader.download_all(vec![task.clone()], None).await;
    assert_eq!(failed.failed.len(), 1);
    assert!(!task.dest.exists());
    assert_eq!(
        tokio::fs::read(task.part_path()).await.unwrap(),
        &body.as_bytes()[..128]
    );
    let recovered = Server::start(vec![response(200, body)]).await;
    task.url = recovered.url.clone();
    let result = downloader.download_all(vec![task.clone()], None).await;
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    assert_eq!(tokio::fs::read(&task.dest).await.unwrap(), body.as_bytes());
    assert!(!task.part_path().exists());
}

#[tokio::test]
async fn blocked_destination_reports_failure_and_recovers_after_unblocking() {
    let body = include_str!("fixtures/1.21.4.json");
    let server = Server::start(vec![response(200, body)]).await;
    let directory = tempfile::tempdir().unwrap();
    let task = FileTask {
        url: server.url.clone(),
        dest: directory.path().join("version.json"),
        sha1: hex::encode(Sha1::digest(body.as_bytes())),
        size: body.len() as u64,
    };
    tokio::fs::create_dir(&task.dest).await.unwrap();
    let downloader = Downloader::new(http_client().unwrap(), 1, "", &Paths::new(directory.path()));
    let failed = downloader.download_all(vec![task.clone()], None).await;
    assert_eq!(failed.failed.len(), 1);
    assert!(task.dest.is_dir());
    tokio::fs::remove_dir(&task.dest).await.unwrap();
    let recovered = downloader.download_all(vec![task.clone()], None).await;
    assert!(recovered.failed.is_empty(), "{:?}", recovered.failed);
    assert_eq!(tokio::fs::read(&task.dest).await.unwrap(), body.as_bytes());
    assert!(!task.part_path().exists());
}

fn response(status: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} Status\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn maven_text_recovers_gateway_error_and_truncated_body() {
    let server = Server::start(vec![
        response(502, "upstream unavailable"),
        "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\npartial".into(),
        response(200, "<metadata><version>54.1.22</version></metadata>"),
    ])
    .await;
    let api = ApiClient::new(http_client().unwrap());
    assert_eq!(
        api.get_text(&server.url).await.unwrap(),
        "<metadata><version>54.1.22</version></metadata>"
    );
    assert_eq!(server.count(), 3);
}

#[tokio::test]
async fn json_and_verified_version_reads_share_transport_recovery() {
    let server = Server::start(vec![
        response(503, "busy"),
        response(200, r#"{"versions":["54.1.22"]}"#),
    ])
    .await;
    let api = ApiClient::new(http_client().unwrap());
    let data: serde_json::Value = api.get_json(&server.url).await.unwrap();
    assert_eq!(data["versions"][0], "54.1.22");
    assert_eq!(server.count(), 2);

    let body = include_str!("fixtures/1.21.4.json");
    let server = Server::start(vec![response(504, "gateway timeout"), response(200, body)]).await;
    let meta = MetaClient::new(http_client().unwrap(), "");
    let mut entry = ManifestEntry {
        id: "1.21.4".into(),
        kind: "release".into(),
        url: server.url.clone(),
        time: String::new(),
        release_time: String::new(),
        sha1: hex::encode(Sha1::digest(body.as_bytes())),
        compliance_level: 1,
    };
    assert_eq!(meta.version_for(&entry).await.unwrap().id, "1.21.4");
    assert_eq!(server.count(), 2);
    entry.sha1 = "0000000000000000000000000000000000000000".into();
    assert!(matches!(
        meta.version_for(&entry).await,
        Err(Error::Checksum(_))
    ));
    assert_eq!(
        server.count(),
        3,
        "checksum failures must not replay metadata reads"
    );
}

#[tokio::test]
async fn permanent_errors_invalid_documents_and_writes_are_not_replayed() {
    for status in [400, 401, 403, 404] {
        let server = Server::start(vec![response(status, "rejected")]).await;
        let api = ApiClient::new(http_client().unwrap());
        assert!(
            matches!(api.get_text(&server.url).await, Err(Error::Api { status: actual, .. }) if actual == status)
        );
        assert_eq!(server.count(), 1);
    }
    let server = Server::start(vec![response(200, "invalid json")]).await;
    let api = ApiClient::new(http_client().unwrap());
    assert!(
        api.get_json::<serde_json::Value>(&server.url)
            .await
            .is_err()
    );
    assert_eq!(server.count(), 1);

    let server = Server::start(vec![response(502, "gateway error")]).await;
    let api = ApiClient::new(http_client().unwrap());
    assert!(api.get_text(&server.url).await.is_err());
    assert_eq!(
        server.count(),
        3,
        "transient errors must exhaust the existing finite budget"
    );

    let server = Server::start(vec![response(502, "gateway error")]).await;
    let api = ApiClient::new(http_client().unwrap());
    assert!(
        api.authenticate(&server.url, "Alice", "test-password")
            .await
            .is_err()
    );
    assert_eq!(server.count(), 1, "authentication must not be replayed");
}

#[tokio::test]
#[ignore = "downloads the real Forge installer (9 MB) from its published profile URL"]
async fn downloads_the_real_forge_installer_from_its_root_alias() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("forge-installer.jar");
    // Independently observed from the official artifact through four HTTP/proxy combinations.
    let task = FileTask {
        url: "https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.4-54.1.22/forge-1.21.4-54.1.22-installer.jar".into(),
        dest: destination.clone(),
        sha1: "c445499201106cd56123029315a8e53228ea677b".into(),
        size: 9_111_901,
    };
    let summary = Downloader::new(http_client().unwrap(), 1, "", &Paths::new(directory.path()))
        .download_all(vec![task], None)
        .await;
    assert!(summary.failed.is_empty(), "{:?}", summary.failed);
    assert_eq!(summary.downloaded, 1);
    let (profile, _) = compass_lib::forge::read_installer(&destination).unwrap();
    assert_eq!(profile.game_version(), Some("1.21.4"));
    println!(
        "Forge installer: verified 9111901 bytes, official SHA-1, and Minecraft 1.21.4 profile"
    );
}
