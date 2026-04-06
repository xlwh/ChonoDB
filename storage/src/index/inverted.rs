use crate::error::Result;
use crate::model::{Label, TimeSeriesId};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Debug)]
pub struct InvertedIndex {
    label_name_to_values: DashMap<String, DashMap<String, BTreeSet<TimeSeriesId>>>,
    series_labels: RwLock<HashMap<TimeSeriesId, Vec<Label>>>,
}

impl InvertedIndex {
    pub fn new() -> Self {
        Self {
            label_name_to_values: DashMap::new(),
            series_labels: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_series(&self, series_id: TimeSeriesId, labels: &[Label]) -> Result<()> {
        {
            let mut series_labels = self.series_labels.write();
            series_labels.insert(series_id, labels.to_vec());
        }
        
        for label in labels {
            self.add_label_entry(&label.name, &label.value, series_id);
        }
        
        Ok(())
    }

    fn add_label_entry(&self, name: &str, value: &str, series_id: TimeSeriesId) {
        let values_map = self
            .label_name_to_values
            .entry(name.to_string())
            .or_insert_with(DashMap::new);
        
        let mut series_set = values_map
            .entry(value.to_string())
            .or_insert_with(BTreeSet::new);
        
        series_set.insert(series_id);
    }

    pub fn remove_series(&self, series_id: TimeSeriesId) -> Result<()> {
        let labels = {
            let series_labels = self.series_labels.read();
            series_labels.get(&series_id).cloned()
        };
        
        if let Some(labels) = labels {
            for label in &labels {
                self.remove_label_entry(&label.name, &label.value, series_id);
            }
            
            let mut series_labels = self.series_labels.write();
            series_labels.remove(&series_id);
        }
        
        Ok(())
    }

    fn remove_label_entry(&self, name: &str, value: &str, series_id: TimeSeriesId) {
        if let Some(values_map) = self.label_name_to_values.get(name) {
            if let Some(mut series_set) = values_map.get_mut(value) {
                series_set.remove(&series_id);
            }
        }
    }

    pub fn lookup(&self, name: &str, value: &str) -> Vec<TimeSeriesId> {
        if let Some(values_map) = self.label_name_to_values.get(name) {
            if let Some(series_set) = values_map.get(value) {
                return series_set.iter().copied().collect();
            }
        }
        Vec::new()
    }

    pub fn lookup_by_matcher(&self, name: &str, matcher: &LabelMatcher) -> Vec<TimeSeriesId> {
        match matcher {
            LabelMatcher::Equal(value) => self.lookup(name, value),
            LabelMatcher::NotEqual(value) => {
                let matching = self.lookup(name, value);
                let all_series: HashSet<TimeSeriesId> = {
                    let series_labels = self.series_labels.read();
                    series_labels.keys().copied().collect()
                };
                all_series
                    .difference(&matching.into_iter().collect())
                    .copied()
                    .collect()
            }
            LabelMatcher::Regex(pattern) => {
                let regex = regex::Regex::new(pattern).unwrap();
                let mut result = Vec::new();
                
                if let Some(values_map) = self.label_name_to_values.get(name) {
                    for entry in values_map.iter() {
                        if regex.is_match(entry.key()) {
                            result.extend(entry.value().iter().copied());
                        }
                    }
                }
                result
            }
            LabelMatcher::NotRegex(pattern) => {
                let regex = regex::Regex::new(pattern).unwrap();
                let mut result = Vec::new();
                
                if let Some(values_map) = self.label_name_to_values.get(name) {
                    for entry in values_map.iter() {
                        if !regex.is_match(entry.key()) {
                            result.extend(entry.value().iter().copied());
                        }
                    }
                }
                result
            }
        }
    }

    pub fn label_names(&self) -> Vec<String> {
        self.label_name_to_values
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub fn label_values(&self, name: &str) -> Vec<String> {
        if let Some(values_map) = self.label_name_to_values.get(name) {
            return values_map
                .iter()
                .map(|entry| entry.key().clone())
                .collect();
        }
        Vec::new()
    }

    pub fn series_count(&self) -> usize {
        let series_labels = self.series_labels.read();
        series_labels.len()
    }

    pub fn get_series_labels(&self, series_id: TimeSeriesId) -> Option<Vec<Label>> {
        let series_labels = self.series_labels.read();
        series_labels.get(&series_id).cloned()
    }
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LabelMatcher {
    Equal(String),
    NotEqual(String),
    Regex(String),
    NotRegex(String),
}

impl LabelMatcher {
    pub fn matches(&self, value: &str) -> bool {
        match self {
            LabelMatcher::Equal(v) => value == v,
            LabelMatcher::NotEqual(v) => value != v,
            LabelMatcher::Regex(pattern) => {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(value))
                    .unwrap_or(false)
            }
            LabelMatcher::NotRegex(pattern) => {
                regex::Regex::new(pattern)
                    .map(|re| !re.is_match(value))
                    .unwrap_or(true)
            }
        }
    }
}

pub fn intersect_series_ids(a: &[TimeSeriesId], b: &[TimeSeriesId]) -> Vec<TimeSeriesId> {
    let set_a: HashSet<_> = a.iter().copied().collect();
    let set_b: HashSet<_> = b.iter().copied().collect();
    set_a.intersection(&set_b).copied().collect()
}

pub fn union_series_ids(a: &[TimeSeriesId], b: &[TimeSeriesId]) -> Vec<TimeSeriesId> {
    let set_a: HashSet<_> = a.iter().copied().collect();
    let set_b: HashSet<_> = b.iter().copied().collect();
    set_a.union(&set_b).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Label;

    #[test]
    fn test_inverted_index_basic() {
        let index = InvertedIndex::new();
        
        let labels1 = vec![
            Label::new("job", "prometheus"),
            Label::new("instance", "localhost:9090"),
        ];
        let labels2 = vec![
            Label::new("job", "prometheus"),
            Label::new("instance", "localhost:9091"),
        ];
        
        index.add_series(1, &labels1).unwrap();
        index.add_series(2, &labels2).unwrap();
        
        let result = index.lookup("job", "prometheus");
        assert_eq!(result.len(), 2);
        
        let result = index.lookup("instance", "localhost:9090");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 1);
    }

    #[test]
    fn test_label_matcher() {
        let matcher = LabelMatcher::Equal("prometheus".to_string());
        assert!(matcher.matches("prometheus"));
        assert!(!matcher.matches("grafana"));
        
        let matcher = LabelMatcher::Regex("prom.*".to_string());
        assert!(matcher.matches("prometheus"));
        assert!(matcher.matches("prom"));
        assert!(!matcher.matches("grafana"));
    }

    #[test]
    fn test_intersect_series_ids() {
        let a = vec![1, 2, 3, 4];
        let b = vec![3, 4, 5, 6];
        
        let result = intersect_series_ids(&a, &b);
        assert!(result.contains(&3));
        assert!(result.contains(&4));
        assert_eq!(result.len(), 2);
    }
}
