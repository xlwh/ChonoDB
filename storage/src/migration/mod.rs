use crate::error::Result;
use crate::export::{ExportData, ExportFormat, ExportTimeSeries, ExportMetadata, ExportSample};
use crate::memstore::MemStore;
use crate::model::{Label, Sample, TimeSeries};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn, error};

/// 迁移配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    /// 批量大小
    pub batch_size: usize,
    /// 并发数
    pub concurrency: usize,
    /// 超时时间（秒）
    pub timeout_secs: u64,
    /// 是否验证数据
    pub verify_data: bool,
    /// 是否跳过错误
    pub skip_errors: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            concurrency: 4,
            timeout_secs: 300,
            verify_data: true,
            skip_errors: false,
        }
    }
}

/// 迁移统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MigrationStats {
    pub total_series: u64,
    pub total_samples: u64,
    pub processed_series: u64,
    pub processed_samples: u64,
    pub failed_series: u64,
    pub failed_samples: u64,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub duration_secs: f64,
}

impl MigrationStats {
    pub fn new() -> Self {
        Self {
            start_time: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        }
    }

    pub fn finish(&mut self) {
        self.end_time = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_samples == 0 {
            return 100.0;
        }
        ((self.processed_samples as f64) / (self.total_samples as f64)) * 100.0
    }
}

/// 数据源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataSourceType {
    ChronoDB,
    Prometheus,
    InfluxDB,
    TimescaleDB,
    OpenTSDB,
    Graphite,
    VictoriaMetrics,
    Thanos,
    M3DB,
}

impl std::fmt::Display for DataSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSourceType::ChronoDB => write!(f, "chronodb"),
            DataSourceType::Prometheus => write!(f, "prometheus"),
            DataSourceType::InfluxDB => write!(f, "influxdb"),
            DataSourceType::TimescaleDB => write!(f, "timescaledb"),
            DataSourceType::OpenTSDB => write!(f, "opentsdb"),
            DataSourceType::Graphite => write!(f, "graphite"),
            DataSourceType::VictoriaMetrics => write!(f, "victoriametrics"),
            DataSourceType::Thanos => write!(f, "thanos"),
            DataSourceType::M3DB => write!(f, "m3db"),
        }
    }
}

/// 数据迁移器
pub struct DataMigrator {
    config: MigrationConfig,
    store: Arc<MemStore>,
}

impl DataMigrator {
    pub fn new(store: Arc<MemStore>, config: MigrationConfig) -> Self {
        Self { config, store }
    }

    /// 从文件导入数据
    pub async fn import_from_file<P: AsRef<Path>>(
        &self,
        file_path: P,
        format: ExportFormat,
        source_type: DataSourceType,
    ) -> Result<MigrationStats> {
        let start = Instant::now();
        let mut stats = MigrationStats::new();

        info!("Starting import from {:?} (format: {:?}, source: {})", 
              file_path.as_ref(), format, source_type);

        let file_content = tokio::fs::read_to_string(file_path).await?;

        let export_data = match format {
            ExportFormat::Json => {
                serde_json::from_str(&file_content)
                    .map_err(|e| crate::error::Error::Internal(format!("JSON parse error: {}", e)))?
            }
            ExportFormat::Csv => {
                self.parse_csv(&file_content)?
            }
            _ => {
                return Err(crate::error::Error::Internal(
                    format!("Unsupported import format: {:?}", format)
                ));
            }
        };

        stats.total_series = export_data.time_series.len() as u64;
        stats.total_samples = export_data.time_series.iter()
            .map(|ts| ts.samples.len() as u64)
            .sum();

        // 导入数据
        for ts in export_data.time_series {
            match self.import_time_series(ts).await {
                Ok(sample_count) => {
                    stats.processed_series += 1;
                    stats.processed_samples += sample_count;
                }
                Err(e) => {
                    stats.failed_series += 1;
                    if !self.config.skip_errors {
                        error!("Failed to import time series: {:?}", e);
                        return Err(e);
                    }
                    warn!("Skipping failed time series: {:?}", e);
                }
            }
        }

        stats.duration_secs = start.elapsed().as_secs_f64();
        stats.finish();

        info!("Import completed: {:?}", stats);

        Ok(stats)
    }

