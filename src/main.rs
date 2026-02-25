use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod admin;
mod build;
mod cbtml;
mod config;
mod content;
mod lua;
mod media;
mod plugin;
mod state;
mod theme;

#[derive(Parser)]
#[command(name = "cblog", about = "Rust + Lua 博客引擎")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 构建静态站点
    Build {
        /// 清除缓存后全量重建
        #[arg(long)]
        clean: bool,

        /// 项目根目录（默认当前目录）
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
    },

    /// 启动后台管理服务
    Serve {
        /// 项目根目录（默认当前目录）
        #[arg(short, long, default_value = ".")]
        root: PathBuf,

        /// 监听地址
        #[arg(long)]
        host: Option<String>,

        /// 监听端口
        #[arg(long)]
        port: Option<u16>,
    },

    /// 初始化新项目
    Init {
        /// 项目目录名
        name: String,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Build { clean, root } => {
            let root = root.canonicalize()?;
            let site_config = config::SiteConfig::load(&root)?;
            build::run(&root, &site_config, clean)?;
        }
        Commands::Serve { root, host, port } => {
            let root = root.canonicalize()?;
            let site_config = config::SiteConfig::load(&root)?;

            let host = host.unwrap_or_else(|| site_config.server.host.clone());
            let port = port.unwrap_or(site_config.server.port);

            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(async move {
                    run_server(root, site_config, &host, port).await
                })?;
        }
        Commands::Init { name } => {
            // TODO!!! 实现 init 命令：创建项目目录、默认 cblog.toml、示例主题和内容
            tracing::info!("初始化项目：{}", name);
            anyhow::bail!("init 命令尚未实现");
        }
    }

    Ok(())
}

async fn run_server(
    root: PathBuf,
    site_config: config::SiteConfig,
    host: &str,
    port: u16,
) -> anyhow::Result<()> {
    let app_state = state::AppState::new(root.clone(), site_config).await?;

    // 首次启动时创建默认管理员
    ensure_default_admin(&app_state).await?;

    let app = admin::router(app_state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("后台管理服务启动：http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

/// 如果 users 表为空，创建默认管理员账号 admin/admin
async fn ensure_default_admin(state: &state::AppState) -> anyhow::Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?;

    if count.0 == 0 {
        let id = ulid::Ulid::new().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // argon2 哈希默认密码
        use argon2::PasswordHasher;
        let salt = argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = argon2::Argon2::default()
            .hash_password(b"admin", &salt)
            .map_err(|e| anyhow::anyhow!("密码哈希失败：{}", e))?
            .to_string();

        sqlx::query("INSERT INTO users (id, username, password_hash, created_at) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind("admin")
            .bind(&hash)
            .bind(&now)
            .execute(&state.db)
            .await?;

        tracing::warn!("已创建默认管理员账号 admin/admin，请尽快修改密码！");
    }

    Ok(())
}
