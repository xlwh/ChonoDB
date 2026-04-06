use chronodb_storage::config::StorageConfig;
use chronodb_storage::memstore::MemStore;
use chronodb_storage::model::{Label, Sample};
use std::path::PathBuf;
use std::sync::Arc;
use axum::serve;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use clap::{Parser, Subcommand};

mod api;
mod tools;

#[derive(Parser)]
#[command(name = "chronodb")]
#[command(about = "ChronoDB - High-performance time-series database")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Data directory
    #[arg(short, long, default_value = "/var/lib/chronodb")]
    data_dir: PathBuf,
    
    /// Listen address
    #[arg(short, long, default_value = "0.0.0.0:9090")]
    listen: String,
    
    /// Configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server
    Server {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Listen address
        #[arg(short, long)]
        listen: Option<String>,
    },
    /// Check data integrity
    Check {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
    },
    /// Compact data
    Compact {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
    },
    /// Backup data
    Backup {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Backup directory
        #[arg(short, long)]
        backup_dir: PathBuf,
    },
    /// Restore data
    Restore {
        /// Backup directory
        #[arg(short, long)]
        backup_dir: PathBuf,
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
    },
    /// Clean up old data
    Cleanup {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Retention days
        #[arg(short, long, default_value = "30")]
        retention_days: u64,
    },
    /// Export data
    Export {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Import data
    Import {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Input file
        #[arg(short, long)]
        input: PathBuf,
    },
    /// Migrate data from Prometheus
    Migrate {
        /// Prometheus data directory
        #[arg(short, long)]
        prometheus_dir: PathBuf,
        /// ChronoDB data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Start time (Unix timestamp)
        #[arg(short, long)]
        start_time: Option<i64>,
        /// End time (Unix timestamp)
        #[arg(short, long)]
        end_time: Option<i64>,
    },
    /// Verify data integrity
    Verify {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Verify mode: full | quick
        #[arg(short, long, default_value = "quick")]
        mode: String,
    },
    /// Benchmark performance
    Bench {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Benchmark duration in seconds
        #[arg(short, long, default_value = "60")]
        duration: u64,
        /// Number of concurrent workers
        #[arg(short, long, default_value = "10")]
        workers: usize,
        /// Output format: text | json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Debug, Clone)]
struct Config {
    data_dir: PathBuf,
    listen_address: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("/var/lib/chronodb"),
            listen_address: "0.0.0.0:9090".to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Some(Commands::Server { data_dir, listen }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            let listen = listen.unwrap_or(cli.listen);
            run_server(data_dir, listen).await
        }
        Some(Commands::Check { data_dir }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::MaintenanceTools::check(data_dir.to_str().unwrap())
                .map(|_| ())
        }
        Some(Commands::Compact { data_dir }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::MaintenanceTools::compact(data_dir.to_str().unwrap())
        }
        Some(Commands::Backup { data_dir, backup_dir }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::MaintenanceTools::backup(
                data_dir.to_str().unwrap(),
                backup_dir.to_str().unwrap()
            )
        }
        Some(Commands::Restore { backup_dir, data_dir }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::MaintenanceTools::restore(
                backup_dir.to_str().unwrap(),
                data_dir.to_str().unwrap()
            )
        }
        Some(Commands::Cleanup { data_dir, retention_days }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::MaintenanceTools::cleanup(
                data_dir.to_str().unwrap(),
                retention_days
            )
        }
        Some(Commands::Export { data_dir, output }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::MaintenanceTools::export(
                data_dir.to_str().unwrap(),
                output.to_str().unwrap()
            )
        }
        Some(Commands::Import { data_dir, input }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::MaintenanceTools::import(
                data_dir.to_str().unwrap(),
                input.to_str().unwrap()
            )
        }
        Some(Commands::Migrate { prometheus_dir, data_dir, start_time, end_time }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::MigrationTool::migrate(
                prometheus_dir.to_str().unwrap(),
                data_dir.to_str().unwrap(),
                start_time,
                end_time
            )
        }
        Some(Commands::Verify { data_dir, mode }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::VerificationTool::verify(
                data_dir.to_str().unwrap(),
                &mode
            )?;
            Ok(())
        }
        Some(Commands::Bench { data_dir, duration, workers, format }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            tools::BenchmarkTool::run(
                data_dir.to_str().unwrap(),
                duration,
                workers,
                &format
            )
        }
        None => {
            // Default to server mode
            run_server(cli.data_dir, cli.listen).await
        }
    }
}

async fn run_server(data_dir: PathBuf, listen_address: String) -> anyhow::Result<()> {
    tracing::info!("Starting ChronoDB server...");
    tracing::info!("Data directory: {:?}", data_dir);
    tracing::info!("Listen address: {}", listen_address);
    
    let storage_config = StorageConfig {
        data_dir: data_dir.to_string_lossy().to_string(),
        ..Default::default()
    };
    
    let store = Arc::new(MemStore::new(storage_config)?);
    
    // Write some test data
    let labels = vec![
        Label::new("__name__", "http_requests_total"),
        Label::new("job", "prometheus"),
        Label::new("instance", "localhost:9090"),
    ];
    
    let samples = vec![
        Sample::new(1000, 100.0),
        Sample::new(2000, 150.0),
        Sample::new(3000, 200.0),
    ];
    
    store.write(labels.clone(), samples)?;
    
    tracing::info!("Wrote test data to store");
    
    let results = store.query(
        &[("job".to_string(), "prometheus".to_string())],
        0,
        10000,
    )?;
    
    tracing::info!("Query returned {} series", results.len());
    
    for ts in &results {
        tracing::info!(
            "Series: {} samples, labels: {:?}",
            ts.samples.len(),
            ts.labels
        );
    }
    
    let stats = store.stats();
    tracing::info!("Stats: {:?}", stats);
    
    // Create API router
    let router = api::create_router(store.clone())
        .layer(CorsLayer::permissive());
    
    // Start HTTP server
    let listener = TcpListener::bind(&listen_address).await?;
    tracing::info!("ChronoDB started successfully on {}", listen_address);
    
    // Wait for ctrl-c signal
    tokio::select! {
        _ = serve(listener, router) => {},
        _ = tokio::signal::ctrl_c() => {},
    }
    
    tracing::info!("Shutting down ChronoDB...");
    store.close()?;
    
    Ok(())
}
