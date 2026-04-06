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
mod stress_test;

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
        #[command(subcommand)]
        command: MigrateCommands,
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
        #[arg(long)]
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
    /// Run stress test
    Stress {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Test duration in seconds
        #[arg(short, long, default_value = "60")]
        duration: u64,
        /// Number of concurrent write threads
        #[arg(long, default_value = "4")]
        write_threads: usize,
        /// Number of concurrent query threads
        #[arg(long, default_value = "2")]
        query_threads: usize,
        /// Write rate per thread (ops/sec)
        #[arg(long, default_value = "100")]
        write_rate: u64,
        /// Query rate per thread (ops/sec)
        #[arg(long, default_value = "50")]
        query_rate: u64,
        /// Number of labels per series
        #[arg(long, default_value = "3")]
        labels_per_series: usize,
        /// Number of samples per write
        #[arg(long, default_value = "10")]
        samples_per_write: usize,
        /// Number of series to generate
        #[arg(long, default_value = "1000")]
        series_count: usize,
    },
    /// Tiered storage management
    Tiered {
        #[command(subcommand)]
        command: TieredCommands,
    },
    /// Cluster management
    Cluster {
        #[command(subcommand)]
        command: ClusterCommands,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Monitoring and alerts
    Monitoring {
        #[command(subcommand)]
        command: MonitoringCommands,
    },
    /// Data management
    Data {
        #[command(subcommand)]
        command: DataCommands,
    },
}

#[derive(Subcommand)]
enum TieredCommands {
    /// Show tiered storage status
    Status {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
    },
    /// Perform tiered storage migration
    Migrate {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
    },
    /// Configure tiered storage
    Configure {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Tier name (hot, warm, cold, archive)
        #[arg(short, long)]
        tier: String,
        /// Retention period in hours
        #[arg(short, long)]
        retention_hours: u64,
        /// Maximum size in GB
        #[arg(short, long)]
        max_size_gb: u64,
    },
}

