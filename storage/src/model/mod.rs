use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type TimeSeriesId = u64;
pub type Timestamp = i64;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub value: String,
}

impl Label {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.name, self.value)
    }
}

pub type Labels = Vec<Label>;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: Timestamp,
    pub value: f64,
}

impl Sample {
    pub fn new(timestamp: Timestamp, value: f64) -> Self {
        Self { timestamp, value }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    pub id: TimeSeriesId,
    pub labels: Labels,
    pub samples: Vec<Sample>,
}

impl TimeSeries {
    pub fn new(id: TimeSeriesId, labels: Labels) -> Self {
        Self {
            id,
            labels,
            samples: Vec::new(),
        }
    }

    pub fn add_sample(&mut self, sample: Sample) {
        self.samples.push(sample);
    }

    pub fn add_samples(&mut self, samples: Vec<Sample>) {
        self.samples.extend(samples);
    }

    pub fn samples_in_range(&self, start: Timestamp, end: Timestamp) -> Vec<&Sample> {
        self.samples
            .iter()
            .filter(|s| s.timestamp >= start && s.timestamp <= end)
            .collect()
    }
}

pub fn labels_to_string(labels: &[Label]) -> String {
    let mut sorted: Vec<_> = labels.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    
    let pairs: Vec<String> = sorted.iter().map(|l| format!("{}={}", l.name, l.value)).collect();
    format!("{{{}}}", pairs.join(", "))
}

pub fn labels_to_map(labels: &[Label]) -> BTreeMap<String, String> {
    labels.iter().map(|l| (l.name.clone(), l.value.clone())).collect()
}

pub fn calculate_series_id(labels: &[Label]) -> TimeSeriesId {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut sorted: Vec<_> = labels.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    
    let mut hasher = DefaultHasher::new();
    for label in sorted {
        label.name.hash(&mut hasher);
        label.value.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_creation() {
        let label = Label::new("job", "prometheus");
        assert_eq!(label.name, "job");
        assert_eq!(label.value, "prometheus");
    }

    #[test]
    fn test_sample_creation() {
        let sample = Sample::new(1000, 42.5);
        assert_eq!(sample.timestamp, 1000);
        assert_eq!(sample.value, 42.5);
    }

    #[test]
    fn test_series_id_deterministic() {
        let labels1 = vec![
            Label::new("job", "prometheus"),
            Label::new("instance", "localhost:9090"),
        ];
        let labels2 = vec![
            Label::new("instance", "localhost:9090"),
            Label::new("job", "prometheus"),
        ];
        
        let id1 = calculate_series_id(&labels1);
        let id2 = calculate_series_id(&labels2);
        assert_eq!(id1, id2);
    }
}
