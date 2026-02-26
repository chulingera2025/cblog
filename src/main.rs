use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod admin;
mod build;
mod cbtml;
mod check;
mod config;
mod content;
mod init;
mod lua;
mod media;
mod plugin;
mod state;
mod theme;

#[derive(Parser)]
#[command(name = "cblog", about = "Rust + Lua 博客引擎")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
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

    /// 检查项目完整性
    Check {
        /// 项目根目录（默认当前目录）
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // None 等同于 Serve { root: ".", host: None, port: None }
    let command = cli.command.unwrap_or(Commands::Serve {
        root: PathBuf::from("."),
        host: None,
        port: None,
    });

    // 对于需要加载配置的命令，使用配置中的日志级别作为默认值
    let default_level = match &command {
        Commands::Build { root, .. }
        | Commands::Serve { root, .. }
        | Commands::Check { root, .. } => {
            config::SiteConfig::load(&root.canonicalize().unwrap_or_else(|_| root.clone()))
                .ok()
                .map(|c| c.server.log_level.clone())
        }
    };

    let default_level = default_level.as_deref().unwrap_or("info");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level)),
        )
        .init();

    match command {
        Commands::Build { clean, root } => {
            let root = root.canonicalize()?;
            if init::ensure_initialized(&root)? {
                tracing::info!("已自动初始化项目");
            }
            let site_config = config::SiteConfig::load(&root)?;
            let plugin_configs = plugin::store::load_all_configs_sync(
                &root.join("cblog.db"),
                &site_config.plugins.enabled,
            );
            let theme_saved_config = theme::config::load_theme_config_sync(
                &root.join("cblog.db"),
                &site_config.theme.active,
            );
            let db_posts = build::stages::load::fetch_db_posts_sync(&root.join("cblog.db"));
            let _stats = build::run(&root, &site_config, clean, plugin_configs, theme_saved_config, db_posts)?;
        }
        Commands::Serve { root, host, port } => {
            let root = root.canonicalize()?;
            if init::ensure_initialized(&root)? {
                tracing::info!("已自动初始化项目");
            }
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
        Commands::Check { root } => {
            let root = root.canonicalize()?;
            let result = check::run(&root)?;

            for w in &result.warnings {
                tracing::warn!("{w}");
            }
            for e in &result.errors {
                tracing::error!("{e}");
            }

            if result.errors.is_empty() {
                tracing::info!(
                    "检查通过（{} 个警告）",
                    result.warnings.len()
                );
            } else {
                anyhow::bail!(
                    "检查未通过：{} 个错误，{} 个警告",
                    result.errors.len(),
                    result.warnings.len()
                );
            }
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

    // 启动后台定时清理过期 token
    admin::cleanup::spawn_token_cleanup(app_state.clone());

    let app = admin::router(app_state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("后台管理服务启动：http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
