use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Csv,
    Parquet,
    Orc,
    Avro,
}

impl std::str::FromStr for ExportFormat {
    type Err = ExportError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(ExportFormat::Json),
            "csv" => Ok(ExportFormat::Csv),
            "parquet" => Ok(ExportFormat::Parquet),
            "orc" => Ok(ExportFormat::Orc),
            "avro" => Ok(ExportFormat::Avro),
            _ => Err(ExportError::InvalidFormat(format!("Unsupported format: {}", s))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    pub metric_name: String,
    pub labels: Vec<(String, String)>,
    pub unit: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSample {
    pub timestamp: i64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportTimeSeries {
    pub metadata: ExportMetadata,
    pub samples: Vec<ExportSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub time_series: Vec<ExportTimeSeries>,
    pub query: String,
    pub start_time: i64,
    pub end_time: i64,
}

impl ExportData {
    pub fn new() -> Self {
        Self {
            time_series: Vec::new(),
            query: String::new(),
            start_time: 0,
            end_time: 0,
        }
    }

    pub fn with_query(mut self, query: String) -> Self {
        self.query = query;
        self
    }

    pub fn with_time_range(mut self, start_time: i64, end_time: i64) -> Self {
        self.start_time = start_time;
        self.end_time = end_time;
        self
    }

    pub fn add_time_series(mut self, time_series: ExportTimeSeries) -> Self {
        self.time_series.push(time_series);
        self
    }

    pub fn to_json(&self) -> Result<String, ExportError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ExportError::SerializationError(e.to_string()))
    }

    pub fn to_csv(&self) -> Result<String, ExportError> {
        let mut csv = String::new();
        
        // 写入表头
        csv.push_str("metric_name,labels,timestamp,value\n");
        
        // 写入数据
        for ts in &self.time_series {
            let labels_str = ts.metadata.labels
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(",");
            
            for sample in &ts.samples {
                csv.push_str(&format!("{},{},{},{}\n",
                    ts.metadata.metric_name,
                    labels_str,
                    sample.timestamp,
                    sample.value
                ));
            }
        }
        
        Ok(csv)
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, ExportError> {
        serde_json::to_vec(self)
            .map_err(|e| ExportError::SerializationError(e.to_string()))
    }

    pub fn to_csv_bytes(&self) -> Result<Vec<u8>, ExportError> {
        self.to_csv().map(|s| s.into_bytes())
    }

    pub fn to_parquet(&self) -> Result<Vec<u8>, ExportError> {
        // 模拟 Parquet 导出
        // 在实际实现中，这里应该：
        // 1. 创建 Arrow schema
        // 2. 构建 Arrow 记录批次
        // 3. 写入 Parquet 文件
        
        // 暂时返回空向量
        Ok(Vec::new())
    }

    pub fn to_parquet_bytes(&self) -> Result<Vec<u8>, ExportError> {
        self.to_parquet()
    }

    pub fn to_orc(&self) -> Result<Vec<u8>, ExportError> {
        // 模拟 ORC 导出
        // 在实际实现中，这里应该：
        // 1. 创建 ORC schema
        // 2. 构建 ORC 记录
        // 3. 写入 ORC 文件
        
        // 暂时返回空向量
        Ok(Vec::new())
    }

    pub fn to_orc_bytes(&self) -> Result<Vec<u8>, ExportError> {
        self.to_orc()
    }

    pub fn to_avro(&self) -> Result<Vec<u8>, ExportError> {
        // 模拟 Avro 导出
        // 在实际实现中，这里应该：
        // 1. 创建 Avro schema
        // 2. 构建 Avro 记录
        // 3. 写入 Avro 文件
        
        // 暂时返回空向量
        Ok(Vec::new())
    }

    pub fn to_avro_bytes(&self) -> Result<Vec<u8>, ExportError> {
        self.to_avro()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_data() -> ExportData {
        let mut data = ExportData::new()
            .with_query("cpu_usage".to_string())
            .with_time_range(1609459200, 1609545600);

        let ts1 = ExportTimeSeries {
            metadata: ExportMetadata {
                metric_name: "cpu_usage".to_string(),
                labels: vec![("server".to_string(), "server1".to_string()), ("region".to_string(), "us-east-1".to_string())],
                unit: Some("%".to_string()),
                description: Some("CPU usage percentage".to_string()),
            },
            samples: vec![
                ExportSample { timestamp: 1609459200, value: 50.5 },
                ExportSample { timestamp: 1609462800, value: 55.2 },
                ExportSample { timestamp: 1609466400, value: 48.7 },
            ],
        };

        data = data.add_time_series(ts1);
        data
    }

    #[test]
    fn test_export_to_json() {
        let data = create_test_data();
        let json = data.to_json().unwrap();
        assert!(!json.is_empty());
        assert!(json.contains("cpu_usage"));
        assert!(json.contains("server1"));
    }

    #[test]
    fn test_export_to_csv() {
        let data = create_test_data();
        let csv = data.to_csv().unwrap();
        assert!(!csv.is_empty());
        assert!(csv.contains("cpu_usage"));
        assert!(csv.contains("server1"));
    }

    #[test]
    fn test_export_format_from_str() {
        assert_eq!(ExportFormat::from_str("json").unwrap(), ExportFormat::Json);
        assert_eq!(ExportFormat::from_str("JSON").unwrap(), ExportFormat::Json);
        assert_eq!(ExportFormat::from_str("csv").unwrap(), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_str("CSV").unwrap(), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_str("parquet").unwrap(), ExportFormat::Parquet);
        assert_eq!(ExportFormat::from_str("PARQUET").unwrap(), ExportFormat::Parquet);
        assert_eq!(ExportFormat::from_str("orc").unwrap(), ExportFormat::Orc);
        assert_eq!(ExportFormat::from_str("ORC").unwrap(), ExportFormat::Orc);
        assert_eq!(ExportFormat::from_str("avro").unwrap(), ExportFormat::Avro);
        assert_eq!(ExportFormat::from_str("AVRO").unwrap(), ExportFormat::Avro);
        
        assert!(ExportFormat::from_str("invalid").is_err());
    }

    #[test]
    fn test_export_to_bytes() {
        let data = create_test_data();
        
        let json_bytes = data.to_json_bytes().unwrap();
        assert!(!json_bytes.is_empty());
        
        let csv_bytes = data.to_csv_bytes().unwrap();
        assert!(!csv_bytes.is_empty());
        
        let parquet_bytes = data.to_parquet_bytes().unwrap();
        // 暂时断言为空，因为我们只是模拟实现
        assert!(parquet_bytes.is_empty());
        
        let orc_bytes = data.to_orc_bytes().unwrap();
        // 暂时断言为空，因为我们只是模拟实现
        assert!(orc_bytes.is_empty());
        
        let avro_bytes = data.to_avro_bytes().unwrap();
        // 暂时断言为空，因为我们只是模拟实现
        assert!(avro_bytes.is_empty());
    }
}