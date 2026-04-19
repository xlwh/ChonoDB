use crate::error::{Error, Result};
use crate::memstore::MemStore;
use crate::model::{Label, Labels, Sample, TimeSeries, TimeSeriesId};
use crate::query::{QueryPlan, QueryResult};
use crate::query::planner::{PlanType, VectorQueryPlan, MatrixQueryPlan, CallPlan, SubqueryPlan};
use crate::query::parser::Function;
use crate::query::parallel::{ParallelQueryExecutor, ParallelConfig, ParallelContext};
use crate::query::cache::{ThreadSafeQueryCache, CacheConfig, CacheKey};
use crate::columnstore::DownsampleLevel;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetOp {
    And,
    Or,
    Unless,
}

#[derive(Clone)]
pub struct QueryExecutor {
    memstore: Arc<MemStore>,
    parallel_ctx: Arc<parking_lot::RwLock<ParallelContext>>,
    cache: Option<ThreadSafeQueryCache<QueryResult>>,
}

pub struct ExecutionContext {
    pub start: i64,
    pub end: i64,
    pub step: i64,
}

impl QueryExecutor {
    pub fn new(memstore: Arc<MemStore>) -> Self {
        let config = ParallelConfig::default();
        let parallel_ctx = Arc::new(parking_lot::RwLock::new(ParallelContext::new(config)));
        Self {
            memstore,
            parallel_ctx,
            cache: None,
        }
    }

    pub fn with_parallel_config(memstore: Arc<MemStore>, config: ParallelConfig) -> Self {
        let parallel_ctx = Arc::new(parking_lot::RwLock::new(ParallelContext::new(config)));
        Self {
            memstore,
            parallel_ctx,
            cache: None,
        }
    }

    pub fn with_cache(memstore: Arc<MemStore>, cache_config: CacheConfig) -> Self {
        let config = ParallelConfig::default();
        let parallel_ctx = Arc::new(parking_lot::RwLock::new(ParallelContext::new(config)));
        let cache = ThreadSafeQueryCache::new(cache_config);
        Self {
            memstore,
            parallel_ctx,
            cache: Some(cache),
        }
    }

    pub fn with_cache_and_parallel(
        memstore: Arc<MemStore>,
        parallel_config: ParallelConfig,
        cache_config: CacheConfig,
    ) -> Self {
        let parallel_ctx = Arc::new(parking_lot::RwLock::new(ParallelContext::new(parallel_config)));
        let cache = ThreadSafeQueryCache::new(cache_config);
        Self {
            memstore,
            parallel_ctx,
            cache: Some(cache),
        }
    }

    pub fn enable_cache(&mut self, config: CacheConfig) {
        self.cache = Some(ThreadSafeQueryCache::new(config));
    }

    pub fn disable_cache(&mut self) {
        self.cache = None;
    }

    pub fn cache_stats(&self) -> Option<crate::query::cache::CacheStats> {
        self.cache.as_ref().map(|c| c.stats())
    }

    pub fn clear_cache(&self) {
        if let Some(ref cache) = self.cache {
            cache.clear();
        }
    }

    pub async fn execute(&self, plan: &QueryPlan) -> Result<QueryResult> {
        self.execute_with_query(plan, None).await
    }

    pub async fn execute_with_query(&self, plan: &QueryPlan, query_str: Option<&str>) -> Result<QueryResult> {
        let ctx = ExecutionContext {
            start: plan.start,
            end: plan.end,
            step: plan.step,
        };
        
        if let Some(ref cache) = self.cache {
            if let Some(query) = query_str {
                let cache_key = CacheKey::new(query.to_string(), plan.start, plan.end, plan.step);
                
                if let Some(cached_result) = cache.get(&cache_key) {
                    tracing::debug!("Cache hit for query: {}", query);
                    return Ok(cached_result);
                }
                
                tracing::debug!("Cache miss for query: {}", query);
            }
        }
        
        let query_range = plan.end - plan.start;
        let downsample_level = DownsampleLevel::from_query_range(query_range);
        
        tracing::info!("Query range: {}ms, using downsample level: {:?}", query_range, downsample_level);
        
        let series = self.execute_plan(&plan.plan_type, &ctx).await?;
        let result = QueryResult::new(series, plan.start, plan.end, plan.step);
        
        if let Some(ref cache) = self.cache {
            if let Some(query) = query_str {
                let cache_key = CacheKey::new(query.to_string(), plan.start, plan.end, plan.step);
                cache.set(cache_key, result.clone());
                tracing::debug!("Cached result for query: {}", query);
            }
        }
        
        Ok(result)
    }

