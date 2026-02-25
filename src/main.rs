use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod build;
mod cbtml;
mod config;
mod content;

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
        Commands::Init { name } => {
            // TODO!!! 实现 init 命令：创建项目目录、默认 cblog.toml、示例主题和内容
            tracing::info!("初始化项目：{}", name);
            anyhow::bail!("init 命令尚未实现");
        }
    }

    Ok(())
}
