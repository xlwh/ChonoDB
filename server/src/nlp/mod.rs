use serde::{Deserialize, Serialize};
use thiserror::Error;
use regex::Regex;

#[derive(Debug, Error)]
pub enum NlpError {
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
    
    #[error("Parsing error: {0}")]
    ParsingError(String),
    
    #[error("Unsupported query type: {0}")]
    UnsupportedQueryType(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlpQuery {
    pub natural_language: String,
    pub promql: String,
    pub query_type: QueryType,
    pub time_range: Option<TimeRange>,
    pub aggregations: Vec<Aggregation>,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QueryType {
    RangeQuery,
    InstantQuery,
    RateQuery,
    SumQuery,
    AvgQuery,
    MaxQuery,
    MinQuery,
    CountQuery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: String,
    pub end: String,
    pub step: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregation {
    pub function: String,
    pub by: Vec<String>,
    pub without: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub label: String,
    pub operator: String,
    pub value: String,
}

pub struct NlpEngine {
    patterns: Vec<(Regex, fn(&str) -> Result<NlpQuery, NlpError>)>,
}

impl Default for NlpEngine {
    fn default() -> Self {
        let mut engine = Self {
            patterns: Vec::new(),
        };
        
        // 添加常见查询模式
        engine.add_patterns();
        engine
    }
}

impl NlpEngine {
    pub fn new() -> Self {
        Self::default()
    }

    fn add_patterns(&mut self) {
        // 模式 1: "cpu usage for the last 5 minutes"
        self.patterns.push((
            Regex::new(r"^([a-zA-Z\s]+) for the last (\d+) (minutes|hours|days)$").unwrap(),
            Self::parse_cpu_usage_last_time,
        ));
        
        // 模式 2: "average memory usage by host"
        self.patterns.push((
            Regex::new(r"^average ([a-zA-Z\s]+) by ([a-zA-Z]+)$").unwrap(),
            Self::parse_average_by,
        ));
        
        // 模式 3: "max disk usage in the last 1 hour"
        self.patterns.push((
            Regex::new(r"^max ([a-zA-Z\s]+) in the last (\d+) (minute|hour|day)$").unwrap(),
            Self::parse_max_in_last_time,
        ));
        
        // 模式 4: "rate of requests per second"
        self.patterns.push((
            Regex::new(r"^rate of ([a-zA-Z\s]+) per second$").unwrap(),
            Self::parse_rate_per_second,
        ));
        
        // 模式 5: "sum of network traffic by datacenter"
        self.patterns.push((
            Regex::new(r"^sum of ([a-zA-Z\s]+) by ([a-zA-Z]+)$").unwrap(),
            Self::parse_sum_by,
        ));
        
        // 模式 6: "min temperature in the last 24 hours"
        self.patterns.push((
            Regex::new(r"^min ([a-zA-Z\s]+) in the last (\d+) (minute|hour|day)s?$").unwrap(),
            Self::parse_min_in_last_time,
        ));
        
        // 模式 7: "count of errors in the last 30 minutes"
        self.patterns.push((
            Regex::new(r"^count of ([a-zA-Z\s]+) in the last (\d+) (minute|hour|day)s?$").unwrap(),
            Self::parse_count_in_last_time,
        ));
        
        // 模式 8: "cpu usage greater than 80%"
        self.patterns.push((
            Regex::new(r"^([a-zA-Z\s]+) greater than (\d+)%$").unwrap(),
            Self::parse_greater_than,
        ));
        
        // 模式 9: "memory usage less than 50%"
        self.patterns.push((
            Regex::new(r"^([a-zA-Z\s]+) less than (\d+)%$").unwrap(),
            Self::parse_less_than,
        ));
        
        // 模式 10: "rate of requests per minute for the last 1 hour"
        self.patterns.push((
            Regex::new(r"^rate of ([a-zA-Z\s]+) per (second|minute|hour) for the last (\d+) (minute|hour|day)s?$").unwrap(),
            Self::parse_rate_per_unit_time,
        ));
    }

    pub fn parse(&self, query: &str) -> Result<NlpQuery, NlpError> {
        for (pattern, parser) in &self.patterns {
            if pattern.is_match(query) {
                return parser(query);
            }
        }
        
        Err(NlpError::UnsupportedQueryType(query.to_string()))
    }

    fn parse_cpu_usage_last_time(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^([a-zA-Z\s]+) for the last (\d+) (minutes|hours|days)$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            let duration = captures[2].to_string();
            let unit = captures[3].to_string();
            
            let time_range = TimeRange {
                start: format!("-{}{}", duration, unit.chars().next().unwrap()),
                end: "now".to_string(),
                step: "15s".to_string(),
            };
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("{}{{}}", metric),
                query_type: QueryType::RangeQuery,
                time_range: Some(time_range),
                aggregations: Vec::new(),
                filters: Vec::new(),
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }

    fn parse_average_by(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^average ([a-zA-Z\s]+) by ([a-zA-Z]+)$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            let by = captures[2].to_string();
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("avg({}{{}}) by ({})
", metric, by),
                query_type: QueryType::AvgQuery,
                time_range: None,
                aggregations: vec![Aggregation {
                    function: "avg".to_string(),
                    by: vec![by],
                    without: Vec::new(),
                }],
                filters: Vec::new(),
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }

    fn parse_max_in_last_time(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^max ([a-zA-Z\s]+) in the last (\d+) (minute|hour|day)$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            let duration = captures[2].to_string();
            let unit = captures[3].to_string();
            
            let time_range = TimeRange {
                start: format!("-{}{}", duration, unit.chars().next().unwrap()),
                end: "now".to_string(),
                step: "15s".to_string(),
            };
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("max({}{{}})", metric),
                query_type: QueryType::MaxQuery,
                time_range: Some(time_range),
                aggregations: vec![Aggregation {
                    function: "max".to_string(),
                    by: Vec::new(),
                    without: Vec::new(),
                }],
                filters: Vec::new(),
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }

    fn parse_rate_per_second(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^rate of ([a-zA-Z\s]+) per second$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("rate({}{{}}[1m])", metric),
                query_type: QueryType::RateQuery,
                time_range: None,
                aggregations: Vec::new(),
                filters: Vec::new(),
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }

    fn parse_sum_by(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^sum of ([a-zA-Z\s]+) by ([a-zA-Z]+)$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            let by = captures[2].to_string();
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("sum({}{{}}) by ({})", metric, by),
                query_type: QueryType::SumQuery,
                time_range: None,
                aggregations: vec![Aggregation {
                    function: "sum".to_string(),
                    by: vec![by],
                    without: Vec::new(),
                }],
                filters: Vec::new(),
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }

    fn parse_min_in_last_time(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^min ([a-zA-Z\s]+) in the last (\d+) (minute|hour|day)s?$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            let duration = captures[2].to_string();
            let unit = captures[3].to_string();
            
            let time_range = TimeRange {
                start: format!("-{}{}", duration, unit.chars().next().unwrap()),
                end: "now".to_string(),
                step: "15s".to_string(),
            };
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("min({}{{}})", metric),
                query_type: QueryType::MinQuery,
                time_range: Some(time_range),
                aggregations: vec![Aggregation {
                    function: "min".to_string(),
                    by: Vec::new(),
                    without: Vec::new(),
                }],
                filters: Vec::new(),
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }

    fn parse_count_in_last_time(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^count of ([a-zA-Z\s]+) in the last (\d+) (minute|hour|day)s?$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            let duration = captures[2].to_string();
            let unit = captures[3].to_string();
            
            let time_range = TimeRange {
                start: format!("-{}{}", duration, unit.chars().next().unwrap()),
                end: "now".to_string(),
                step: "15s".to_string(),
            };
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("count({}{{}})", metric),
                query_type: QueryType::CountQuery,
                time_range: Some(time_range),
                aggregations: vec![Aggregation {
                    function: "count".to_string(),
                    by: Vec::new(),
                    without: Vec::new(),
                }],
                filters: Vec::new(),
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }

    fn parse_greater_than(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^([a-zA-Z\s]+) greater than (\d+)%$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            let value = captures[2].to_string();
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("{}{{}} > {}", metric, value),
                query_type: QueryType::InstantQuery,
                time_range: None,
                aggregations: Vec::new(),
                filters: vec![Filter {
                    label: "value".to_string(),
                    operator: ">>".to_string(),
                    value: value,
                }],
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }

    fn parse_less_than(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^([a-zA-Z\s]+) less than (\d+)%$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            let value = captures[2].to_string();
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("{}{{}} < {}", metric, value),
                query_type: QueryType::InstantQuery,
                time_range: None,
                aggregations: Vec::new(),
                filters: vec![Filter {
                    label: "value".to_string(),
                    operator: "<".to_string(),
                    value: value,
                }],
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }

    fn parse_rate_per_unit_time(query: &str) -> Result<NlpQuery, NlpError> {
        let re = Regex::new(r"^rate of ([a-zA-Z\s]+) per (second|minute|hour) for the last (\d+) (minute|hour|day)s?$").unwrap();
        if let Some(captures) = re.captures(query) {
            let metric = captures[1].trim().replace(" ", "_");
            let unit = captures[2].to_string();
            let duration = captures[3].to_string();
            let time_unit = captures[4].to_string();
            
            let time_range = TimeRange {
                start: format!("-{}{}", duration, time_unit.chars().next().unwrap()),
                end: "now".to_string(),
                step: "15s".to_string(),
            };
            
            // 根据单位设置合适的时间窗口
            let window = match unit.as_str() {
                "second" => "1m",
                "minute" => "5m",
                "hour" => "1h",
                _ => "1m",
            };
            
            Ok(NlpQuery {
                natural_language: query.to_string(),
                promql: format!("rate({}{{}}[{}])", metric, window),
                query_type: QueryType::RateQuery,
                time_range: Some(time_range),
                aggregations: Vec::new(),
                filters: Vec::new(),
            })
        } else {
            Err(NlpError::ParsingError(query.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_usage() {
        let engine = NlpEngine::new();
        let query = "cpu usage for the last 5 minutes";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "cpu_usage{}");
        assert_eq!(nlp_query.query_type, QueryType::RangeQuery);
        assert!(nlp_query.time_range.is_some());
    }

    #[test]
    fn test_parse_average_by() {
        let engine = NlpEngine::new();
        let query = "average memory usage by host";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "avg(memory_usage{}) by (host)\n");
        assert_eq!(nlp_query.query_type, QueryType::AvgQuery);
        assert!(!nlp_query.aggregations.is_empty());
    }

    #[test]
    fn test_parse_max_in_last_time() {
        let engine = NlpEngine::new();
        let query = "max disk usage in the last 1 hour";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "max(disk_usage{})");
        assert_eq!(nlp_query.query_type, QueryType::MaxQuery);
        assert!(nlp_query.time_range.is_some());
    }

    #[test]
    fn test_parse_rate_per_second() {
        let engine = NlpEngine::new();
        let query = "rate of requests per second";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "rate(requests{}[1m])");
        assert_eq!(nlp_query.query_type, QueryType::RateQuery);
    }

    #[test]
    fn test_unsupported_query() {
        let engine = NlpEngine::new();
        let query = "this is an unsupported query";
        let result = engine.parse(query);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            NlpError::UnsupportedQueryType(msg) => {
                assert_eq!(msg, query);
            }
            _ => panic!("Expected UnsupportedQueryType error"),
        }
    }

    #[test]
    fn test_parse_sum_by() {
        let engine = NlpEngine::new();
        let query = "sum of network traffic by datacenter";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "sum(network_traffic{}) by (datacenter)");
        assert_eq!(nlp_query.query_type, QueryType::SumQuery);
        assert!(!nlp_query.aggregations.is_empty());
    }

    #[test]
    fn test_parse_min_in_last_time() {
        let engine = NlpEngine::new();
        let query = "min temperature in the last 24 hours";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "min(temperature{})");
        assert_eq!(nlp_query.query_type, QueryType::MinQuery);
        assert!(nlp_query.time_range.is_some());
    }

    #[test]
    fn test_parse_count_in_last_time() {
        let engine = NlpEngine::new();
        let query = "count of errors in the last 30 minutes";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "count(errors{})");
        assert_eq!(nlp_query.query_type, QueryType::CountQuery);
        assert!(nlp_query.time_range.is_some());
    }

    #[test]
    fn test_parse_greater_than() {
        let engine = NlpEngine::new();
        let query = "cpu usage greater than 80%";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "cpu_usage{} > 80");
        assert_eq!(nlp_query.query_type, QueryType::InstantQuery);
        assert!(!nlp_query.filters.is_empty());
    }

    #[test]
    fn test_parse_less_than() {
        let engine = NlpEngine::new();
        let query = "memory usage less than 50%";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "memory_usage{} < 50");
        assert_eq!(nlp_query.query_type, QueryType::InstantQuery);
        assert!(!nlp_query.filters.is_empty());
    }

    #[test]
    fn test_parse_rate_per_unit_time() {
        let engine = NlpEngine::new();
        let query = "rate of requests per minute for the last 1 hour";
        let result = engine.parse(query);
        
        assert!(result.is_ok());
        let nlp_query = result.unwrap();
        assert_eq!(nlp_query.natural_language, query);
        assert_eq!(nlp_query.promql, "rate(requests{}[5m])");
        assert_eq!(nlp_query.query_type, QueryType::RateQuery);
        assert!(nlp_query.time_range.is_some());
    }
}