    fn execute_plan<'a>(&'a self, plan: &'a PlanType, ctx: &'a ExecutionContext) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<TimeSeries>>> + Send + 'a>> {
        Box::pin(async move {
            match plan {
                PlanType::VectorQuery(vq) => self.execute_vector_query(vq, ctx).await,
                PlanType::MatrixQuery(mq) => self.execute_matrix_query(mq, ctx).await,
                PlanType::Subquery(sq) => self.execute_subquery(sq, ctx).await,
                PlanType::Call(call) => self.execute_call(call, ctx).await,
                PlanType::BinaryExpr(bin) => self.execute_binary_expr(bin, ctx).await,
                PlanType::UnaryExpr(unary) => self.execute_unary_expr(unary, ctx).await,
                PlanType::Aggregation(agg) => self.execute_aggregation(agg, ctx).await,
            }
        })
    }

    async fn execute_vector_query(&self, plan: &VectorQueryPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        let matchers: Vec<(String, String)> = plan.matchers.clone();

        // Handle @ modifier
        let (query_start, query_end) = if let Some(at_timestamp) = plan.at {
            let timestamp = if at_timestamp == -1 {
                // @ start()
                ctx.start
            } else if at_timestamp == -2 {
                // @ end()
                ctx.end
            } else {
                // @ timestamp
                at_timestamp
            };
            // For @ modifier, query a single point in time
            (timestamp, timestamp)
        } else {
            (ctx.start, ctx.end)
        };

        // Handle offset modifier
        let (final_start, final_end) = if let Some(offset_ms) = plan.offset {
            // Apply offset: positive offset looks back in time, negative looks forward
            (query_start - offset_ms, query_end - offset_ms)
        } else {
            (query_start, query_end)
        };

        let query_range = final_end - final_start;
        let downsample_level = DownsampleLevel::from_query_range(query_range);

        let series_ids = self.memstore.find_series(&matchers)?;

        if series_ids.is_empty() {
            return Ok(Vec::new());
        }

        let should_parallel = {
            let parallel_ctx = self.parallel_ctx.read();
            parallel_ctx.should_use_parallel(series_ids.len(), query_range)
        };

        if should_parallel {
            self.execute_vector_query_parallel(series_ids, final_start, final_end, downsample_level).await
        } else {
            self.memstore.query_with_downsample(&matchers, final_start, final_end, downsample_level)
        }
    }
    
    async fn execute_vector_query_parallel(
        &self,
        series_ids: Vec<TimeSeriesId>,
        start: i64,
        end: i64,
        downsample_level: DownsampleLevel,
    ) -> Result<Vec<TimeSeries>> {
        let executor = {
            let parallel_ctx = self.parallel_ctx.read();
            parallel_ctx.executor.clone()
        };
        
        let memstore = self.memstore.clone();
        
        let process_fn = move |series_id: TimeSeriesId| {
            let memstore = memstore.clone();
            async move {
                if let Some(mut ts) = memstore.get_series(series_id) {
                    let samples: Vec<Sample> = ts.samples.drain(..).collect();
                    let filtered: Vec<Sample> = samples.into_iter()
                        .filter(|s| s.timestamp >= start && s.timestamp <= end)
                        .collect();
                    
                    if !filtered.is_empty() {
                        let downsampled = Self::apply_downsampling_to_samples(filtered, downsample_level);
                        if !downsampled.is_empty() {
                            ts.samples = downsampled;
                            return Ok(Some(ts));
                        }
                    }
                }
                Ok(None)
            }
        };
        
        let (results, stats) = executor.execute_series_parallel(series_ids, process_fn).await?;
        
        {
            let mut parallel_ctx = self.parallel_ctx.write();
            parallel_ctx.record_stats(stats);
            parallel_ctx.adjust_concurrency();
        }
        
        Ok(results)
    }
    
    fn apply_downsampling_to_samples(samples: Vec<Sample>, level: DownsampleLevel) -> Vec<Sample> {
        if samples.is_empty() || level == DownsampleLevel::L0 {
            return samples;
        }
        
        let resolution = level.resolution_ms();
        if resolution == 0 {
            return samples;
        }
        
        let mut downsampled = Vec::new();
        let mut current_window = samples[0].timestamp - (samples[0].timestamp % resolution);
        let mut window_samples = Vec::new();
        
        for sample in samples {
            let sample_window = sample.timestamp - (sample.timestamp % resolution);
            
            if sample_window != current_window {
                if !window_samples.is_empty() {
                    let sum: f64 = window_samples.iter().map(|s: &Sample| s.value).sum();
                    let avg = sum / window_samples.len() as f64;
                    downsampled.push(Sample::new(current_window, avg));
                }
                
                current_window = sample_window;
                window_samples.clear();
            }
            
            window_samples.push(sample);
        }
        
        if !window_samples.is_empty() {
            let sum: f64 = window_samples.iter().map(|s: &Sample| s.value).sum();
            let avg = sum / window_samples.len() as f64;
            downsampled.push(Sample::new(current_window, avg));
        }
        
        downsampled
    }

    async fn execute_matrix_query(&self, plan: &MatrixQueryPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        self.execute_vector_query(&plan.vector_plan, ctx).await
    }

    async fn execute_subquery(&self, plan: &SubqueryPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        // Subquery executes the inner expression over a range with a specific resolution
        // For example: rate(http_requests_total[5m])[30m:1m] means:
        // - Execute rate(http_requests_total[5m]) every 1m
        // - Over the last 30m

        let mut results = Vec::new();

        // Calculate the number of steps
        let num_steps = (plan.range / plan.resolution) as usize;

        // Execute the inner query at each step
        for i in 0..=num_steps {
            let timestamp = ctx.end - plan.range + (i as i64 * plan.resolution);

            // Create a context for this step
            let step_ctx = ExecutionContext {
                start: timestamp,
                end: timestamp,
                step: plan.resolution,
            };

            // Execute the inner expression
            let step_results = self.execute_plan(&plan.expr.plan_type, &step_ctx).await?;

            // Add results with the current timestamp
            for ts in step_results {
                let mut new_ts = TimeSeries::new(ts.id, ts.labels.clone());
                for sample in &ts.samples {
                    new_ts.add_sample(Sample::new(timestamp, sample.value));
                }
                results.push(new_ts);
            }
        }

        Ok(results)
    }

    async fn execute_call(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        match plan.func {
            // Range vector functions
            Function::Rate => self.execute_rate(plan, ctx).await,
            Function::Irate => self.execute_irate(plan, ctx).await,
            Function::Increase => self.execute_increase(plan, ctx).await,
            Function::Delta => self.execute_delta(plan, ctx).await,
            Function::Idelta => self.execute_idelta(plan, ctx).await,
            Function::Resets => self.execute_resets(plan, ctx).await,
            Function::Changes => self.execute_changes(plan, ctx).await,
            Function::Deriv => self.execute_deriv(plan, ctx).await,
            Function::PredictLinear => self.execute_predict_linear(plan, ctx).await,
            Function::HoltWinters => self.execute_holt_winters(plan, ctx).await,
            
            // Aggregation functions
            Function::Sum => self.execute_sum(plan, ctx).await,
            Function::Avg => self.execute_avg(plan, ctx).await,
            Function::Min => self.execute_min(plan, ctx).await,
            Function::Max => self.execute_max(plan, ctx).await,
            Function::Count => self.execute_count(plan, ctx).await,
            Function::Stddev => self.execute_stddev(plan, ctx).await,
            Function::Stdvar => self.execute_stdvar(plan, ctx).await,
            Function::TopK => self.execute_topk(plan, ctx).await,
            Function::BottomK => self.execute_bottomk(plan, ctx).await,
            Function::Quantile => self.execute_quantile(plan, ctx).await,
            Function::CountValues => self.execute_count_values(plan, ctx).await,
            
            // Math functions
            Function::Abs => self.execute_math_unary(plan, ctx, |v| v.abs()).await,
            Function::Ceil => self.execute_math_unary(plan, ctx, |v| v.ceil()).await,
            Function::Floor => self.execute_math_unary(plan, ctx, |v| v.floor()).await,
            Function::Round => self.execute_round(plan, ctx).await,
            Function::Clamp => self.execute_clamp(plan, ctx).await,
            Function::ClampMax => self.execute_clamp_max(plan, ctx).await,
            Function::ClampMin => self.execute_clamp_min(plan, ctx).await,
            Function::Exp => self.execute_math_unary(plan, ctx, |v| v.exp()).await,
            Function::Ln => self.execute_math_unary(plan, ctx, |v| v.ln()).await,
            Function::Log2 => self.execute_math_unary(plan, ctx, |v| v.log2()).await,
            Function::Log10 => self.execute_math_unary(plan, ctx, |v| v.log10()).await,
            Function::Sqrt => self.execute_math_unary(plan, ctx, |v| v.sqrt()).await,
            
            // Trigonometric functions
            Function::Sin => self.execute_math_unary(plan, ctx, |v| v.sin()).await,
            Function::Cos => self.execute_math_unary(plan, ctx, |v| v.cos()).await,
            Function::Tan => self.execute_math_unary(plan, ctx, |v| v.tan()).await,
            Function::Asin => self.execute_math_unary(plan, ctx, |v| v.asin()).await,
            Function::Acos => self.execute_math_unary(plan, ctx, |v| v.acos()).await,
            Function::Atan => self.execute_math_unary(plan, ctx, |v| v.atan()).await,
            Function::Sinh => self.execute_math_unary(plan, ctx, |v| v.sinh()).await,
            Function::Cosh => self.execute_math_unary(plan, ctx, |v| v.cosh()).await,
            Function::Tanh => self.execute_math_unary(plan, ctx, |v| v.tanh()).await,
            Function::Atanh => self.execute_math_unary(plan, ctx, |v| v.atanh()).await,
            
            // Time functions
            Function::Time => self.execute_time(ctx).await,
            Function::Timestamp => self.execute_timestamp(plan, ctx).await,
            
            // Label functions
            Function::LabelReplace => self.execute_label_replace(plan, ctx).await,
            Function::LabelJoin => self.execute_label_join(plan, ctx).await,
            
            // Sort functions
            Function::Sort => self.execute_sort(plan, ctx, false).await,
            Function::SortDesc => self.execute_sort(plan, ctx, true).await,
            
            // Other functions
            Function::Absent => self.execute_absent(plan, ctx).await,
            Function::AbsentOverTime => self.execute_absent_over_time(plan, ctx).await,
            Function::PresentOverTime => self.execute_present_over_time(plan, ctx).await,
            Function::HistogramQuantile => self.execute_histogram_quantile(plan, ctx).await,
            
            // Scalar functions
            Function::Scalar => self.execute_scalar(plan, ctx).await,
            Function::Vector => self.execute_vector(plan, ctx).await,
            
            _ => Err(Error::InvalidData(format!("Function {} not implemented", plan.func.name()))),
        }
    }

    async fn execute_rate(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("rate() requires exactly one argument".to_string()));
        }
        
        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;
        
        let mut result = Vec::new();
        for ts in series {
            let mut rate_series = ts.clone();
            rate_series.samples = self.calculate_rate(&ts.samples, ctx.step)?;
            if !rate_series.samples.is_empty() {
                result.push(rate_series);
            }
        }
        
        Ok(result)
    }

    async fn execute_sum(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("sum() requires exactly one argument".to_string()));
        }
        
        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;
        
        if series.is_empty() {
            return Ok(Vec::new());
        }
        
        if series.len() > 20 {
            let executor = {
                let parallel_ctx = self.parallel_ctx.read();
                parallel_ctx.executor.clone()
            };
            
            let aggregate_fn = |batch: Vec<TimeSeries>| {
                async move {
                    let mut timestamp_sums = HashMap::new();
                    
                    for ts in &batch {
                        for sample in &ts.samples {
                            *timestamp_sums.entry(sample.timestamp).or_insert(0.0) += sample.value;
                        }
                    }
                    
                    let mut sum_series = TimeSeries::new(0, batch[0].labels.clone());
                    let mut timestamps: Vec<_> = timestamp_sums.keys().collect();
                    timestamps.sort();
                    
                    for timestamp in timestamps {
                        sum_series.add_sample(Sample::new(*timestamp, timestamp_sums[timestamp]));
                    }
                    
                    Ok(sum_series)
                }
            };
            
            let (result, stats) = executor.parallel_aggregate(series, aggregate_fn).await?;
            
            {
                let mut parallel_ctx = self.parallel_ctx.write();
                parallel_ctx.record_stats(stats);
            }
            
            Ok(vec![result])
        } else {
            let mut timestamp_sums = HashMap::new();
            
            for ts in &series {
                for sample in &ts.samples {
                    *timestamp_sums.entry(sample.timestamp).or_insert(0.0) += sample.value;
                }
            }
            
            let mut sum_series = TimeSeries::new(0, series[0].labels.clone());
            
            let mut timestamps: Vec<_> = timestamp_sums.keys().collect();
            timestamps.sort();
            
            for timestamp in timestamps {
                sum_series.add_sample(Sample::new(*timestamp, timestamp_sums[timestamp]));
            }
            
            Ok(vec![sum_series])
        }
    }

    async fn execute_avg(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("avg() requires exactly one argument".to_string()));
        }
        
        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;
        
        if series.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut sum_series = TimeSeries::new(0, series[0].labels.clone());
        let mut count = 0;
        
        for ts in &series {
            for sample in &ts.samples {
                sum_series.add_sample(sample.clone());
                count += 1;
            }
        }
        
        if count > 0 {
            let avg = sum_series.samples.iter().map(|s| s.value).sum::<f64>() / count as f64;
            sum_series.samples = vec![Sample::new(ctx.start, avg)];
            Ok(vec![sum_series])
        } else {
            Ok(Vec::new())
        }
    }

    async fn execute_min(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("min() requires exactly one argument".to_string()));
        }
        
        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;
        
        if series.is_empty() {
            return Ok(Vec::new());
        }
        
        let min = series.iter()
            .flat_map(|ts| ts.samples.iter())
            .map(|s| s.value)
            .fold(f64::MAX, f64::min);
        
        let mut min_series = TimeSeries::new(0, series[0].labels.clone());
        min_series.add_sample(Sample::new(ctx.start, min));
        
        Ok(vec![min_series])
    }

    async fn execute_max(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("max() requires exactly one argument".to_string()));
        }
        
        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;
        
        if series.is_empty() {
            return Ok(Vec::new());
        }
        
        let max = series.iter()
            .flat_map(|ts| ts.samples.iter())
            .map(|s| s.value)
            .fold(f64::MIN, f64::max);
        
        let mut max_series = TimeSeries::new(0, series[0].labels.clone());
        max_series.add_sample(Sample::new(ctx.start, max));
        
        Ok(vec![max_series])
    }

    async fn execute_count(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("count() requires exactly one argument".to_string()));
        }
        
        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;
        
        let count = series.iter().map(|ts| ts.samples.len()).sum::<usize>();
        
        if count > 0 {
            let mut count_series = TimeSeries::new(0, series[0].labels.clone());
            count_series.add_sample(Sample::new(ctx.start, count as f64));
            Ok(vec![count_series])
        } else {
            Ok(Vec::new())
        }
    }

    async fn execute_binary_expr(&self, plan: &crate::query::planner::BinaryExprPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        use crate::query::parser::BinaryOp;
        
        let lhs_series = self.execute_plan(&plan.lhs.plan_type, ctx).await?;
        let rhs_series = self.execute_plan(&plan.rhs.plan_type, ctx).await?;
        
        match plan.op {
            BinaryOp::Add => self.execute_binary_op(lhs_series, rhs_series, |a, b| a + b),
            BinaryOp::Sub => self.execute_binary_op(lhs_series, rhs_series, |a, b| a - b),
            BinaryOp::Mul => self.execute_binary_op(lhs_series, rhs_series, |a, b| a * b),
            BinaryOp::Div => self.execute_binary_op(lhs_series, rhs_series, |a, b| {
                if b == 0.0 { f64::NAN } else { a / b }
            }),
            BinaryOp::Mod => self.execute_binary_op(lhs_series, rhs_series, |a, b| {
                if b == 0.0 { f64::NAN } else { a % b }
            }),
            BinaryOp::Pow => self.execute_binary_op(lhs_series, rhs_series, |a, b| a.powf(b)),
            BinaryOp::Eq => self.execute_comparison_op(lhs_series, rhs_series, |a, b| if a == b { 1.0 } else { 0.0 }),
            BinaryOp::Ne => self.execute_comparison_op(lhs_series, rhs_series, |a, b| if a != b { 1.0 } else { 0.0 }),
            BinaryOp::Lt => self.execute_comparison_op(lhs_series, rhs_series, |a, b| if a < b { 1.0 } else { 0.0 }),
            BinaryOp::Le => self.execute_comparison_op(lhs_series, rhs_series, |a, b| if a <= b { 1.0 } else { 0.0 }),
            BinaryOp::Gt => self.execute_comparison_op(lhs_series, rhs_series, |a, b| if a > b { 1.0 } else { 0.0 }),
            BinaryOp::Ge => self.execute_comparison_op(lhs_series, rhs_series, |a, b| if a >= b { 1.0 } else { 0.0 }),
            BinaryOp::And => self.execute_set_op(lhs_series, rhs_series, SetOp::And),
            BinaryOp::Or => self.execute_set_op(lhs_series, rhs_series, SetOp::Or),
            BinaryOp::Unless => self.execute_set_op(lhs_series, rhs_series, SetOp::Unless),
        }
    }
    
    fn execute_binary_op<F>(&self, lhs: Vec<TimeSeries>, rhs: Vec<TimeSeries>, op: F) -> Result<Vec<TimeSeries>>
    where
        F: Fn(f64, f64) -> f64,
    {
        let mut result = Vec::new();
        
        // Simple implementation: match by series ID
        for lhs_ts in &lhs {
            for rhs_ts in &rhs {
                if lhs_ts.id == rhs_ts.id {
                    let mut new_ts = lhs_ts.clone();
                    new_ts.samples = self.apply_binary_op_to_samples(&lhs_ts.samples, &rhs_ts.samples, &op);
                    if !new_ts.samples.is_empty() {
                        result.push(new_ts);
                    }
                }
            }
        }
        
        // If no matching series, try one-to-one matching by position
        if result.is_empty() && lhs.len() == rhs.len() {
            for (lhs_ts, rhs_ts) in lhs.iter().zip(rhs.iter()) {
                let mut new_ts = lhs_ts.clone();
                new_ts.samples = self.apply_binary_op_to_samples(&lhs_ts.samples, &rhs_ts.samples, &op);
                if !new_ts.samples.is_empty() {
                    result.push(new_ts);
                }
            }
        }
        
        Ok(result)
    }
    
    fn apply_binary_op_to_samples<F>(&self, lhs: &[Sample], rhs: &[Sample], op: &F) -> Vec<Sample>
    where
        F: Fn(f64, f64) -> f64,
    {
        let mut result = Vec::new();
        
        // Match samples by timestamp
        for lhs_sample in lhs {
            if let Some(rhs_sample) = rhs.iter().find(|s| s.timestamp == lhs_sample.timestamp) {
                result.push(Sample::new(lhs_sample.timestamp, op(lhs_sample.value, rhs_sample.value)));
            }
        }
        
        result
    }
    
    fn execute_comparison_op<F>(&self, lhs: Vec<TimeSeries>, rhs: Vec<TimeSeries>, op: F) -> Result<Vec<TimeSeries>>
    where
        F: Fn(f64, f64) -> f64,
    {
        self.execute_binary_op(lhs, rhs, op)
    }
    
    fn execute_set_op(&self, lhs: Vec<TimeSeries>, rhs: Vec<TimeSeries>, op: SetOp) -> Result<Vec<TimeSeries>> {
        use std::collections::HashSet;
        
        let lhs_ids: HashSet<TimeSeriesId> = lhs.iter().map(|ts| ts.id).collect();
        let rhs_ids: HashSet<TimeSeriesId> = rhs.iter().map(|ts| ts.id).collect();
        
        let result_ids: HashSet<TimeSeriesId> = match op {
            SetOp::And => lhs_ids.intersection(&rhs_ids).copied().collect(),
            SetOp::Or => lhs_ids.union(&rhs_ids).copied().collect(),
            SetOp::Unless => lhs_ids.difference(&rhs_ids).copied().collect(),
        };
        
        let mut result = Vec::new();
        
        for ts in lhs {
            if result_ids.contains(&ts.id) {
                result.push(ts);
            }
        }
        
        // For Or, also add series from rhs that are not in lhs
        if op == SetOp::Or {
            for ts in rhs {
                if result_ids.contains(&ts.id) && !lhs_ids.contains(&ts.id) {
                    result.push(ts);
                }
            }
        }
        
        Ok(result)
    }

    async fn execute_unary_expr(&self, plan: &crate::query::planner::UnaryExprPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        use crate::query::parser::UnaryOp;
        
        let series = self.execute_plan(&plan.expr.plan_type, ctx).await?;
        
        let mut result = Vec::new();
        for ts in series {
            let mut new_ts = ts.clone();
            new_ts.samples = ts.samples.iter()
                .map(|s| {
                    let new_value = match plan.op {
                        UnaryOp::Add => s.value,
                        UnaryOp::Sub => -s.value,
                    };
                    Sample::new(s.timestamp, new_value)
                })
                .collect();
            result.push(new_ts);
        }
        
        Ok(result)
    }

    async fn execute_aggregation(&self, plan: &crate::query::planner::AggregationPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        let series = self.execute_plan(&plan.expr.plan_type, ctx).await?;
        
        if series.is_empty() {
            return Ok(Vec::new());
        }
        
        // Group series by the grouping labels
        let mut groups: HashMap<Vec<(String, String)>, Vec<TimeSeries>> = HashMap::new();
        
        for ts in series {
            let group_key = if plan.without {
                // Without: group by all labels except the specified ones
                ts.labels.iter()
                    .filter(|l| !plan.grouping.contains(&l.name))
                    .map(|l| (l.name.clone(), l.value.clone()))
                    .collect()
            } else {
                // By: group by the specified labels
                ts.labels.iter()
                    .filter(|l| plan.grouping.contains(&l.name))
                    .map(|l| (l.name.clone(), l.value.clone()))
                    .collect()
            };
            
            groups.entry(group_key).or_insert_with(Vec::new).push(ts);
        }
        
        // Aggregate each group
        let mut result = Vec::new();
        for (group_key, group_series) in groups {
            let aggregated = self.aggregate_group(&plan.op, group_key, group_series, ctx)?;
            result.push(aggregated);
        }
        
        Ok(result)
    }
    
    fn aggregate_group(&self, op: &Function, group_key: Vec<(String, String)>, series: Vec<TimeSeries>, _ctx: &ExecutionContext) -> Result<TimeSeries> {
        use crate::query::parser::Function;
        
        // Create labels from group key
        let labels: Labels = group_key.iter()
            .map(|(name, value)| Label::new(name.clone(), value.clone()))
            .collect();
        
        let mut result_ts = TimeSeries::new(0, labels);
        
        // Collect all samples from all series in the group
        let all_samples: Vec<Sample> = series.iter()
            .flat_map(|ts| ts.samples.clone())
            .collect();
        
        if all_samples.is_empty() {
            return Ok(result_ts);
        }
        
        // Group samples by timestamp for aggregation
        let mut samples_by_time: HashMap<i64, Vec<f64>> = HashMap::new();
        for sample in all_samples {
            samples_by_time.entry(sample.timestamp).or_insert_with(Vec::new).push(sample.value);
        }
        
        // Aggregate each timestamp
        for (timestamp, values) in samples_by_time {
            let aggregated_value = match op {
                Function::Sum => values.iter().sum(),
                Function::Avg => values.iter().sum::<f64>() / values.len() as f64,
                Function::Min => values.iter().copied().fold(f64::MAX, f64::min),
                Function::Max => values.iter().copied().fold(f64::MIN, f64::max),
                Function::Count => values.len() as f64,
                Function::Stddev => {
                    let mean = values.iter().sum::<f64>() / values.len() as f64;
                    let variance = values.iter()
                        .map(|v| (v - mean).powi(2))
                        .sum::<f64>() / values.len() as f64;
                    variance.sqrt()
                }
                Function::Stdvar => {
                    let mean = values.iter().sum::<f64>() / values.len() as f64;
                    values.iter()
                        .map(|v| (v - mean).powi(2))
                        .sum::<f64>() / values.len() as f64
                }
                _ => return Err(Error::InvalidData(format!("Unsupported aggregation function: {:?}", op))),
            };
            
            result_ts.add_sample(Sample::new(timestamp, aggregated_value));
        }
        
        Ok(result_ts)
    }

    // ========== Range Vector Functions ==========

    async fn execute_irate(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("irate() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut irate_series = ts.clone();
            irate_series.samples = self.calculate_irate(&ts.samples)?;
            if !irate_series.samples.is_empty() {
                result.push(irate_series);
            }
        }

        Ok(result)
    }

    async fn execute_increase(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("increase() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut increase_series = ts.clone();
            increase_series.samples = self.calculate_increase(&ts.samples)?;
            if !increase_series.samples.is_empty() {
                result.push(increase_series);
            }
        }

        Ok(result)
    }

    async fn execute_delta(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("delta() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut delta_series = ts.clone();
            delta_series.samples = self.calculate_delta(&ts.samples)?;
            if !delta_series.samples.is_empty() {
                result.push(delta_series);
            }
        }

        Ok(result)
    }

    async fn execute_idelta(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("idelta() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut idelta_series = ts.clone();
            idelta_series.samples = self.calculate_idelta(&ts.samples)?;
            if !idelta_series.samples.is_empty() {
                result.push(idelta_series);
            }
        }

        Ok(result)
    }

    async fn execute_resets(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("resets() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let resets = self.calculate_resets(&ts.samples);
            let mut resets_series = TimeSeries::new(ts.id, ts.labels.clone());
            resets_series.add_sample(Sample::new(ctx.end, resets as f64));
            result.push(resets_series);
        }

        Ok(result)
    }

    async fn execute_changes(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("changes() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let changes = self.calculate_changes(&ts.samples);
            let mut changes_series = TimeSeries::new(ts.id, ts.labels.clone());
            changes_series.add_sample(Sample::new(ctx.end, changes as f64));
            result.push(changes_series);
        }

        Ok(result)
    }

    async fn execute_deriv(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("deriv() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut deriv_series = ts.clone();
            deriv_series.samples = self.calculate_deriv(&ts.samples)?;
            if !deriv_series.samples.is_empty() {
                result.push(deriv_series);
            }
        }

        Ok(result)
    }

    async fn execute_predict_linear(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 2 {
            return Err(Error::InvalidData("predict_linear() requires exactly two arguments".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        // Get prediction time from second argument
        let prediction_time = if let crate::query::planner::PlanType::VectorQuery(_) = &plan.args[1].plan_type {
            // For simplicity, assume a fixed prediction time
            60.0
        } else {
            60.0
        };

        let mut result = Vec::new();
        for ts in series {
            if let Some(predicted) = self.calculate_predict_linear(&ts.samples, prediction_time) {
                let mut predict_series = TimeSeries::new(ts.id, ts.labels.clone());
                predict_series.add_sample(Sample::new(ctx.end, predicted));
                result.push(predict_series);
            }
        }

        Ok(result)
    }

    async fn execute_holt_winters(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 3 {
            return Err(Error::InvalidData("holt_winters() requires exactly three arguments".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut hw_series = ts.clone();
            hw_series.samples = self.calculate_holt_winters(&ts.samples, 0.3, 0.1)?;
            if !hw_series.samples.is_empty() {
                result.push(hw_series);
            }
        }

        Ok(result)
    }

    // ========== Aggregation Functions ==========

    async fn execute_stddev(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("stddev() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        if series.is_empty() {
            return Ok(Vec::new());
        }

        let values: Vec<f64> = series.iter()
            .flat_map(|ts| ts.samples.iter().map(|s| s.value))
            .collect();

        if values.is_empty() {
            return Ok(Vec::new());
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        let stddev = variance.sqrt();

        let mut stddev_series = TimeSeries::new(0, series[0].labels.clone());
        stddev_series.add_sample(Sample::new(ctx.start, stddev));

        Ok(vec![stddev_series])
    }

    async fn execute_stdvar(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("stdvar() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        if series.is_empty() {
            return Ok(Vec::new());
        }

        let values: Vec<f64> = series.iter()
            .flat_map(|ts| ts.samples.iter().map(|s| s.value))
            .collect();

        if values.is_empty() {
            return Ok(Vec::new());
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;

        let mut stdvar_series = TimeSeries::new(0, series[0].labels.clone());
        stdvar_series.add_sample(Sample::new(ctx.start, variance));

        Ok(vec![stdvar_series])
    }

    async fn execute_topk(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 2 {
            return Err(Error::InvalidData("topk() requires exactly two arguments: k and expression".to_string()));
        }

        let series = self.execute_plan(&plan.args[1].plan_type, ctx).await?;

        // Get k from plan, default to 0 if not specified
        let k = plan.k.unwrap_or(0);

        // Handle k=0 case
        if k == 0 {
            return Ok(Vec::new());
        }

        let mut all_samples: Vec<(TimeSeriesId, &Sample, &crate::model::Labels)> = Vec::new();
        for ts in &series {
            for sample in &ts.samples {
                all_samples.push((ts.id, sample, &ts.labels));
            }
        }

        // Handle empty data case
        if all_samples.is_empty() {
            return Ok(Vec::new());
        }

        // Sort by value descending
        all_samples.sort_by(|a, b| {
            match b.1.value.partial_cmp(&a.1.value) {
                Some(ordering) => ordering,
                None => std::cmp::Ordering::Equal, // Handle NaN values
            }
        });

        // Take top k (handle k > series count case)
        let topk_samples: Vec<_> = all_samples.into_iter().take(k).collect();

        // Group by series
        let mut result = Vec::new();
        for (id, sample, labels) in topk_samples {
            let mut ts = TimeSeries::new(id, labels.clone());
            ts.add_sample(sample.clone());
            result.push(ts);
        }

        Ok(result)
    }

    async fn execute_bottomk(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 2 {
            return Err(Error::InvalidData("bottomk() requires exactly two arguments: k and expression".to_string()));
        }

        let series = self.execute_plan(&plan.args[1].plan_type, ctx).await?;

        // Get k from plan, default to 0 if not specified
        let k = plan.k.unwrap_or(0);

        // Handle k=0 case
        if k == 0 {
            return Ok(Vec::new());
        }

        let mut all_samples: Vec<(TimeSeriesId, &Sample, &crate::model::Labels)> = Vec::new();
        for ts in &series {
            for sample in &ts.samples {
                all_samples.push((ts.id, sample, &ts.labels));
            }
        }

        // Handle empty data case
        if all_samples.is_empty() {
            return Ok(Vec::new());
        }

        // Sort by value ascending
        all_samples.sort_by(|a, b| {
            match a.1.value.partial_cmp(&b.1.value) {
                Some(ordering) => ordering,
                None => std::cmp::Ordering::Equal, // Handle NaN values
            }
        });

        // Take bottom k (handle k > series count case)
        let bottomk_samples: Vec<_> = all_samples.into_iter().take(k).collect();

        // Group by series
        let mut result = Vec::new();
        for (id, sample, labels) in bottomk_samples {
            let mut ts = TimeSeries::new(id, labels.clone());
            ts.add_sample(sample.clone());
            result.push(ts);
        }

        Ok(result)
    }

    async fn execute_quantile(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 2 {
            return Err(Error::InvalidData("quantile() requires exactly two arguments: quantile and expression".to_string()));
        }

        let series = self.execute_plan(&plan.args[1].plan_type, ctx).await?;

        // Get quantile from plan, default to 0.5 if not specified
        let quantile = plan.quantile.unwrap_or(0.5);

        // Validate quantile range
        if quantile < 0.0 || quantile > 1.0 {
            return Err(Error::InvalidData(format!("quantile must be between 0 and 1, got {}", quantile)));
        }

        if series.is_empty() {
            return Ok(Vec::new());
        }

        let mut values: Vec<f64> = series.iter()
            .flat_map(|ts| ts.samples.iter().map(|s| s.value))
            .collect();

        if values.is_empty() {
            return Ok(Vec::new());
        }

        // Sort values, handling NaN
        values.sort_by(|a, b| {
            match a.partial_cmp(b) {
                Some(ordering) => ordering,
                None => std::cmp::Ordering::Equal,
            }
        });

        // Calculate quantile using linear interpolation
        let n = values.len();
        let result_value = if n == 1 {
            values[0]
        } else if quantile == 0.0 {
            values[0]
        } else if quantile == 1.0 {
            values[n - 1]
        } else {
            // Linear interpolation
            let position = quantile * (n - 1) as f64;
            let lower_index = position.floor() as usize;
            let upper_index = position.ceil() as usize;
            let fraction = position - lower_index as f64;

            if lower_index == upper_index {
                values[lower_index]
            } else {
                values[lower_index] * (1.0 - fraction) + values[upper_index] * fraction
            }
        };

        let mut quantile_series = TimeSeries::new(0, series[0].labels.clone());
        quantile_series.add_sample(Sample::new(ctx.start, result_value));

        Ok(vec![quantile_series])
    }

    async fn execute_count_values(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 2 {
            return Err(Error::InvalidData("count_values() requires exactly 2 arguments: label name and expression".to_string()));
        }

        // Get label name from first argument
        let label_name = match &plan.args[0].plan_type {
            PlanType::VectorQuery(vq) if vq.matchers.len() == 1 => {
                // Try to extract string literal from matchers
                vq.matchers[0].1.clone()
            }
            _ => {
                // For now, use a default label name if we can't extract it properly
                // In a full implementation, we'd parse the literal string from the AST
                "value".to_string()
            }
        };

        let series = self.execute_plan(&plan.args[1].plan_type, ctx).await?;

        // Count occurrences of each value
        let mut value_counts: HashMap<String, u64> = HashMap::new();
        for ts in &series {
            for sample in &ts.samples {
                // Format the value as a string (Prometheus-style)
                let value_str = format_value(sample.value);
                *value_counts.entry(value_str).or_insert(0) += 1;
            }
        }

        // Create result series - one for each unique value
        let mut result = Vec::new();
        for (value, count) in value_counts {
            let labels = vec![Label::new(label_name.clone(), value)];
            let mut ts = TimeSeries::new(0, labels);
            ts.add_sample(Sample::new(ctx.start, count as f64));
            result.push(ts);
        }

        Ok(result)
    }

    // ========== Math Functions ==========

    async fn execute_math_unary<F>(&self, plan: &CallPlan, ctx: &ExecutionContext, f: F) -> Result<Vec<TimeSeries>>
    where
        F: Fn(f64) -> f64,
    {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("Math function requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut new_series = ts.clone();
            new_series.samples = ts.samples.iter()
                .map(|s| Sample::new(s.timestamp, f(s.value)))
                .collect();
            result.push(new_series);
        }

        Ok(result)
    }

    async fn execute_round(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        let decimals = if plan.args.len() == 2 {
            // Get decimals from second argument (simplified to 0)
            0
        } else {
            0
        };

        let multiplier = 10f64.powi(decimals);

        self.execute_math_unary(plan, ctx, |v| (v * multiplier).round() / multiplier).await
    }

    async fn execute_clamp(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 3 {
            return Err(Error::InvalidData("clamp() requires exactly three arguments".to_string()));
        }

        let min = 0.0; // Simplified
        let max = 100.0; // Simplified

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut new_series = ts.clone();
            new_series.samples = ts.samples.iter()
                .map(|s| Sample::new(s.timestamp, s.value.clamp(min, max)))
                .collect();
            result.push(new_series);
        }

        Ok(result)
    }

    async fn execute_clamp_max(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 2 {
            return Err(Error::InvalidData("clamp_max() requires exactly two arguments".to_string()));
        }

        let max = 100.0; // Simplified

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut new_series = ts.clone();
            new_series.samples = ts.samples.iter()
                .map(|s| Sample::new(s.timestamp, s.value.min(max)))
                .collect();
            result.push(new_series);
        }

        Ok(result)
    }

    async fn execute_clamp_min(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 2 {
            return Err(Error::InvalidData("clamp_min() requires exactly two arguments".to_string()));
        }

        let min = 0.0; // Simplified

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut new_series = ts.clone();
            new_series.samples = ts.samples.iter()
                .map(|s| Sample::new(s.timestamp, s.value.max(min)))
                .collect();
            result.push(new_series);
        }

        Ok(result)
    }

    // ========== Time Functions ==========

    async fn execute_time(&self, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        let mut series = TimeSeries::new(0, crate::model::Labels::new());
        series.add_sample(Sample::new(ctx.start, ctx.start as f64 / 1000.0));

        Ok(vec![series])
    }

    async fn execute_timestamp(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("timestamp() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let mut new_series = ts.clone();
            new_series.samples = ts.samples.iter()
                .map(|s| Sample::new(s.timestamp, s.timestamp as f64 / 1000.0))
                .collect();
            result.push(new_series);
        }

        Ok(result)
    }

    // ========== Label Functions ==========

    async fn execute_label_replace(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 5 {
            return Err(Error::InvalidData("label_replace() requires exactly five arguments".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        // Simplified implementation - just return the series
        Ok(series)
    }

    async fn execute_label_join(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() < 4 {
            return Err(Error::InvalidData("label_join() requires at least four arguments".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        // Simplified implementation - just return the series
        Ok(series)
    }

    // ========== Sort Functions ==========

    async fn execute_sort(&self, plan: &CallPlan, ctx: &ExecutionContext, descending: bool) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("sort() requires exactly one argument".to_string()));
        }

        let mut series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        // Sort series by their first sample value
        series.sort_by(|a, b| {
            let a_val = a.samples.first().map(|s| s.value).unwrap_or(0.0);
            let b_val = b.samples.first().map(|s| s.value).unwrap_or(0.0);
            if descending {
                b_val.partial_cmp(&a_val).unwrap()
            } else {
                a_val.partial_cmp(&b_val).unwrap()
            }
        });

        Ok(series)
    }

    // ========== Other Functions ==========

    async fn execute_absent(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("absent() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        if series.is_empty() || series.iter().all(|ts| ts.samples.is_empty()) {
            // Return 1 if no data
            let mut absent_series = TimeSeries::new(0, crate::model::Labels::new());
            absent_series.add_sample(Sample::new(ctx.start, 1.0));
            Ok(vec![absent_series])
        } else {
            // Return empty if data exists
            Ok(Vec::new())
        }
    }

    async fn execute_absent_over_time(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("absent_over_time() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            if ts.samples.is_empty() {
                let mut absent_series = TimeSeries::new(ts.id, ts.labels.clone());
                absent_series.add_sample(Sample::new(ctx.end, 1.0));
                result.push(absent_series);
            }
        }

        Ok(result)
    }

    async fn execute_present_over_time(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("present_over_time() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        let mut result = Vec::new();
        for ts in series {
            let count = ts.samples.len() as f64;
            let mut present_series = TimeSeries::new(ts.id, ts.labels.clone());
            present_series.add_sample(Sample::new(ctx.end, count));
            result.push(present_series);
        }

        Ok(result)
    }

    async fn execute_histogram_quantile(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 2 {
            return Err(Error::InvalidData("histogram_quantile() requires exactly two arguments".to_string()));
        }

        let series = self.execute_plan(&plan.args[1].plan_type, ctx).await?;

        // Get quantile from first argument (simplified to 0.99)
        let quantile = 0.99;

        let mut result = Vec::new();
        for ts in series {
            // Simplified histogram quantile calculation
            let mut values: Vec<f64> = ts.samples.iter().map(|s| s.value).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap());

            if !values.is_empty() {
                let index = (quantile * (values.len() - 1) as f64) as usize;
                let result_value = values[index];

                let mut quantile_series = TimeSeries::new(ts.id, ts.labels.clone());
                quantile_series.add_sample(Sample::new(ctx.end, result_value));
                result.push(quantile_series);
            }
        }

        Ok(result)
    }

    // ========== Helper Functions ==========

    fn calculate_rate(&self, samples: &[Sample], _step: i64) -> Result<Vec<Sample>> {
        if samples.len() < 2 {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();
        for i in 1..samples.len() {
            let prev = &samples[i-1];
            let curr = &samples[i];

            let time_delta = (curr.timestamp - prev.timestamp) as f64;
            if time_delta <= 0.0 {
                continue;
            }

            let value_delta = curr.value - prev.value;

            // 处理计数器重置：如果值为负，说明计数器重置了，跳过这个样本对
            // 或者可以认为是新计数器的值，但 rate 应该为 0
            if value_delta < 0.0 {
                // 计数器重置，跳过这个负值
                continue;
            }

            let rate = value_delta / time_delta;

            result.push(Sample::new(curr.timestamp, rate));
        }

        Ok(result)
    }

    fn calculate_irate(&self, samples: &[Sample]) -> Result<Vec<Sample>> {
        if samples.len() < 2 {
            return Ok(Vec::new());
        }

        // irate uses the last two samples
        let prev = &samples[samples.len() - 2];
        let curr = &samples[samples.len() - 1];

        let time_delta = (curr.timestamp - prev.timestamp) as f64;
        if time_delta <= 0.0 {
            return Ok(Vec::new());
        }

        let value_delta = curr.value - prev.value;
        let rate = value_delta / time_delta;

        Ok(vec![Sample::new(curr.timestamp, rate)])
    }

    fn calculate_increase(&self, samples: &[Sample]) -> Result<Vec<Sample>> {
        if samples.len() < 2 {
            return Ok(Vec::new());
        }

        let first = &samples[0];
        let last = &samples[samples.len() - 1];

        let time_delta = (last.timestamp - first.timestamp) as f64;
        if time_delta <= 0.0 {
            return Ok(Vec::new());
        }

        let value_delta = last.value - first.value;
        let increase = value_delta;

        Ok(vec![Sample::new(last.timestamp, increase)])
    }

    fn calculate_delta(&self, samples: &[Sample]) -> Result<Vec<Sample>> {
        if samples.len() < 2 {
            return Ok(Vec::new());
        }

        let first = &samples[0];
        let last = &samples[samples.len() - 1];

        let value_delta = last.value - first.value;

        Ok(vec![Sample::new(last.timestamp, value_delta)])
    }

    fn calculate_idelta(&self, samples: &[Sample]) -> Result<Vec<Sample>> {
        if samples.len() < 2 {
            return Ok(Vec::new());
        }

        // idelta uses the last two samples
        let prev = &samples[samples.len() - 2];
        let curr = &samples[samples.len() - 1];

        let value_delta = curr.value - prev.value;

        Ok(vec![Sample::new(curr.timestamp, value_delta)])
    }

    fn calculate_resets(&self, samples: &[Sample]) -> usize {
        if samples.len() < 2 {
            return 0;
        }

        let mut resets = 0;
        for i in 1..samples.len() {
            if samples[i].value < samples[i - 1].value {
                resets += 1;
            }
        }

        resets
    }

    fn calculate_changes(&self, samples: &[Sample]) -> usize {
        if samples.len() < 2 {
            return 0;
        }

        let mut changes = 0;
        for i in 1..samples.len() {
            if samples[i].value != samples[i - 1].value {
                changes += 1;
            }
        }

        changes
    }

    fn calculate_deriv(&self, samples: &[Sample]) -> Result<Vec<Sample>> {
        if samples.len() < 2 {
            return Ok(Vec::new());
        }

        // Simple linear regression for derivative
        let n = samples.len() as f64;
        let sum_x: f64 = samples.iter().map(|s| s.timestamp as f64).sum();
        let sum_y: f64 = samples.iter().map(|s| s.value).sum();
        let sum_xy: f64 = samples.iter().map(|s| s.timestamp as f64 * s.value).sum();
        let sum_x2: f64 = samples.iter().map(|s| (s.timestamp as f64).powi(2)).sum();

        let denominator = n * sum_x2 - sum_x.powi(2);
        if denominator == 0.0 {
            return Ok(Vec::new());
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denominator;

        let last = &samples[samples.len() - 1];
        Ok(vec![Sample::new(last.timestamp, slope)])
    }

    fn calculate_predict_linear(&self, samples: &[Sample], prediction_time: f64) -> Option<f64> {
        if samples.len() < 2 {
            return None;
        }

        // Simple linear regression
        let n = samples.len() as f64;
        let sum_x: f64 = samples.iter().map(|s| s.timestamp as f64).sum();
        let sum_y: f64 = samples.iter().map(|s| s.value).sum();
        let sum_xy: f64 = samples.iter().map(|s| s.timestamp as f64 * s.value).sum();
        let sum_x2: f64 = samples.iter().map(|s| (s.timestamp as f64).powi(2)).sum();

        let denominator = n * sum_x2 - sum_x.powi(2);
        if denominator == 0.0 {
            return None;
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denominator;
        let intercept = (sum_y - slope * sum_x) / n;

        let last_timestamp = samples[samples.len() - 1].timestamp as f64;
        let predicted = slope * (last_timestamp + prediction_time * 1000.0) + intercept;

        Some(predicted)
    }

    fn calculate_holt_winters(&self, samples: &[Sample], alpha: f64, beta: f64) -> Result<Vec<Sample>> {
        if samples.len() < 2 {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();
        let mut level = samples[0].value;
        let mut trend = samples[1].value - samples[0].value;

        for (_i, sample) in samples.iter().enumerate().skip(1) {
            let prev_level = level;
            level = alpha * sample.value + (1.0 - alpha) * (level + trend);
            trend = beta * (level - prev_level) + (1.0 - beta) * trend;

            let forecast = level + trend;
            result.push(Sample::new(sample.timestamp, forecast));
        }

        Ok(result)
    }

    /// Execute scalar() function - convert single-element vector to scalar
    async fn execute_scalar(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("scalar() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        // scalar() returns the value of the single element in the vector
        // If the vector has more than one element, it returns NaN
        // If the vector is empty, it returns NaN
        let value = if series.len() == 1 && series[0].samples.len() == 1 {
            series[0].samples[0].value
        } else {
            f64::NAN
        };

        // Return a scalar as a vector with an empty label set
        let mut scalar_series = TimeSeries::new(0, vec![]);
        scalar_series.add_sample(Sample::new(ctx.start, value));

        Ok(vec![scalar_series])
    }

    /// Execute vector() function - convert scalar to vector
    async fn execute_vector(&self, plan: &CallPlan, ctx: &ExecutionContext) -> Result<Vec<TimeSeries>> {
        if plan.args.len() != 1 {
            return Err(Error::InvalidData("vector() requires exactly one argument".to_string()));
        }

        let series = self.execute_plan(&plan.args[0].plan_type, ctx).await?;

        // vector() converts a scalar to a vector with no labels
        // If the input is already a vector, it returns the vector as-is
        if series.len() == 1 && series[0].labels.is_empty() {
            // Already a scalar-like vector, return as-is
            Ok(series)
        } else if series.len() == 1 && series[0].samples.len() == 1 {
            // Single value, convert to vector with no labels
            let mut vector_series = TimeSeries::new(0, vec![]);
            vector_series.add_sample(Sample::new(ctx.start, series[0].samples[0].value));
            Ok(vec![vector_series])
        } else {
            // Return the series as-is (it's already a vector)
            Ok(series)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageConfig;
    use crate::model::{Label, Sample};
    use tempfile::tempdir;

    fn create_test_store() -> (tempfile::TempDir, Arc<MemStore>) {
        let temp_dir = tempdir().unwrap();
        std::fs::create_dir_all(temp_dir.path()).unwrap();
        let config = StorageConfig {
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        (temp_dir, Arc::new(MemStore::new(config).unwrap()))
    }

    #[tokio::test]
    async fn test_execute_vector_query() {
        let (_temp_dir, store) = create_test_store();
        let executor = QueryExecutor::new(store.clone());

        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
        ];

        let samples = vec![
            Sample::new(1000, 100.0),
            Sample::new(2000, 200.0),
            Sample::new(3000, 300.0),
        ];

        store.write(labels, samples).unwrap();

        let plan = QueryPlan {
            plan_type: PlanType::VectorQuery(VectorQueryPlan {
                name: Some("http_requests_total".to_string()),
                matchers: vec![("job".to_string(), "prometheus".to_string())],
                at: None,
                offset: None,
            }),
            start: 0,
            end: 4000,
            step: 1000,
        };

        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.series_count(), 1);
        assert_eq!(result.sample_count(), 3);
    }

    #[tokio::test]
    async fn test_execute_rate() {
        let (_temp_dir, store) = create_test_store();
        let executor = QueryExecutor::new(store.clone());

        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
        ];

        let samples = vec![
            Sample::new(1000, 100.0),
            Sample::new(2000, 200.0),
            Sample::new(3000, 300.0),
        ];

        store.write(labels, samples).unwrap();

        let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
            name: Some("http_requests_total".to_string()),
            matchers: vec![("job".to_string(), "prometheus".to_string())],
                at: None,
                offset: None,
        });

        let call_plan = PlanType::Call(CallPlan {
            func: Function::Rate,
            args: vec![QueryPlan {
                plan_type: vector_plan,
                start: 0,
                end: 4000,
                step: 1000,
            }],
            k: None,
            quantile: None,
        });

        let plan = QueryPlan {
            plan_type: call_plan,
            start: 0,
            end: 4000,
            step: 1000,
        };

        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.series_count(), 1);
        assert_eq!(result.sample_count(), 2); // 2 rates from 3 samples
    }

    #[tokio::test]
    async fn test_execute_sum() {
        let (_temp_dir, store) = create_test_store();
        let executor = QueryExecutor::new(store.clone());

        let labels1 = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
            Label::new("instance", "localhost:9090"),
        ];

        let labels2 = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
            Label::new("instance", "localhost:9091"),
        ];

        store.write(labels1, vec![Sample::new(1000, 100.0)]).unwrap();
        store.write(labels2, vec![Sample::new(1000, 200.0)]).unwrap();

        let vector_plan = PlanType::VectorQuery(VectorQueryPlan {
            name: Some("http_requests_total".to_string()),
            matchers: vec![("job".to_string(), "prometheus".to_string())],
                at: None,
                offset: None,
        });

        let call_plan = PlanType::Call(CallPlan {
            func: Function::Sum,
            args: vec![QueryPlan {
                plan_type: vector_plan,
                start: 0,
                end: 2000,
                step: 1000,
            }],
            k: None,
            quantile: None,
        });

        let plan = QueryPlan {
            plan_type: call_plan,
            start: 0,
            end: 2000,
            step: 1000,
        };

        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.series_count(), 1);
        assert_eq!(result.series[0].samples[0].value, 300.0);
    }

    #[tokio::test]
    async fn test_cache_hit() {
        let (_temp_dir, store) = create_test_store();
        let cache_config = CacheConfig::new(100, 1024 * 1024, 300);
        let executor = QueryExecutor::with_cache(store.clone(), cache_config);

        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
        ];

        let samples = vec![
            Sample::new(1000, 100.0),
            Sample::new(2000, 200.0),
            Sample::new(3000, 300.0),
        ];

        store.write(labels, samples).unwrap();

        let plan = QueryPlan {
            plan_type: PlanType::VectorQuery(VectorQueryPlan {
                name: Some("http_requests_total".to_string()),
                matchers: vec![("job".to_string(), "prometheus".to_string())],
                at: None,
                offset: None,
            }),
            start: 0,
            end: 4000,
            step: 1000,
        };

        let result1 = executor.execute_with_query(&plan, Some("http_requests_total{job=\"prometheus\"}")).await.unwrap();
        assert_eq!(result1.series_count(), 1);

        let stats = executor.cache_stats().unwrap();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);

        let result2 = executor.execute_with_query(&plan, Some("http_requests_total{job=\"prometheus\"}")).await.unwrap();
        assert_eq!(result2.series_count(), 1);

        let stats = executor.cache_stats().unwrap();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
        
        assert_eq!(result1.start, result2.start);
        assert_eq!(result1.end, result2.end);
    }

    #[tokio::test]
    async fn test_cache_miss_different_query() {
        let (_temp_dir, store) = create_test_store();
        let cache_config = CacheConfig::new(100, 1024 * 1024, 300);
        let executor = QueryExecutor::with_cache(store.clone(), cache_config);

        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
        ];

        let samples = vec![
            Sample::new(1000, 100.0),
        ];

        store.write(labels, samples).unwrap();

        let plan = QueryPlan {
            plan_type: PlanType::VectorQuery(VectorQueryPlan {
                name: Some("http_requests_total".to_string()),
                matchers: vec![("job".to_string(), "prometheus".to_string())],
                at: None,
                offset: None,
            }),
            start: 0,
            end: 2000,
            step: 1000,
        };

        executor.execute_with_query(&plan, Some("http_requests_total{job=\"prometheus\"}")).await.unwrap();
        
        let plan2 = QueryPlan {
            plan_type: PlanType::VectorQuery(VectorQueryPlan {
                name: Some("http_requests_total".to_string()),
                matchers: vec![],
                at: None,
                offset: None,
            }),
            start: 0,
            end: 2000,
            step: 1000,
        };
        
        executor.execute_with_query(&plan2, Some("http_requests_total")).await.unwrap();

        let stats = executor.cache_stats().unwrap();
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.hits, 0);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let (_temp_dir, store) = create_test_store();
        let cache_config = CacheConfig::new(100, 1024 * 1024, 300);
        let executor = QueryExecutor::with_cache(store.clone(), cache_config);

        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
        ];

        let samples = vec![Sample::new(1000, 100.0)];
        store.write(labels, samples).unwrap();

        let plan = QueryPlan {
            plan_type: PlanType::VectorQuery(VectorQueryPlan {
                name: Some("http_requests_total".to_string()),
                matchers: vec![("job".to_string(), "prometheus".to_string())],
                at: None,
                offset: None,
            }),
            start: 0,
            end: 2000,
            step: 1000,
        };

        executor.execute_with_query(&plan, Some("http_requests_total{job=\"prometheus\"}")).await.unwrap();
        
        let stats = executor.cache_stats().unwrap();
        assert_eq!(stats.entry_count, 1);
        
        executor.clear_cache();
        
        let stats = executor.cache_stats().unwrap();
        assert_eq!(stats.entry_count, 0);
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let (_temp_dir, store) = create_test_store();
        let executor = QueryExecutor::new(store.clone());

        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
        ];

        let samples = vec![Sample::new(1000, 100.0)];
        store.write(labels, samples).unwrap();

        let plan = QueryPlan {
            plan_type: PlanType::VectorQuery(VectorQueryPlan {
                name: Some("http_requests_total".to_string()),
                matchers: vec![("job".to_string(), "prometheus".to_string())],
                at: None,
                offset: None,
            }),
            start: 0,
            end: 2000,
            step: 1000,
        };

        executor.execute_with_query(&plan, Some("http_requests_total{job=\"prometheus\"}")).await.unwrap();
        
        assert!(executor.cache_stats().is_none());
    }

    #[tokio::test]
    async fn test_cache_hit_rate() {
        let (_temp_dir, store) = create_test_store();
        let cache_config = CacheConfig::new(100, 1024 * 1024, 300);
        let executor = QueryExecutor::with_cache(store.clone(), cache_config);

        let labels = vec![
            Label::new("__name__", "http_requests_total"),
            Label::new("job", "prometheus"),
        ];

        let samples = vec![Sample::new(1000, 100.0)];
        store.write(labels, samples).unwrap();

        let plan = QueryPlan {
            plan_type: PlanType::VectorQuery(VectorQueryPlan {
                name: Some("http_requests_total".to_string()),
                matchers: vec![("job".to_string(), "prometheus".to_string())],
                at: None,
                offset: None,
            }),
            start: 0,
            end: 2000,
            step: 1000,
        };

        executor.execute_with_query(&plan, Some("query1")).await.unwrap();
        executor.execute_with_query(&plan, Some("query1")).await.unwrap();
        executor.execute_with_query(&plan, Some("query1")).await.unwrap();
        executor.execute_with_query(&plan, Some("query2")).await.unwrap();

        let stats = executor.cache_stats().unwrap();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 2);
        assert!((stats.hit_rate() - 0.5).abs() < 0.01);
    }
}

/// Format a float value as a string in Prometheus-compatible format
fn format_value(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else if value.is_infinite() {
        if value.is_sign_positive() {
            "+Inf".to_string()
        } else {
            "-Inf".to_string()
        }
    } else if value == value.trunc() {
        // Integer value
        format!("{:.0}", value)
    } else {
        // Float value with appropriate precision
        format!("{:.6}", value).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
