//! Metadata GETs must recover transient transport failures without replaying writes.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use compass_lib::api::{ApiClient, http_client};
use compass_lib::error::Error;
use compass_lib::meta::{ManifestEntry, MetaClient};
use compass_lib::net::{Downloader, FileTask};
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

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
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
    let summary = Downloader::new(http_client().unwrap(), 1, "")
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
