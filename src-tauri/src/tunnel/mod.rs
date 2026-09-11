//! 隧道领域逻辑。
//!
//! 分层：`provider` 定义引擎无关类型，`cloudflared` 是当前唯一实现，
//! `registry` 持有进程句柄并提供计数。上层只依赖 `provider` 与 `registry`。

pub mod cloudflared;
pub mod job;
pub mod metrics;
pub mod provider;
pub mod registry;
pub mod site;
