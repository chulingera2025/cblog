use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Clone, Default, serde::Serialize)]
pub struct SiteSettings {
    pub site_title: String,
    pub site_subtitle: String,
    pub site_url: String,
    pub admin_email: String,
}

impl SiteSettings {
    pub async fn load(db: &SqlitePool) -> Result<Self> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM site_settings")
                .fetch_all(db)
                .await?;

        let mut settings = Self::default();
        for (key, value) in rows {
            match key.as_str() {
                "site_title" => settings.site_title = value,
                "site_subtitle" => settings.site_subtitle = value,
                "site_url" => settings.site_url = value,
                "admin_email" => settings.admin_email = value,
                _ => {}
            }
        }
        Ok(settings)
    }

    pub async fn save(&self, db: &SqlitePool) -> Result<()> {
        let pairs = [
            ("site_title", &self.site_title),
            ("site_subtitle", &self.site_subtitle),
            ("site_url", &self.site_url),
            ("admin_email", &self.admin_email),
        ];

        for (key, value) in pairs {
            sqlx::query(
                "INSERT INTO site_settings (key, value) VALUES (?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(key)
            .bind(value)
            .execute(db)
            .await?;
        }
        Ok(())
    }
}