#[derive(Subcommand)]
enum ClusterCommands {
    /// Show cluster status
    Status,
    /// List cluster nodes
    ListNodes,
    /// Add cluster node
    AddNode {
        /// Node address (host:port)
        #[arg(short, long)]
        address: String,
    },
    /// Remove cluster node
    RemoveNode {
        /// Node address (host:port)
        #[arg(short, long)]
        address: String,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show {
        /// Configuration file path
        #[arg(short, long)]
        config_file: String,
    },
    /// Validate configuration file
    Validate {
        /// Configuration file path
        #[arg(short, long)]
        config_file: String,
    },
    /// Generate default configuration file
    Generate {
        /// Output file path
        #[arg(short, long)]
        output_file: String,
    },
}

#[derive(Subcommand)]
enum MonitoringCommands {
    /// Show current metrics
    Metrics,
    /// Show alert rules
    Alerts,
    /// Check system health
    Health,
}

#[derive(Subcommand)]
enum DataCommands {
    /// List time series
    ListSeries {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Series pattern
        #[arg(short, long, default_value = "*")]
        pattern: String,
    },
    /// Delete time series
    DeleteSeries {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Series pattern
        #[arg(short, long)]
        pattern: String,
    },
    /// Export time series data
    ExportSeries {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Series pattern
        #[arg(short, long, default_value = "*")]
        pattern: String,
        /// Output file path
        #[arg(short, long)]
        output_file: String,
    },
    /// Import time series data
    ImportSeries {
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
        /// Input file path
        #[arg(short, long)]
        input_file: String,
    },
}

#[derive(Subcommand)]
enum MigrateCommands {
    /// Migrate data from Prometheus
    Prometheus {
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
    /// Migrate data from InfluxDB
    InfluxDB {
        /// InfluxDB URL
        #[arg(short, long)]
        influxdb_url: String,
        /// Database name
        #[arg(short, long)]
        database: String,
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
    /// Migrate data from Graphite
    Graphite {
        /// Graphite data directory
        #[arg(short, long)]
        graphite_dir: PathBuf,
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
    /// Migrate data from OpenTSDB
    OpenTSDB {
        /// OpenTSDB URL
        #[arg(short, long)]
        opentsdb_url: String,
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
        Some(Commands::Migrate { command }) => {
            match command {
                MigrateCommands::Prometheus { prometheus_dir, data_dir, start_time, end_time } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::MigrationTool::migrate(
                        prometheus_dir.to_str().unwrap(),
                        data_dir.to_str().unwrap(),
                        start_time,
                        end_time
                    )
                }
                MigrateCommands::InfluxDB { influxdb_url, database, data_dir, start_time, end_time } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::MigrationTool::migrate_from_influxdb(
                        &influxdb_url,
                        &database,
                        data_dir.to_str().unwrap(),
                        start_time,
                        end_time
                    )
                }
                MigrateCommands::Graphite { graphite_dir, data_dir, start_time, end_time } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::MigrationTool::migrate_from_graphite(
                        graphite_dir.to_str().unwrap(),
                        data_dir.to_str().unwrap(),
                        start_time,
                        end_time
                    )
                }
                MigrateCommands::OpenTSDB { opentsdb_url, data_dir, start_time, end_time } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::MigrationTool::migrate_from_opentsdb(
                        &opentsdb_url,
                        data_dir.to_str().unwrap(),
                        start_time,
                        end_time
                    )
                }
            }
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
        Some(Commands::Stress { data_dir, duration, write_threads, query_threads, write_rate, query_rate, labels_per_series, samples_per_write, series_count }) => {
            let data_dir = data_dir.unwrap_or(cli.data_dir);
            let config = stress_test::StressTestConfig {
                data_dir: data_dir.to_str().unwrap().to_string(),
                duration,
                write_threads,
                query_threads,
                write_rate,
                query_rate,
                labels_per_series,
                samples_per_write,
                series_count,
            };
            let stress_test = stress_test::StressTest::new(config);
            stress_test.run();
            Ok(())
        }
        Some(Commands::Tiered { command }) => {
            match command {
                TieredCommands::Status { data_dir } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::OpsTool::status(data_dir.to_str().unwrap())
                }
                TieredCommands::Migrate { data_dir } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::OpsTool::migrate(data_dir.to_str().unwrap())
                }
                TieredCommands::Configure { data_dir, tier, retention_hours, max_size_gb } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::OpsTool::configure(data_dir.to_str().unwrap(), &tier, retention_hours, max_size_gb)
                }
            }
        }
        Some(Commands::Cluster { command }) => {
            match command {
                ClusterCommands::Status => {
                    tools::ClusterTool::status()
                }
                ClusterCommands::ListNodes => {
                    tools::ClusterTool::list_nodes()
                }
                ClusterCommands::AddNode { address } => {
                    tools::ClusterTool::add_node(&address)
                }
                ClusterCommands::RemoveNode { address } => {
                    tools::ClusterTool::remove_node(&address)
                }
            }
        }
        Some(Commands::Config { command }) => {
            match command {
                ConfigCommands::Show { config_file } => {
                    tools::ConfigTool::show(&config_file)
                }
                ConfigCommands::Validate { config_file } => {
                    tools::ConfigTool::validate(&config_file)
                }
                ConfigCommands::Generate { output_file } => {
                    tools::ConfigTool::generate(&output_file)
                }
            }
        }
        Some(Commands::Monitoring { command }) => {
            match command {
                MonitoringCommands::Metrics => {
                    tools::MonitoringTool::metrics()
                }
                MonitoringCommands::Alerts => {
                    tools::MonitoringTool::alerts()
                }
                MonitoringCommands::Health => {
                    tools::MonitoringTool::health()
                }
            }
        }
        Some(Commands::Data { command }) => {
            match command {
                DataCommands::ListSeries { data_dir, pattern } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::DataTool::list_series(data_dir.to_str().unwrap(), &pattern)
                }
                DataCommands::DeleteSeries { data_dir, pattern } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::DataTool::delete_series(data_dir.to_str().unwrap(), &pattern)
                }
                DataCommands::ExportSeries { data_dir, pattern, output_file } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::DataTool::export_series(data_dir.to_str().unwrap(), &pattern, &output_file)
                }
                DataCommands::ImportSeries { data_dir, input_file } => {
                    let data_dir = data_dir.unwrap_or(cli.data_dir);
                    tools::DataTool::import_series(data_dir.to_str().unwrap(), &input_file)
                }
            }
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
