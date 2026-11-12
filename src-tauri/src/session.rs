//! 会话令牌的生命周期：**什么时候该换、换不动怎么办、换了之后谁必须跟着变**。
//!
//! 单独一个模块是因为这三件事各有归属，塞进任何一个现有模块都会破掉它自己写下的不变量：
//! - `account.rs` 的头注释承诺"落盘收敛到唯一一处，换钥匙串只改那三个函数"——
//!   往里加联网逻辑，那条承诺就不再成立。
//! - `commands.rs` 的不变量是"只有命令"（`ipc_contract` 门就是按 `pub async fn` 数命令的）——
//!   一个 `pub` 的辅助函数放这儿会被当成命令，而用 `pub(crate)` 躲开等于拆门。
//!
//! 存储仍在 `account.rs`（`Secrets.obtained_at` / `rotate_secrets`），策略与编排在这里。

use crate::account::{self, Account, AccountKind, now_unix};
use crate::api::ApiClient;
use crate::paths::Paths;

/// 令牌超过这个年龄就轮换。
///
/// mirror 的会话 TTL 是 24 小时，取它的一半：留出足够余量给"启动器一直开着不重启"
/// 的情况，又不至于每次检查都发一次请求。TTL 是服务端的属性，启动器无从查询，
/// 所以这里是一个**必须与服务端保持一致的假设**——服务端改了 TTL，这个常量要跟着看。
pub const TOKEN_ROTATE_AFTER_SECS: u64 = 12 * 3600;

/// 令牌是否该轮换了。纯函数（`now` 由调用方给），所以边界可以离线测。
///
/// 为什么必须提前：mirror 的 `/authserver/refresh` 内部先 validate，**过期的令牌救不回来**。
/// 等 validate 报 401 再想补救已经晚了，那时唯一能做的只剩要求用户重新输入独立密码。
///
/// `obtained_at == 0` 视为来源不明（旧账号文件迁移过来的），一律轮换：宁可多发一次请求，
/// 也不要拿着一个不知道还剩多久寿命的令牌去进服。
pub fn needs_rotation(obtained_at: u64, now: u64) -> bool {
    obtained_at == 0 || now.saturating_sub(obtained_at) >= TOKEN_ROTATE_AFTER_SECS
}

/// 令牌过半就轮换，返回手上的账号（可能已轮换）。**永不失败**：调用方之一在启动路径上，
/// 轮换是优化，不该成为拦路的东西。
pub async fn rotate_if_due(api: &ApiClient, paths: &Paths, a: Account) -> Account {
    // 离线账号没有站点也没有令牌；空令牌是"账号还在但机密取不到"的残态，
    // 拿它去 refresh 只会换来一个 401，而那不该被解读成"令牌过期"
    if a.kind == AccountKind::Offline
        || a.access_token.is_empty()
        || !needs_rotation(a.token_obtained_at, now_unix())
    {
        return a;
    }
    let Ok(resp) = api
        .refresh(&a.api_url, &a.access_token, &a.client_token)
        .await
    else {
        // 换不动就用现有的：validate 刚通过（或正要离线启动），一次网络抖动
        // 不该把一个本来能用的账号变成"需要重新登录"。下次检查会再试。
        return a;
    };
    let obtained_at = now_unix();
    let next = Account {
        access_token: resp.access_token,
        client_token: resp.client_token,
        token_obtained_at: obtained_at,
        ..a.clone()
    };
    // 落盘失败也照样把新令牌交出去。mirror 的 refresh 已经**销毁了原会话**，
    // 所以旧令牌从这一刻起是废的：交出去只会让本次启动失败在一个来自服务器的
    // "无效会话"上，看不出根因。用新令牌的话本次能玩，下次启动读到磁盘上那个
    // 作废的旧令牌，会走 auth_check 的 expired 分支——那条路是会向用户解释的。
    let _ = account::rotate_secrets(
        paths,
        &next.id(),
        &next.access_token,
        &next.client_token,
        obtained_at,
    );
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 阈值本身是多少不重要，重要的是三条边界：0（来源不明）一律轮换、阈值内不换、
    /// 阈值外换。写死数值而不是从常量算出来，是因为算出来的断言只会把实现里的
    /// 同一个式子再抄一遍——那种测试改实现时跟着改，永远不会红。
    #[test]
    fn rotation_is_due_only_past_the_threshold_and_always_for_an_unknown_age() {
        let base = 1_700_000_000u64;
        assert!(needs_rotation(0, base), "来源不明的令牌必须轮换");
        assert!(!needs_rotation(base, base), "刚拿到的令牌不该立刻轮换");
        assert!(
            !needs_rotation(base, base + TOKEN_ROTATE_AFTER_SECS - 1),
            "阈值内不该轮换（否则每次检查都发一次请求）"
        );
        assert!(
            needs_rotation(base, base + TOKEN_ROTATE_AFTER_SECS),
            "到阈值就该轮换"
        );
        // 时钟回拨（改系统时间、夏令时、虚拟机迁移）不该让 saturating_sub 下溢成
        // 一个巨大的年龄，于是每次检查都轮换
        assert!(
            !needs_rotation(base + 3600, base),
            "now 早于 obtained_at 时不该轮换"
        );
    }
}
