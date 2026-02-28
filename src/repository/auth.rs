use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AuthRepository {
    db: SqlitePool,
}

impl AuthRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// 根据用户名查询用户的 (id, password_hash)
    pub async fn find_user_by_username(&self, username: &str) -> Result<Option<(String, String)>> {
        let row = sqlx::query_as::<_, (String, String)>(
            "SELECT id, password_hash FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.db)
        .await?;
        Ok(row)
    }

    /// 更新用户最后登录时间
    pub async fn update_last_login(&self, user_id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
            .bind(&now)
            .bind(user_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// 撤销 token（加入黑名单）
    pub async fn revoke_token(&self, jti: &str, expires_at: &str) -> Result<()> {
        sqlx::query("INSERT OR IGNORE INTO revoked_tokens (jti, expires_at) VALUES (?, ?)")
            .bind(jti)
            .bind(expires_at)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// 检查 token 是否已被撤销
    pub async fn is_token_revoked(&self, jti: &str) -> bool {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM revoked_tokens WHERE jti = ?)",
        )
        .bind(jti)
        .fetch_one(&self.db)
        .await
        .unwrap_or(true)
    }

    /// 查询用户密码哈希
    pub async fn get_password_hash(&self, user_id: &str) -> Result<Option<String>> {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT password_hash FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|(hash,)| hash))
    }

    /// 更新用户密码
    pub async fn update_password(&self, user_id: &str, password_hash: &str) -> Result<()> {
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(password_hash)
            .bind(user_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// 创建用户
    pub async fn create_user(
        &self,
        id: &str,
        username: &str,
        password_hash: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(username)
        .bind(password_hash)
        .bind(&now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// 检查是否有用户存在（安装状态）
    pub async fn has_users(&self) -> bool {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.db)
            .await
            .unwrap_or((0,));
        count.0 > 0
    }

    /// 清理过期的 revoked tokens
    pub async fn cleanup_expired_tokens(&self) -> Result<u64> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query("DELETE FROM revoked_tokens WHERE expires_at < ?")
            .bind(&now)
            .execute(&self.db)
            .await?;
        Ok(result.rows_affected())
    }
}
