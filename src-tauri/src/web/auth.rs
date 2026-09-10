//! Web 控制台的认证：token 生成 / 存储 / 校验，以及会话与限速。
//!
//! **本模块的每条措施都不是可选的**，理由见 plans/2026-09-10-web-console/02-security.md。
//!
//! 核心前提：Quick Tunnel 的公网 URL **不是密钥**——它会出现在 Cloudflare 边缘日志里，
//! 用户也可能截图或转发。因此 token 必须是真正的认证凭据，
//! 而不是「在随机域名之外再加一道随机数」。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use argon2::password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

/// token 与 session id 的随机字节数。
///
/// 32 字节 = 256 bit，base64url 后 43 个字符。穷举不可行。
const SECRET_BYTES: usize = 32;

/// 会话有效期。手机上不必频繁重登，但也不能无限期。
const SESSION_TTL: Duration = Duration::from_secs(12 * 60 * 60);

/// 单个来源连续失败多少次后锁定。
const MAX_FAILURES_PER_IP: u32 = 5;
/// 锁定时长。
const LOCKOUT: Duration = Duration::from_secs(15 * 60);
/// 全局兜底：任意来源合计，每窗口最多多少次登录尝试。
///
/// 存在的意义：经 cloudflared 转发后真实来源取自 `CF-Connecting-IP`，
/// 而该请求头**可以伪造**。按 IP 限速能被换 IP 绕开，这道闸不依赖任何请求头，
/// 是伪造绕不过去的兜底。
const MAX_GLOBAL_ATTEMPTS: u32 = 30;
const GLOBAL_WINDOW: Duration = Duration::from_secs(60);

