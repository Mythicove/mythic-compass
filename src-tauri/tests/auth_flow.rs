//! 令牌轮换的行为测试。
//!
//! 用一个假的 mirror 而不是 mock 对象：`rotate_if_due` 的全部行为就在"发不发请求"
//! 与"拿到响应之后往磁盘写了什么"这两件事上，而它们要一起验才有意义——
//! 单独 mock 掉 HTTP 的话，测到的只是我对 mock 的期望。
//!
//! 为什么这件事值得单独测：mirror 的 `/authserver/refresh` **只能轮换尚未过期的令牌**
//! （它内部先 validate），所以轮换必须提前主动做。而轮换成功的瞬间旧令牌就作废了
//! （refresh 销毁原会话），于是"新令牌有没有落盘""时间戳有没有跟着一起落盘"
//! 都是会静默坏掉、且症状离原因很远的地方。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use compass_lib::account::{self, Account, AccountKind};
use compass_lib::api::{ApiClient, http_client};
use compass_lib::paths::Paths;
use compass_lib::session::{TOKEN_ROTATE_AFTER_SECS, needs_rotation, rotate_if_due};

/// 假 mirror：只实现 `/authserver/refresh`，并记下被调用了几次。
struct FakeMirror {
    base: String,
    calls: Arc<AtomicUsize>,
}

impl FakeMirror {
    /// `succeed`：true 时返回一个新令牌，false 时按 mirror 的样子回 403（模拟站点故障
    /// 与令牌已死两种情况——对调用方来说都是"换不动"）。
    async fn start(succeed: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let counter = Arc::clone(&counter);
                tokio::spawn(async move {
                    let _ = handle(stream, succeed, counter).await;
                });
            }
        });
        Self { base, calls }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

async fn handle(
    mut stream: TcpStream,
    succeed: bool,
    calls: Arc<AtomicUsize>,
) -> std::io::Result<()> {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let head = String::from_utf8_lossy(&buf[..n]).to_string();
    calls.fetch_add(1, Ordering::SeqCst);

    // 不是 refresh 的请求一律 404：如果轮换之外的代码路径也打到这里，测试要能看出来
    let (status, body) = if head.starts_with("POST /authserver/refresh") && succeed {
        (
            "200 OK",
            r#"{"accessToken":"rotated-access","clientToken":"same-client"}"#.to_string(),
        )
    } else if head.starts_with("POST /authserver/refresh") {
        (
            "403 Forbidden",
            r#"{"error":"ForbiddenOperationException","errorMessage":"Invalid token."}"#
                .to_string(),
        )
    } else {
        ("404 Not Found", r#"{"error":"not found"}"#.to_string())
    };
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await
}

fn paths() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let p = Paths::new(dir.path().join("data"));
    p.ensure().unwrap();
    (dir, p)
}

fn api() -> ApiClient {
    ApiClient::new(http_client().unwrap())
}

/// 一个已登录的站点账号，令牌取得时刻由调用方给（用来造"到期/未到期"）。
fn signed_in(base: &str, obtained_at: u64) -> Account {
    Account {
        player_name: "Alice".into(),
        player_uuid: "uuid-1".into(),
        access_token: "stale-access".into(),
        client_token: "same-client".into(),
        api_url: base.into(),
        signed_in_at: 1_700_000_000,
        token_obtained_at: obtained_at,
        kind: AccountKind::Yggdrasil,
    }
}

/// 未到阈值：一个请求都不该发。
/// 这条盯的是"每次检查都轮换一次"——那会给站点添一倍无谓流量，
/// 而且每次轮换都销毁再重建会话，把"撤销某台设备"这件事变得难以对账。
#[tokio::test]
async fn a_token_within_its_lifetime_is_left_alone() {
    let mirror = FakeMirror::start(true).await;
    let (_d, p) = paths();
    let a = signed_in(&mirror.base, account::now_unix());
    account::save(&p, &a).unwrap();

    let got = rotate_if_due(&api(), &p, a.clone()).await;

    assert_eq!(mirror.call_count(), 0, "未到阈值不该发请求");
    assert_eq!(got.access_token, "stale-access");
    assert_eq!(got.token_obtained_at, a.token_obtained_at);
}