    /// 导出数据到文件
    pub async fn export_to_file<P: AsRef<Path>>(
        &self,
        file_path: P,
        format: ExportFormat,
        query: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<MigrationStats> {
        let start = Instant::now();
        let mut stats = MigrationStats::new();

        info!("Starting export to {:?} (format: {:?})", file_path.as_ref(), format);

        // 查询数据
        let series_list = self.store.query(&[("__name__".to_string(), query.to_string())], start_time, end_time)?;

        stats.total_series = series_list.len() as u64;
        stats.total_samples = series_list.iter()
            .map(|ts| ts.samples.len() as u64)
            .sum();

        // 转换为导出格式
        let mut export_data = ExportData::new()
            .with_query(query.to_string())
            .with_time_range(start_time, end_time);

        for ts in series_list {
            let export_ts = self.convert_to_export_format(ts)?;
            export_data = export_data.add_time_series(export_ts);
            stats.processed_series += 1;
            stats.processed_samples += export_data.time_series.last()
                .map(|ts| ts.samples.len() as u64)
                .unwrap_or(0);
        }

        // 写入文件
        let content = match format {
            ExportFormat::Json => export_data.to_json()
                .map_err(|e| crate::error::Error::Internal(format!("JSON serialization error: {}", e)))?,
            ExportFormat::Csv => export_data.to_csv()
                .map_err(|e| crate::error::Error::Internal(format!("CSV serialization error: {}", e)))?,
            _ => {
                return Err(crate::error::Error::Internal(
                    format!("Unsupported export format: {:?}", format)
                ));
            }
        };

        tokio::fs::write(file_path, content).await?;

        stats.duration_secs = start.elapsed().as_secs_f64();
        stats.finish();

        info!("Export completed: {:?}", stats);

        Ok(stats)
    }

    /// 从 Prometheus 导入数据
    pub async fn import_from_prometheus(
        &self,
        prometheus_url: &str,
        _query: &str,
        _start_time: i64,
        _end_time: i64,
    ) -> Result<MigrationStats> {
        info!("Importing from Prometheus: {}", prometheus_url);

        // 这里应该实现 Prometheus HTTP API 调用
        // 简化实现，返回空统计
        let mut stats = MigrationStats::new();
        stats.finish();

        warn!("Prometheus import not fully implemented yet");

        Ok(stats)
    }

    /// 从 InfluxDB 导入数据
    pub async fn import_from_influxdb(
        &self,
        influxdb_url: &str,
        database: &str,
        _query: &str,
    ) -> Result<MigrationStats> {
        info!("Importing from InfluxDB: {}/{}", influxdb_url, database);

        // 这里应该实现 InfluxDB HTTP API 调用
        // 简化实现，返回空统计
        let mut stats = MigrationStats::new();
        stats.finish();

        warn!("InfluxDB import not fully implemented yet");

        Ok(stats)
    }

    /// 从其他时序数据库迁移数据
    pub async fn migrate_from(
        &self,
        source_type: DataSourceType,
        source_config: &str,
    ) -> Result<MigrationStats> {
        info!("Migrating from {}: {}", source_type, source_config);

        match source_type {
            DataSourceType::Prometheus => {
                self.import_from_prometheus(source_config, "", 0, 0).await
            }
            DataSourceType::InfluxDB => {
                self.import_from_influxdb(source_config, "default", "").await
            }
            _ => {
                Err(crate::error::Error::Internal(
                    format!("Migration from {} not implemented yet", source_type)
                ))
            }
        }
    }

    /// 导入时间序列
    async fn import_time_series(&self, ts: ExportTimeSeries) -> Result<u64> {
        let labels: Vec<Label> = ts.metadata.labels
            .into_iter()
            .map(|(k, v)| Label::new(&k, &v))
            .collect();

        let samples: Vec<Sample> = ts.samples
            .into_iter()
            .map(|s| Sample::new(s.timestamp, s.value))
            .collect();

        let sample_count = samples.len() as u64;

        // 批量写入
        self.store.write_batch(vec![(labels, samples)])?;

        Ok(sample_count)
    }

    /// 转换为导出格式
    fn convert_to_export_format(&self, ts: TimeSeries) -> Result<ExportTimeSeries> {
        let labels: Vec<(String, String)> = ts.labels
            .iter()
            .map(|l| (l.name.clone(), l.value.clone()))
            .collect();

        let samples: Vec<ExportSample> = ts.samples
            .iter()
            .map(|s| ExportSample {
                timestamp: s.timestamp,
                value: s.value,
            })
            .collect();

        let metadata = ExportMetadata {
            metric_name: ts.labels
                .iter()
                .find(|l| l.name == "__name__")
                .map(|l| l.value.clone())
                .unwrap_or_default(),
            labels,
            unit: None,
            description: None,
        };

        Ok(ExportTimeSeries {
            metadata,
            samples,
        })
    }

    /// 解析 CSV
    fn parse_csv(&self, content: &str) -> Result<ExportData> {
        let mut export_data = ExportData::new();
        let mut current_series: HashMap<String, ExportTimeSeries> = HashMap::new();

        for line in content.lines().skip(1) { // 跳过表头
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 4 {
                continue;
            }

            let metric_name = parts[0].to_string();
            let labels_str = parts[1];
            let timestamp: i64 = parts[2].parse()
                .map_err(|_| crate::error::Error::Internal("Invalid timestamp".to_string()))?;
            let value: f64 = parts[3].parse()
                .map_err(|_| crate::error::Error::Internal("Invalid value".to_string()))?;

            // 解析标签
            let labels: Vec<(String, String)> = labels_str
                .split(';')
                .filter_map(|s| {
                    let kv: Vec<&str> = s.split('=').collect();
                    if kv.len() == 2 {
                        Some((kv[0].to_string(), kv[1].to_string()))
                    } else {
                        None
                    }
                })
                .collect();

            let series_key = format!("{}:{}", metric_name, labels_str);

            let series = current_series.entry(series_key.clone()).or_insert_with(|| {
                ExportTimeSeries {
                    metadata: ExportMetadata {
                        metric_name: metric_name.clone(),
                        labels: labels.clone(),
                        unit: None,
                        description: None,
                    },
                    samples: Vec::new(),
                }
            });

            series.samples.push(ExportSample { timestamp, value });
        }

        for (_, series) in current_series {
            export_data = export_data.add_time_series(series);
        }

        Ok(export_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_migration_config() {
        let config = MigrationConfig::default();
        assert_eq!(config.batch_size, 1000);
        assert_eq!(config.concurrency, 4);
        assert_eq!(config.timeout_secs, 300);
        assert!(config.verify_data);
        assert!(!config.skip_errors);
    }

    #[test]
    fn test_migration_stats() {
        let mut stats = MigrationStats::new();
        assert!(stats.start_time.is_some());
        
        stats.total_series = 100;
        stats.total_samples = 1000;
        stats.processed_series = 90;
        stats.processed_samples = 900;
        
        assert_eq!(stats.success_rate(), 90.0);
        
        stats.finish();
        assert!(stats.end_time.is_some());
    }

    #[test]
    fn test_data_source_type() {
        assert_eq!(DataSourceType::Prometheus.to_string(), "prometheus");
        assert_eq!(DataSourceType::InfluxDB.to_string(), "influxdb");
        assert_eq!(DataSourceType::ChronoDB.to_string(), "chronodb");
    }
}
