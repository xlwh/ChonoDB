use chronodb_server::Server;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 告警规则文件路径
    #[arg(long = "alert-rules")]
    alert_rules: Option<String>,
    
    /// 服务器配置文件路径
    #[arg(long = "config")]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");
    
    info!("Starting ChronoDB Server...");
    
    // 解析命令行参数
    let args = Args::parse();
    
    // 创建服务器配置
    let mut config = if let Some(config_path) = args.config {
        chronodb_server::config::ServerConfig::from_file(std::path::Path::new(&config_path))?
    } else {
        chronodb_server::config::ServerConfig::default()
    };
    
    // 添加告警规则文件
    if let Some(alert_rules) = args.alert_rules {
        config.rules.rule_files.push(std::path::PathBuf::from(alert_rules));
    }
    
    // 创建并启动服务器
    let server = Server::with_config(config).await?;
    server.run().await?;
    
    Ok(())
}