/// 到期：换新令牌，且**内存与磁盘都换**。
#[tokio::test]
async fn a_token_past_the_threshold_is_rotated_in_memory_and_on_disk() {
    let mirror = FakeMirror::start(true).await;
    let (_d, p) = paths();
    let a = signed_in(
        &mirror.base,
        account::now_unix() - TOKEN_ROTATE_AFTER_SECS - 1,
    );
    account::save(&p, &a).unwrap();
    let id = a.id();

    let got = rotate_if_due(&api(), &p, a).await;

    assert_eq!(mirror.call_count(), 1);
    assert_eq!(got.access_token, "rotated-access");
    assert_eq!(got.client_token, "same-client");
    // 关键不变量：换完不该又立刻到期（"新令牌 + 旧时间戳"就是这个断言会抓住的 bug）
    assert!(
        !needs_rotation(got.token_obtained_at, account::now_unix()),
        "刚轮换完就又判定为该轮换：obtained_at={}",
        got.token_obtained_at
    );

    // 磁盘上也必须是新令牌：mirror 的 refresh 已经销毁了原会话，
    // 把作废的旧令牌留在盘上，等于给下次启动埋一个"刚用过就要求重新登录"
    let text = std::fs::read_to_string(p.secrets_file()).unwrap();
    assert!(text.contains("rotated-access"), "新令牌没落盘：{text}");
    assert!(
        !text.contains("stale-access"),
        "作废的旧令牌还在盘上：{text}"
    );

    // 轮换换的是令牌，不是这个人：身份与账号 id 都不该动
    assert_eq!(got.player_name, "Alice");
    assert_eq!(got.player_uuid, "uuid-1");
    assert_eq!(got.signed_in_at, 1_700_000_000, "轮换不该改写登录时间");
    assert_eq!(
        got.id(),
        id,
        "轮换不该换账号 id（否则机密会挂到另一个键上）"
    );

    // 从磁盘重新读回来也是一致的
    let reloaded = account::active(&p).unwrap();
    assert_eq!(reloaded.access_token, "rotated-access");
    assert_eq!(reloaded.token_obtained_at, got.token_obtained_at);
}

/// 换不动（站点故障、令牌已死）：保留现有令牌，不报错，也不把磁盘弄坏。
///
/// 这条是"静默重登不能变成静默登出"：validate 刚通过、或正要离线启动，
/// 一次网络抖动不该把一个本来能用的账号变成"需要重新登录"。
#[tokio::test]
async fn a_failing_refresh_keeps_the_token_that_is_still_in_hand() {
    let mirror = FakeMirror::start(false).await;
    let (_d, p) = paths();
    let a = signed_in(
        &mirror.base,
        account::now_unix() - TOKEN_ROTATE_AFTER_SECS - 1,
    );
    account::save(&p, &a).unwrap();

    let got = rotate_if_due(&api(), &p, a.clone()).await;

    assert_eq!(mirror.call_count(), 1, "确实尝试过轮换");
    assert_eq!(got.access_token, "stale-access", "换不动就该用现有的");
    assert_eq!(
        got.token_obtained_at, a.token_obtained_at,
        "失败不该更新时间戳（否则一个到期令牌会被记成新的，于是一直不再尝试）"
    );
    let text = std::fs::read_to_string(p.secrets_file()).unwrap();
    assert!(
        text.contains("stale-access"),
        "磁盘上的令牌被弄坏了：{text}"
    );
}

/// 离线账号永不触网：它没有站点也没有令牌，拿空 api_url 去 refresh 必然失败，
/// 而那不该被解读成"令牌过期"。
#[tokio::test]
async fn offline_accounts_never_attempt_a_refresh() {
    let mirror = FakeMirror::start(true).await;
    let (_d, p) = paths();
    account::add_offline(&p, "Steve").unwrap();
    let a = account::active(&p).unwrap();
    assert_eq!(a.kind, AccountKind::Offline);
    assert!(a.access_token.is_empty());

    // api_url 指向假 mirror：如果它真的发了请求，call_count 会暴露出来
    let mut pointed = a.clone();
    pointed.api_url = mirror.base.clone();
    let got = rotate_if_due(&api(), &p, pointed).await;

    assert_eq!(mirror.call_count(), 0, "离线账号不该发任何请求");
    assert_eq!(got.access_token, "", "离线账号不该凭空多出一个令牌");
    assert_eq!(got.token_obtained_at, 0);
}

/// 账号还在但机密取不到（secrets.json 被删/损坏）：不该去 refresh，
/// 也不该把这个残态报成"令牌过期"——那是"需要重新登录"，不是"登录已失效"。
#[tokio::test]
async fn an_account_whose_secrets_are_missing_is_not_treated_as_expired() {
    let mirror = FakeMirror::start(true).await;
    let (_d, p) = paths();
    // obtained_at = 0 是"来源不明"，一律判定为该轮换；但没有令牌就换不了
    let a = signed_in(&mirror.base, 0);
    let without_token = Account {
        access_token: String::new(),
        client_token: String::new(),
        ..a
    };

    let got = rotate_if_due(&api(), &p, without_token).await;

    assert_eq!(mirror.call_count(), 0, "没有令牌就不该发请求");
    assert_eq!(got.access_token, "");
}

/// `obtained_at = 0`（旧账号文件迁移过来的）确实要尝试轮换：
/// 宁可多发一次请求，也不要拿着一个不知道还剩多久寿命的令牌去进服。
#[tokio::test]
async fn a_token_of_unknown_age_is_rotated() {
    let mirror = FakeMirror::start(true).await;
    let (_d, p) = paths();
    let a = signed_in(&mirror.base, 0);
    account::save(&p, &a).unwrap();

    let got = rotate_if_due(&api(), &p, a).await;

    assert_eq!(mirror.call_count(), 1);
    assert_eq!(got.access_token, "rotated-access");
    assert_ne!(got.token_obtained_at, 0, "轮换之后就该有确切的时刻了");
}
