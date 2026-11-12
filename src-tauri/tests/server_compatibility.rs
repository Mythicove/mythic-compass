use compass_lib::api::{ApiClient, http_client};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn resolves_only_one_matching_public_java_destination_and_reads_its_status() {
    for (directory, host, port, expected_path) in [
        (
            r#"[{"id":"public","address":"PLAY.example.org:25565","edition":"java"}]"#,
            "play.example.org",
            25565,
            Some("/api/isles/servers/public/status"),
        ),
        (
            r#"[{"id":"ipv6","address":"[::1]:25565","edition":"java"}]"#,
            "::1",
            25565,
            Some("/api/isles/servers/ipv6/status"),
        ),
        (
            r#"[{"id":"wrong-port","address":"play.example.org:25566","edition":"java"}]"#,
            "play.example.org",
            25565,
            None,
        ),
        (
            r#"[{"id":"bedrock","address":"play.example.org:25565","edition":"bedrock"}]"#,
            "play.example.org",
            25565,
            None,
        ),
        (
            r#"[{"id":"a","address":"play.example.org:25565","edition":"java"},{"id":"b","address":"play.example.org:25565","edition":"java"}]"#,
            "play.example.org",
            25565,
            None,
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let mut paths = Vec::new();
            for body in [directory, r#"{"reachable":true,"version":"Paper 1.21.4","protocolVersions":["1.21-1.21.1","1.21.4"]}"#].into_iter().take(if expected_path.is_some() { 2 } else { 1 }) {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buffer = Vec::new();
                loop {
                    let mut chunk = [0; 1024];
                    let n = stream.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    buffer.extend_from_slice(&chunk[..n]);
                    if buffer.windows(4).any(|v| v == b"\r\n\r\n") { break; }
                }
                let request = String::from_utf8(buffer).unwrap();
                assert!(!request.to_lowercase().contains("authorization:"));
                paths.push(request.split_whitespace().nth(1).unwrap().to_owned());
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
            paths
        });
        let status = ApiClient::new(http_client().unwrap())
            .community_server_status(&base, host, port)
            .await
            .unwrap();
        assert_eq!(status.is_some(), expected_path.is_some());
        if let Some(status) = status {
            assert!(status.reachable);
            assert_eq!(status.version.as_deref(), Some("Paper 1.21.4"));
            assert_eq!(status.protocol_versions, ["1.21-1.21.1", "1.21.4"]);
        }
        let paths = server.await.unwrap();
        assert_eq!(
            paths.first().map(String::as_str),
            Some("/api/isles/servers")
        );
        assert_eq!(paths.get(1).map(String::as_str), expected_path);
    }
}
