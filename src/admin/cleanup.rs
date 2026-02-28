use crate::state::AppState;

/// 启动后台定时任务：每小时清理过期的 revoked_tokens
pub fn spawn_token_cleanup(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            match state.auth.cleanup_expired_tokens().await {
                Ok(rows) => {
                    if rows > 0 {
                        tracing::info!("已清理 {} 条过期 token 记录", rows);
                    }
                }
                Err(e) => {
                    tracing::warn!("清理 revoked_tokens 失败: {e}");
                }
            }
        }
    });
}
