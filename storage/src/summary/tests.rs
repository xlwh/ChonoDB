#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_summary() {
        let generator = SummaryGenerator::new();
        
        let metric_name = "cpu_usage";
        let labels = vec![("server", "server1"), ("region", "us-east-1")];
        
        // 生成测试数据：模拟 CPU 使用率，有轻微上升趋势
        let mut samples = Vec::new();
        let start_time = 1609459200; // 2021-01-01 00:00:00
        
        for i in 0..100 {
            let timestamp = start_time + i * 60; // 每分钟一个样本
            let value = 50.0 + i as f64 * 0.1; // 从 50 开始，每分钟增加 0.1
            samples.push((timestamp, value));
        }
        
        let result = generator.generate_summary(metric_name, &labels, &samples);
        assert!(result.is_ok());
        
        let summary = result.unwrap();
        assert_eq!(summary.metric_name, metric_name);
        assert_eq!(summary.labels, labels);
        assert_eq!(summary.sample_count, 100);
        
        // 检查基本统计信息
        assert!(summary.basic_stats.min >= 50.0);
        assert!(summary.basic_stats.max <= 60.0);
        assert!(summary.basic_stats.mean >= 54.0 && summary.basic_stats.mean <= 56.0);
        
        // 检查趋势分析
        assert!(summary.trend.is_some());
        if let Some(trend) = &summary.trend {
            assert!(trend.slope > 0.0);
            assert_eq!(trend.trend_direction, TrendDirection::Up);
        }
        
        // 检查周期性检测
        assert!(summary.periodicity.is_some());
        if let Some(periodicity) = &summary.periodicity {
            assert!(periodicity.is_periodic);
            assert_eq!(periodicity.period, 60); // 60秒周期
        }
    }

    #[test]
    fn test_insufficient_samples() {
        let generator = SummaryGenerator::new().with_min_samples(10);
        
        let metric_name = "cpu_usage";
        let labels = vec![("server", "server1")];
        let samples = vec![(1609459200, 50.0), (1609459260, 51.0)];
        
        let result = generator.generate_summary(metric_name, &labels, &samples);
        assert!(result.is_err());
        match result.unwrap_err() {
            SummaryError::InvalidData(msg) => {
                assert!(msg.contains("Insufficient samples"));
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[test]
    fn test_basic_stats_calculation() {
        let generator = SummaryGenerator::new();
        
        let samples = vec![
            &(1609459200, 10.0),
            &(1609459260, 20.0),
            &(1609459320, 30.0),
            &(1609459380, 40.0),
            &(1609459440, 50.0),
        ];
        
        let result = generator.calculate_basic_stats(&samples);
        assert!(result.is_ok());
        
        let stats = result.unwrap();
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 50.0);
        assert_eq!(stats.mean, 30.0);
        assert_eq!(stats.median, 30.0);
    }

    #[test]
    fn test_trend_analysis() {
        let generator = SummaryGenerator::new();
        
        let samples = vec![
            &(1609459200, 10.0),
            &(1609459260, 20.0),
            &(1609459320, 30.0),
            &(1609459380, 40.0),
            &(1609459440, 50.0),
        ];
        
        let trend = generator.analyze_trend(&samples);
        assert!(trend.is_some());
        if let Some(trend_info) = trend {
            assert!(trend_info.slope > 0.0);
            assert_eq!(trend_info.trend_direction, TrendDirection::Up);
            assert!(trend_info.r_squared > 0.99); // 完全线性，R² 接近 1
        }
    }

    #[test]
    fn test_periodicity_detection() {
        let generator = SummaryGenerator::new();
        
        let mut samples = Vec::new();
        let start_time = 1609459200;
        
        for i in 0..20 {
            let timestamp = start_time + i * 300; // 每 5 分钟一个样本
            samples.push(&(timestamp, 50.0 + (i % 12) as f64));
        }
        
        let periodicity = generator.detect_periodicity(&samples);
        assert!(periodicity.is_some());
        if let Some(period_info) = periodicity {
            assert!(period_info.is_periodic);
            assert_eq!(period_info.period, 300); // 300秒周期
            assert!(period_info.confidence > 0.9);
        }
    }
}