/// 生成一个密码学随机的 secret（token 或 session id）。
///
/// **刻意不提供「自定义 token」入口**：用户自选的口令几乎必然弱于随机串，
/// 而「好记」的价值远低于它带来的风险——这个 token 保护的是
/// 「在本机开关任意端口对外暴露」的能力。
pub fn generate_secret() -> Result<String, String> {
    let mut bytes = [0u8; SECRET_BYTES];
    // 直接取操作系统的 CSPRNG。不经用户态 PRNG 中转：
    // 这条路径决定了 token 的全部强度，中间层越少越好。
    getrandom::fill(&mut bytes).map_err(|e| format!("生成随机数失败：{e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// 计算 token 的 Argon2id 哈希（PHC 字符串，自带盐与参数）。
///
/// 存哈希而非明文，是因为 `state.json` 是明文 JSON：
/// 用户可能备份、同步到网盘、贴进 issue 求助。
pub fn hash_token(token: &str) -> Result<String, String> {
    Argon2::default()
        .hash_password(token.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| format!("处理 token 失败：{e}"))
}

/// 校验 token 是否与哈希匹配。
///
/// 走 argon2 的 `verify_password`，其内部比对是常数时间的。
/// **绝不能改成 `==` 比对哈希字符串**——逐字节短路会泄漏前缀匹配长度，
/// 配合可重复请求足以逐位还原。
pub fn verify_token(token: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        // 哈希本身损坏时一律判为不匹配，而不是放行。
        return false;
    };
    Argon2::default()
        .verify_password(token.as_bytes(), &parsed)
        .is_ok()
}

/// 会话与限速的内存状态。
///
/// 刻意**不落盘**：应用重启后需要重新登录是可接受的代价，
/// 换来「重启即全部失效」的确定性。
#[derive(Default)]
pub struct AuthState {
    sessions: HashMap<String, Instant>,
    failures: HashMap<String, Failure>,
    global: Vec<Instant>,
}

struct Failure {
    count: u32,
    locked_until: Option<Instant>,
}

impl AuthState {
    /// 签发会话，返回 session id。
    pub fn create_session(&mut self) -> Result<String, String> {
        let id = generate_secret()?;
        self.sessions.insert(id.clone(), Instant::now() + SESSION_TTL);
        Ok(id)
    }

    /// 会话是否有效；顺带清理过期项。
    pub fn is_valid_session(&mut self, id: &str) -> bool {
        let now = Instant::now();
        self.sessions.retain(|_, expiry| *expiry > now);
        self.sessions.contains_key(id)
    }

    pub fn revoke_session(&mut self, id: &str) {
        self.sessions.remove(id);
    }

    /// 作废全部会话。重新生成 token 时必须调用：
    /// 否则旧设备仍持有有效 cookie，换 token 形同虚设。
    pub fn revoke_all(&mut self) {
        self.sessions.clear();
    }

    /// 本次登录尝试是否被限速拦下。
    ///
    /// 两道闸：按来源锁定 + 全局窗口计数。
    pub fn is_rate_limited(&mut self, source: &str) -> bool {
        let now = Instant::now();

        self.global.retain(|t| now.duration_since(*t) < GLOBAL_WINDOW);
        if self.global.len() as u32 >= MAX_GLOBAL_ATTEMPTS {
            return true;
        }

        match self.failures.get(source) {
            Some(f) => matches!(f.locked_until, Some(until) if until > now),
            None => false,
        }
    }

    /// 登记一次尝试（无论成败），用于全局窗口计数。
    pub fn record_attempt(&mut self) {
        self.global.push(Instant::now());
    }

    pub fn record_failure(&mut self, source: &str) {
        let now = Instant::now();
        let entry = self.failures.entry(source.to_string()).or_insert(Failure {
            count: 0,
            locked_until: None,
        });

        // 锁定期已过则从头计数，不让用户永久背着历史失败次数。
        if matches!(entry.locked_until, Some(until) if until <= now) {
            entry.count = 0;
            entry.locked_until = None;
        }

        entry.count += 1;
        if entry.count >= MAX_FAILURES_PER_IP {
            entry.locked_until = Some(now + LOCKOUT);
        }
    }

    pub fn record_success(&mut self, source: &str) {
        self.failures.remove(source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 生成的_secret_长度足够且每次不同() {
        let a = generate_secret().unwrap();
        let b = generate_secret().unwrap();
        // 32 字节 base64url 无填充 = 43 字符
        assert_eq!(a.len(), 43);
        assert_ne!(a, b, "两次生成不应相同");
    }

    #[test]
    fn 正确的_token_能通过校验() {
        let token = generate_secret().unwrap();
        let phc = hash_token(&token).unwrap();

        assert!(verify_token(&token, &phc));
        assert!(!verify_token("wrong-token", &phc));
    }

    #[test]
    fn 哈希不包含_token_明文() {
        let token = generate_secret().unwrap();
        let phc = hash_token(&token).unwrap();
        // 落盘的是 PHC 串，其中绝不能出现明文
        assert!(!phc.contains(&token));
        assert!(phc.starts_with("$argon2id$"));
    }

    #[test]
    fn 损坏的哈希一律判为不匹配() {
        // 关键行为：解析失败时必须拒绝，而不是放行
        assert!(!verify_token("any", "not-a-phc-string"));
        assert!(!verify_token("any", ""));
    }

    #[test]
    fn 会话可签发校验与作废() {
        let mut auth = AuthState::default();
        let id = auth.create_session().unwrap();

        assert!(auth.is_valid_session(&id));
        assert!(!auth.is_valid_session("不存在的-session"));

        auth.revoke_session(&id);
        assert!(!auth.is_valid_session(&id));
    }

    #[test]
    fn 重新生成_token_会踢掉所有已登录设备() {
        let mut auth = AuthState::default();
        let a = auth.create_session().unwrap();
        let b = auth.create_session().unwrap();

        auth.revoke_all();

        assert!(!auth.is_valid_session(&a));
        assert!(!auth.is_valid_session(&b));
    }

    #[test]
    fn 连续失败会锁定该来源() {
        let mut auth = AuthState::default();

        for _ in 0..MAX_FAILURES_PER_IP {
            assert!(!auth.is_rate_limited("1.2.3.4"));
            auth.record_failure("1.2.3.4");
        }

        assert!(auth.is_rate_limited("1.2.3.4"), "达到阈值后应锁定");
        // 其他来源不受牵连
        assert!(!auth.is_rate_limited("5.6.7.8"));
    }

    #[test]
    fn 成功登录会清掉失败计数() {
        let mut auth = AuthState::default();
        auth.record_failure("1.2.3.4");
        auth.record_failure("1.2.3.4");
        auth.record_success("1.2.3.4");

        for _ in 0..MAX_FAILURES_PER_IP - 1 {
            auth.record_failure("1.2.3.4");
        }
        assert!(!auth.is_rate_limited("1.2.3.4"), "计数应已被重置");
    }

    #[test]
    fn 全局兜底能拦住伪造来源的枚举() {
        let mut auth = AuthState::default();

        // 每次换一个「IP」，按来源的锁定完全绕开
        for i in 0..MAX_GLOBAL_ATTEMPTS {
            let ip = format!("10.0.0.{i}");
            assert!(!auth.is_rate_limited(&ip));
            auth.record_attempt();
            auth.record_failure(&ip);
        }

        // 但全局窗口拦得住——这正是它存在的意义
        assert!(
            auth.is_rate_limited("10.0.0.254"),
            "换 IP 也应被全局兜底拦下"
        );
    }
}
