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
#[command(name = "cblog", about = "Rust + Lua 博客引擎", version = long_version())]
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

        /// 强制全量重建（不清除缓存）
        #[arg(long)]
        force: bool,

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
        Commands::Build { clean, force, root } => {
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
            let site_settings = admin::settings::SiteSettings::load_sync(&root.join("cblog.db"));
            let _stats = build::run(&root, &site_config, build::BuildParams {
                clean,
                force,
                plugin_configs,
                theme_saved_config,
                db_posts,
                site_settings,
            })?;
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
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if let Some(info) = detect_port_process(port) {
                tracing::error!("端口 {port} 已被占用：{info}");
            } else {
                tracing::error!("端口 {port} 已被占用");
            }
            return Err(e.into());
        }
        Err(e) => return Err(e.into()),
    };
    tracing::info!("后台管理服务启动：http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

/// 通过 /proc 检测占用指定端口的进程信息（仅 Linux）
fn detect_port_process(port: u16) -> Option<String> {
    use std::fs;

    let port_hex = format!("{:04X}", port);

    // 遍历 /proc/net/tcp 和 tcp6 查找本地监听端口
    for net_file in &["/proc/net/tcp", "/proc/net/tcp6"] {
        let content = fs::read_to_string(net_file).ok()?;
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 4 {
                continue;
            }
            // fields[1] = local_address (hex_ip:hex_port), fields[3] = state (0A = LISTEN)
            if fields[3] != "0A" {
                continue;
            }
            if let Some(lport) = fields[1].rsplit(':').next()
                && lport == port_hex
                && let Some(inode) = fields.get(9)
            {
                return find_pid_by_inode(inode);
            }
        }
    }
    None
}

fn find_pid_by_inode(target_inode: &str) -> Option<String> {
    use std::fs;

    let socket_pattern = format!("socket:[{target_inode}]");
    for entry in fs::read_dir("/proc").ok()? {
        let entry = entry.ok()?;
        let pid_str = entry.file_name().to_string_lossy().to_string();
        if !pid_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let fd_dir = entry.path().join("fd");
        if let Ok(fds) = fs::read_dir(&fd_dir) {
            for fd in fds.flatten() {
                if let Ok(link) = fs::read_link(fd.path())
                    && link.to_string_lossy() == socket_pattern
                {
                    let comm = fs::read_to_string(entry.path().join("comm"))
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    return Some(format!("PID {pid_str} ({comm})"));
                }
            }
        }
    }
    None
}

const fn long_version() -> &'static str {
    concat!(
        env!("CARGO_PKG_VERSION"),
        "\ncommit:  ",
        env!("CBLOG_GIT_COMMIT"),
        "\nbuild:   ",
        env!("CBLOG_BUILD_TIME"),
        "\ntarget:  ",
        env!("CBLOG_BUILD_TARGET"),
        "\nprofile: ",
        env!("CBLOG_BUILD_PROFILE"),
    )
}
