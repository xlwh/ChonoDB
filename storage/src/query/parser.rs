use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// PromQL表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expr {
    pub expr_type: ExprType,
}

impl Expr {
    pub fn new(expr_type: ExprType) -> Self {
        Self { expr_type }
    }
}

/// 表达式类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExprType {
    VectorSelector(VectorSelector),
    MatrixSelector(MatrixSelector),
    Subquery(Subquery),
    Call(Call),
    BinaryExpr(BinaryExpr),
    UnaryExpr(UnaryExpr),
    Aggregation(Aggregation),
    NumberLiteral(f64),
    StringLiteral(String),
}

/// 向量选择器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSelector {
    pub name: Option<String>,
    pub matchers: Matchers,
    pub offset: Option<i64>,
    pub at: Option<AtModifier>,
}

/// 矩阵选择器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixSelector {
    pub vector_selector: VectorSelector,
    pub range: i64,
}

/// 子查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subquery {
    pub expr: Box<Expr>,
    pub range: i64,      // 时间范围（毫秒）
    pub resolution: i64, // 分辨率/步长（毫秒）
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Call {
    pub func: Function,
    pub args: Vec<Expr>,
}

/// 二元表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
    pub matching: Option<VectorMatching>,
}

/// 一元表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub expr: Box<Expr>,
}

/// 聚合表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregation {
    pub op: Function,
    pub expr: Box<Expr>,
    pub grouping: Vec<String>,
    pub without: bool,
}

/// @修饰符
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtModifier {
    pub timestamp: i64,
}

/// 向量匹配
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMatching {
    pub cardinality: VectorMatchCardinality,
    pub matching_labels: Vec<String>,
    pub on: bool,
}

/// 向量匹配基数
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VectorMatchCardinality {
    OneToOne,
    ManyToOne,
    OneToMany,
    ManyToMany,
}

/// 匹配操作符
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchOp {
    Equal,
    NotEqual,
    RegexMatch,
    RegexNotMatch,
    Regex,
    NotRegex,
}

impl fmt::Display for MatchOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchOp::Equal => write!(f, "="),
            MatchOp::NotEqual => write!(f, "!="),
            MatchOp::RegexMatch => write!(f, "=~"),
            MatchOp::RegexNotMatch => write!(f, "!~"),
            MatchOp::Regex => write!(f, "=~"),
            MatchOp::NotRegex => write!(f, "!~"),
        }
    }
}

/// 标签匹配器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matcher {
    pub name: String,
    pub value: String,
    pub op: MatchOp,
}

impl Matcher {
    pub fn new(name: String, value: String, op: MatchOp) -> Self {
        Self { name, value, op }
    }

    pub fn eq(name: &str, value: &str) -> Self {
        Self::new(name.to_string(), value.to_string(), MatchOp::Equal)
    }
}

/// 匹配器集合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matchers {
    pub matchers: Vec<Matcher>,
}

impl Matchers {
    pub fn new(matchers: Vec<Matcher>) -> Self {
        Self { matchers }
    }

    pub fn empty() -> Self {
        Self { matchers: Vec::new() }
    }

    pub fn add(&mut self, matcher: Matcher) {
        self.matchers.push(matcher);
    }
}

/// 一元操作符
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UnaryOp {
    Add,
    Sub,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Add => write!(f, "+"),
            UnaryOp::Sub => write!(f, "-"),
        }
    }
}

/// 二元操作符
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Unless,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOp::Add => write!(f, "+"),
            BinaryOp::Sub => write!(f, "-"),
            BinaryOp::Mul => write!(f, "*"),
            BinaryOp::Div => write!(f, "/"),
            BinaryOp::Mod => write!(f, "%"),
            BinaryOp::Pow => write!(f, "^"),
            BinaryOp::Eq => write!(f, "=="),
            BinaryOp::Ne => write!(f, "!="),
            BinaryOp::Lt => write!(f, "<"),
            BinaryOp::Le => write!(f, "<="),
            BinaryOp::Gt => write!(f, ">"),
            BinaryOp::Ge => write!(f, ">="),
            BinaryOp::And => write!(f, "and"),
            BinaryOp::Or => write!(f, "or"),
            BinaryOp::Unless => write!(f, "unless"),
        }
    }
}

/// PromQL函数
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Function {
    // 聚合函数
    Sum,
    Avg,
    Min,
    Max,
    Count,
    Stddev,
    Stdvar,
    TopK,
    BottomK,
    Quantile,
    CountValues,
    
    // 范围向量函数
    Rate,
    Irate,
    Increase,
    Delta,
    Idelta,
    Deriv,
    PredictLinear,
    HoltWinters,
    Resets,
    Changes,
    
    // 数学函数
    Abs,
    Ceil,
    Floor,
    Exp,
    Ln,
    Log2,
    Log10,
    Sqrt,
    Round,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Asinh,
    Acosh,
    Atanh,
    Deg,
    Rad,
    
    // 时间函数
    Time,
    Timestamp,
    DayOfMonth,
    DayOfWeek,
    DaysInMonth,
    Hour,
    Minute,
    Month,
    Year,
    
    // 标签函数
    LabelReplace,
    LabelJoin,
    
    // 其他函数
    Sort,
    SortDesc,
    Clamp,
    ClampMax,
    ClampMin,
    Absent,
    AbsentOverTime,
    PresentOverTime,
    Scalar,
    Vector,
    HistogramQuantile,
    
    // 自定义函数
    Custom(String),
}

impl Function {
    pub fn name(&self) -> &str {
        match self {
            Function::Sum => "sum",
            Function::Avg => "avg",
            Function::Min => "min",
            Function::Max => "max",
            Function::Count => "count",
            Function::Stddev => "stddev",
            Function::Stdvar => "stdvar",
            Function::TopK => "topk",
            Function::BottomK => "bottomk",
            Function::Quantile => "quantile",
            Function::CountValues => "count_values",
            Function::Rate => "rate",
            Function::Irate => "irate",
            Function::Increase => "increase",
            Function::Delta => "delta",
            Function::Idelta => "idelta",
            Function::Deriv => "deriv",
            Function::PredictLinear => "predict_linear",
            Function::HoltWinters => "holt_winters",
            Function::Resets => "resets",
            Function::Changes => "changes",
            Function::Abs => "abs",
            Function::Ceil => "ceil",
            Function::Floor => "floor",
            Function::Exp => "exp",
            Function::Ln => "ln",
            Function::Log2 => "log2",
            Function::Log10 => "log10",
            Function::Sqrt => "sqrt",
            Function::Round => "round",
            Function::Sin => "sin",
            Function::Cos => "cos",
            Function::Tan => "tan",
            Function::Asin => "asin",
            Function::Acos => "acos",
            Function::Atan => "atan",
            Function::Sinh => "sinh",
            Function::Cosh => "cosh",
            Function::Tanh => "tanh",
            Function::Asinh => "asinh",
            Function::Acosh => "acosh",
            Function::Atanh => "atanh",
            Function::Deg => "deg",
            Function::Rad => "rad",
            Function::Time => "time",
            Function::Timestamp => "timestamp",
            Function::DayOfMonth => "day_of_month",
            Function::DayOfWeek => "day_of_week",
            Function::DaysInMonth => "days_in_month",
            Function::Hour => "hour",
            Function::Minute => "minute",
            Function::Month => "month",
            Function::Year => "year",
            Function::LabelReplace => "label_replace",
            Function::LabelJoin => "label_join",
            Function::Sort => "sort",
            Function::SortDesc => "sort_desc",
            Function::Clamp => "clamp",
            Function::ClampMax => "clamp_max",
            Function::ClampMin => "clamp_min",
            Function::Absent => "absent",
            Function::AbsentOverTime => "absent_over_time",
            Function::PresentOverTime => "present_over_time",
            Function::Scalar => "scalar",
            Function::Vector => "vector",
            Function::HistogramQuantile => "histogram_quantile",
            Function::Custom(name) => name.as_str(),
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "sum" => Some(Function::Sum),
            "avg" => Some(Function::Avg),
            "min" => Some(Function::Min),
            "max" => Some(Function::Max),
            "count" => Some(Function::Count),
            "stddev" => Some(Function::Stddev),
            "stdvar" => Some(Function::Stdvar),
            "topk" => Some(Function::TopK),
            "bottomk" => Some(Function::BottomK),
            "quantile" => Some(Function::Quantile),
            "count_values" => Some(Function::CountValues),
            "rate" => Some(Function::Rate),
            "irate" => Some(Function::Irate),
            "increase" => Some(Function::Increase),
            "delta" => Some(Function::Delta),
            "idelta" => Some(Function::Idelta),
            "deriv" => Some(Function::Deriv),
            "predict_linear" => Some(Function::PredictLinear),
            "holt_winters" => Some(Function::HoltWinters),
            "resets" => Some(Function::Resets),
            "changes" => Some(Function::Changes),
            "abs" => Some(Function::Abs),
            "ceil" => Some(Function::Ceil),
            "floor" => Some(Function::Floor),
            "exp" => Some(Function::Exp),
            "ln" => Some(Function::Ln),
            "log2" => Some(Function::Log2),
            "log10" => Some(Function::Log10),
            "sqrt" => Some(Function::Sqrt),
            "round" => Some(Function::Round),
            "sin" => Some(Function::Sin),
            "cos" => Some(Function::Cos),
            "tan" => Some(Function::Tan),
            "asin" => Some(Function::Asin),
            "acos" => Some(Function::Acos),
            "atan" => Some(Function::Atan),
            "sinh" => Some(Function::Sinh),
            "cosh" => Some(Function::Cosh),
            "tanh" => Some(Function::Tanh),
            "asinh" => Some(Function::Asinh),
            "acosh" => Some(Function::Acosh),
            "atanh" => Some(Function::Atanh),
            "deg" => Some(Function::Deg),
            "rad" => Some(Function::Rad),
            "time" => Some(Function::Time),
            "timestamp" => Some(Function::Timestamp),
            "day_of_month" => Some(Function::DayOfMonth),
            "day_of_week" => Some(Function::DayOfWeek),
            "days_in_month" => Some(Function::DaysInMonth),
            "hour" => Some(Function::Hour),
            "minute" => Some(Function::Minute),
            "month" => Some(Function::Month),
            "year" => Some(Function::Year),
            "label_replace" => Some(Function::LabelReplace),
            "label_join" => Some(Function::LabelJoin),
            "sort" => Some(Function::Sort),
            "sort_desc" => Some(Function::SortDesc),
            "clamp" => Some(Function::Clamp),
            "clamp_max" => Some(Function::ClampMax),
            "clamp_min" => Some(Function::ClampMin),
            "absent" => Some(Function::Absent),
            "absent_over_time" => Some(Function::AbsentOverTime),
            "present_over_time" => Some(Function::PresentOverTime),
            "scalar" => Some(Function::Scalar),
            "vector" => Some(Function::Vector),
            "histogram_quantile" => Some(Function::HistogramQuantile),
            _ => None,
        }
    }

    pub fn is_aggregation(&self) -> bool {
        matches!(self, 
            Function::Sum | 
            Function::Avg | 
            Function::Min | 
            Function::Max | 
            Function::Count | 
            Function::Stddev | 
            Function::Stdvar | 
            Function::TopK | 
            Function::BottomK | 
            Function::Quantile
        )
    }

    pub fn is_range_function(&self) -> bool {
        matches!(self,
            Function::Rate |
            Function::Irate |
            Function::Increase |
            Function::Delta |
            Function::Deriv |
            Function::PredictLinear |
            Function::HoltWinters
        )
    }
}

/// 解析PromQL表达式
pub fn parse_promql(query: &str) -> Result<Expr> {
    // 简化实现：解析基本的指标名称和标签匹配器
    let query = query.trim();

    // 首先检查是否是聚合表达式 (需要在普通函数调用之前检查)
    // 支持格式: sum by (job) (expr), sum(expr) by (job), sum(expr)
    if let Some(first_paren) = query.find('(') {
        let before_paren = &query[..first_paren].trim();
        // 提取可能的函数名（处理 "sum by (job)" 这种格式）
        let possible_func_name = before_paren.split_whitespace().next().unwrap_or("");

        if let Some(func) = Function::from_name(possible_func_name) {
            if is_aggregation_function(&func) {
                return parse_aggregation(query, func);
            }
        }
    }

    // 检查是否是普通函数调用（非聚合函数）
    if let Some(paren_idx) = query.find('(') {
        let func_name = &query[..paren_idx].trim();
        if let Some(func) = Function::from_name(func_name) {
            // 找到匹配的右括号（处理嵌套）
            let mut depth = 1;
            let mut end_idx = paren_idx + 1;
            for (i, c) in query[paren_idx + 1..].chars().enumerate() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end_idx = paren_idx + 1 + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            // 解析函数参数
            let args_str = &query[paren_idx + 1..end_idx];
            let args = parse_args(args_str)?;

            // 检查函数调用后面是否有子查询修饰符
            let after_call = &query[end_idx + 1..].trim();
            if after_call.starts_with('[') && after_call.contains(':') {
                // 这是子查询，需要递归解析
                let subquery_expr = Expr::new(ExprType::Call(Call { func, args }));
                return parse_subquery_suffix(subquery_expr, after_call);
            }

            return Ok(Expr::new(ExprType::Call(Call { func, args })));
        }
    }

    // 检查是否是子查询 (包含 [<range>:<resolution>] 语法)
    // 需要找到最外层的方括号
    if let Some(bracket_idx) = find_outermost_bracket(query) {
        let expr_part = &query[..bracket_idx];
        let inner = &query[bracket_idx + 1..query.len() - 1];

        // 检查是否包含冒号，表示是子查询
        if inner.contains(':') {
            // 解析子查询
            let parts: Vec<&str> = inner.split(':').collect();
            if parts.len() == 2 {
                let range = parse_duration(parts[0].trim())?;
                let resolution = parse_duration(parts[1].trim())?;

                let expr = parse_promql(expr_part.trim())?;
                return Ok(Expr::new(ExprType::Subquery(Subquery {
                    expr: Box::new(expr),
                    range,
                    resolution,
                })));
            }
        }

        // 矩阵选择器
        let range = parse_duration(inner)?;
        let vector_selector = parse_vector_selector(expr_part)?;
        return Ok(Expr::new(ExprType::MatrixSelector(MatrixSelector {
            vector_selector,
            range,
        })));
    }

    // 解析向量选择器
    let vector_selector = parse_vector_selector(query)?;
    Ok(Expr::new(ExprType::VectorSelector(vector_selector)))
}

/// 找到最外层的方括号位置
fn find_outermost_bracket(query: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in query.chars().enumerate() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ => {}
        }
        // 如果在最外层遇到右方括号，返回其位置
        if c == ']' && depth == 0 {
            // 找到匹配的左方括号
            let mut inner_depth = 1;
            for j in (0..i).rev() {
                match query.chars().nth(j).unwrap() {
                    '[' => {
                        inner_depth -= 1;
                        if inner_depth == 0 {
                            return Some(j);
                        }
                    }
                    ']' => inner_depth += 1,
                    _ => {}
                }
            }
        }
    }
    None
}

/// 解析子查询后缀 [<range>:<resolution>]
fn parse_subquery_suffix(expr: Expr, suffix: &str) -> Result<Expr> {
    if !suffix.starts_with('[') || !suffix.ends_with(']') {
        return Err(Error::InvalidData("Invalid subquery suffix".to_string()));
    }

    let inner = &suffix[1..suffix.len() - 1];
    let parts: Vec<&str> = inner.split(':').collect();

    if parts.len() != 2 {
        return Err(Error::InvalidData("Invalid subquery syntax, expected [<range>:<resolution>]".to_string()));
    }

    let range = parse_duration(parts[0].trim())?;
    let resolution = parse_duration(parts[1].trim())?;

    Ok(Expr::new(ExprType::Subquery(Subquery {
        expr: Box::new(expr),
        range,
        resolution,
    })))
}

/// 检查是否是聚合函数
fn is_aggregation_function(func: &Function) -> bool {
    matches!(func,
        Function::Sum | Function::Avg | Function::Min | Function::Max |
        Function::Count | Function::Stddev | Function::Stdvar |
        Function::TopK | Function::BottomK | Function::Quantile |
        Function::CountValues
    )
}

/// 解析聚合表达式，支持以下格式：
/// - sum(http_requests_total)
/// - sum by (job) (http_requests_total)
/// - sum(http_requests_total) by (job)
/// - sum without (instance) (http_requests_total)
fn parse_aggregation(query: &str, op: Function) -> Result<Expr> {
    let query = query.trim();

    // 获取函数名后的部分
    let func_name_len = op.name().len();
    let after_func = query[func_name_len..].trim();

    let mut grouping: Vec<String> = vec![];
    let mut without = false;
    let expr_str: &str;

    // 检查是否有 by (labels) 或 without (labels) 修饰符在表达式之前
    if after_func.to_lowercase().starts_with("by ") {
        let (labels, rest) = parse_grouping_labels(&after_func[3..])?;
        grouping = labels;
        without = false;
        expr_str = rest.trim();
    } else if after_func.to_lowercase().starts_with("without ") {
        let (labels, rest) = parse_grouping_labels(&after_func[8..])?;
        grouping = labels;
        without = true;
        expr_str = rest.trim();
    } else {
        expr_str = after_func;
    }
    
    // 解析表达式部分（括号内的内容）
    if !expr_str.starts_with('(') {
        return Err(Error::InvalidData(format!(
            "Expected '(' after aggregation modifier, got: {}", expr_str
        )));
    }
    
    // 找到匹配的右括号
    let mut depth = 1;
    let mut end_idx = 1;
    for (i, c) in expr_str[1..].chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    
    if depth != 0 {
        return Err(Error::InvalidData("Unclosed parenthesis in aggregation".to_string()));
    }
    
    let inner_expr = &expr_str[1..end_idx];
    let expr = parse_promql(inner_expr)?;
    
    // 检查表达式后面是否有 by (labels) 或 without (labels) 修饰符
    let after_expr = &expr_str[end_idx + 1..].trim();
    let remaining = if after_expr.to_lowercase().starts_with("by ") {
        let (labels, rest) = parse_grouping_labels(&after_expr[3..])?;
        grouping = labels;
        without = false;
        rest.trim()
    } else if after_expr.to_lowercase().starts_with("without ") {
        let (labels, rest) = parse_grouping_labels(&after_expr[8..])?;
        grouping = labels;
        without = true;
        rest.trim()
    } else {
        after_expr
    };
    
    // 检查是否有子查询修饰符
    if remaining.starts_with('[') && remaining.contains(':') {
        let agg_expr = Expr::new(ExprType::Aggregation(Aggregation {
            op,
            expr: Box::new(expr),
            grouping,
            without,
        }));
        return parse_subquery_suffix(agg_expr, remaining);
    }
    
    Ok(Expr::new(ExprType::Aggregation(Aggregation {
        op,
        expr: Box::new(expr),
        grouping,
        without,
    })))
}

/// 解析分组标签，如 "(job, instance) (expr)" 返回 (labels, rest)
fn parse_grouping_labels(s: &str) -> Result<(Vec<String>, &str)> {
    let s = s.trim();

    if !s.starts_with('(') {
        return Err(Error::InvalidData(format!(
            "Expected '(' after by/without, got: {}", s
        )));
    }

    // 找到匹配的右括号
    let mut depth = 1;
    let mut end_idx = 1;
    for (i, c) in s[1..].chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err(Error::InvalidData("Unclosed parenthesis in grouping labels".to_string()));
    }
    
    let labels_str = &s[1..end_idx];
    let labels: Vec<String> = labels_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    
    let rest = &s[end_idx + 1..];
    Ok((labels, rest))
}

fn parse_vector_selector(s: &str) -> Result<VectorSelector> {
    let s = s.trim();

    // 解析名称和标签
    let mut name = None;
    let mut matchers = Vec::new();
    let mut offset = None;
    let mut at = None;

    // 首先检查是否有 offset 修饰符（在 @ 之前或之后都可能出现）
    // offset 修饰符格式: "offset 5m" 或 "offset -1h"
    let s_without_offset = if let Some(offset_idx) = s.to_lowercase().find(" offset ") {
        let before_offset = &s[..offset_idx];
        let after_offset = &s[offset_idx + 8..].trim(); // Skip " offset "

        // 解析 offset 值，但需要注意 @ 修饰符可能在 offset 之后
        let offset_value = if let Some(at_idx) = after_offset.find(" @") {
            // offset 后面有 @，只取 offset 部分
            &after_offset[..at_idx].trim()
        } else if after_offset.find('@').is_some() {
            // offset 后面有 @（没有空格）
            let at_idx = after_offset.find('@').unwrap();
            &after_offset[..at_idx].trim()
        } else {
            after_offset
        };

        offset = Some(parse_offset_modifier(offset_value)?);
        before_offset
    } else {
        s
    };

    // 然后检查是否有 @ 修饰符
    let base_str = if let Some(at_idx) = s_without_offset.find(" @") {
        let base = &s_without_offset[..at_idx];
        let at_part = &s_without_offset[at_idx + 2..].trim(); // Skip " @"

        // 解析 @ 修饰符
        at = Some(parse_at_modifier(at_part)?);
        base
    } else if let Some(at_idx) = s_without_offset.find('@') {
        let base = &s_without_offset[..at_idx];
        let at_part = &s_without_offset[at_idx + 1..].trim();

        // 解析 @ 修饰符
        at = Some(parse_at_modifier(at_part)?);
        base
    } else {
        s_without_offset
    };

    if let Some(brace_idx) = base_str.find('{') {
        // 有标签匹配器
        name = if brace_idx > 0 {
            Some(base_str[..brace_idx].trim().to_string())
        } else {
            None
        };

        let matchers_str = &base_str[brace_idx + 1..base_str.len() - 1];
        matchers = parse_matchers(matchers_str)?;
    } else {
        // 只有名称
        name = Some(base_str.to_string());
    }

    Ok(VectorSelector {
        name,
        matchers: Matchers::new(matchers),
        offset,
        at,
    })
}

fn parse_offset_modifier(s: &str) -> Result<i64> {
    let s = s.trim();

    // 检查是否有负号
    let (is_negative, s) = if s.starts_with('-') {
        (true, s[1..].trim())
    } else if s.starts_with('+') {
        (false, s[1..].trim())
    } else {
        (false, s)
    };

    // 解析持续时间，如 "5m", "1h", "2d"
    let duration_ms = parse_duration(s)?;

    // 应用符号
    if is_negative {
        Ok(-duration_ms)
    } else {
        Ok(duration_ms)
    }
}

fn parse_at_modifier(s: &str) -> Result<AtModifier> {
    let s = s.trim();

    // 检查是否是 start() 或 end()
    if s.eq_ignore_ascii_case("start()") {
        // 使用特殊值 -1 表示 start()
        return Ok(AtModifier { timestamp: -1 });
    }
    if s.eq_ignore_ascii_case("end()") {
        // 使用特殊值 -2 表示 end()
        return Ok(AtModifier { timestamp: -2 });
    }

    // 解析时间戳（可以是整数或浮点数）
    let timestamp = s.parse::<f64>()
        .map_err(|_| Error::InvalidData(format!("Invalid @ modifier timestamp: {}", s)))?;

    // 转换为毫秒时间戳
    Ok(AtModifier {
        timestamp: timestamp as i64,
    })
}

fn parse_matchers(s: &str) -> Result<Vec<Matcher>> {
    let mut matchers = Vec::new();
    
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        
        // 解析 name="value" 或 name='value'
        if let Some(eq_idx) = part.find("=") {
            let name = part[..eq_idx].trim().to_string();
            let value_part = &part[eq_idx + 1..].trim();
            
            // 去除引号
            let value = if value_part.starts_with('"') && value_part.ends_with('"') {
                value_part[1..value_part.len() - 1].to_string()
            } else if value_part.starts_with('\'') && value_part.ends_with('\'') {
                value_part[1..value_part.len() - 1].to_string()
            } else {
                value_part.to_string()
            };
            
            matchers.push(Matcher::eq(&name, &value));
        }
    }
    
    Ok(matchers)
}

fn parse_args(s: &str) -> Result<Vec<Expr>> {
    let mut args = Vec::new();
    
    if s.trim().is_empty() {
        return Ok(args);
    }
    
    // 简单实现：按逗号分割参数
    let mut depth = 0;
    let mut start = 0;
    
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                let arg_str = &s[start..i];
                args.push(parse_promql(arg_str)?);
                start = i + 1;
            }
            _ => {}
        }
    }
    
    // 处理最后一个参数
    let last_arg = &s[start..];
    args.push(parse_promql(last_arg)?);
    
    Ok(args)
}

fn parse_duration(s: &str) -> Result<i64> {
    let s = s.trim();
    
    // 解析数字部分
    let num_end = s.find(|c: char| !c.is_ascii_digit())
        .unwrap_or(s.len());
    let num: i64 = s[..num_end].parse()
        .map_err(|_| Error::InvalidData(format!("Invalid duration number: {}", s)))?;
    
    // 解析单位
    let unit = &s[num_end..];
    let seconds = match unit {
        "s" => num,
        "m" => num * 60,
        "h" => num * 3600,
        "d" => num * 86400,
        "w" => num * 604800,
        "y" => num * 31536000,
        _ => return Err(Error::InvalidData(format!("Invalid duration unit: {}", unit))),
    };
    
    Ok(seconds * 1000) // 转换为毫秒
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vector_selector() {
        let expr = parse_promql("http_requests_total").unwrap();
        match expr.expr_type {
            ExprType::VectorSelector(vs) => {
                assert_eq!(vs.name, Some("http_requests_total".to_string()));
            }
            _ => panic!("Expected VectorSelector"),
        }
    }

    #[test]
    fn test_parse_vector_selector_with_labels() {
        let expr = parse_promql("http_requests_total{job=\"prometheus\"}").unwrap();
        match expr.expr_type {
            ExprType::VectorSelector(vs) => {
                assert_eq!(vs.name, Some("http_requests_total".to_string()));
                assert_eq!(vs.matchers.matchers.len(), 1);
                assert_eq!(vs.matchers.matchers[0].name, "job");
                assert_eq!(vs.matchers.matchers[0].value, "prometheus");
            }
            _ => panic!("Expected VectorSelector"),
        }
    }

    #[test]
    fn test_parse_matrix_selector() {
        let expr = parse_promql("http_requests_total[5m]").unwrap();
        match expr.expr_type {
            ExprType::MatrixSelector(ms) => {
                assert_eq!(ms.range, 300000); // 5 minutes in milliseconds
            }
            _ => panic!("Expected MatrixSelector"),
        }
    }

    #[test]
    fn test_parse_call() {
        let expr = parse_promql("rate(http_requests_total[5m])").unwrap();
        match expr.expr_type {
            ExprType::Call(call) => {
                assert_eq!(call.func.name(), "rate");
                assert_eq!(call.args.len(), 1);
            }
            _ => panic!("Expected Call"),
        }
    }

    #[test]
    fn test_function_name() {
        assert_eq!(Function::Rate.name(), "rate");
        assert_eq!(Function::Sum.name(), "sum");
    }

    #[test]
    fn test_function_from_name() {
        assert_eq!(Function::from_name("rate"), Some(Function::Rate));
        assert_eq!(Function::from_name("sum"), Some(Function::Sum));
        assert_eq!(Function::from_name("unknown"), None);
    }

    #[test]
    fn test_parse_aggregation_by() {
        // Test sum by (job) (http_requests_total)
        let expr = parse_promql("sum by (job) (http_requests_total)").unwrap();
        match &expr.expr_type {
            ExprType::Aggregation(agg) => {
                assert_eq!(agg.op.name(), "sum");
                assert_eq!(agg.grouping, vec!["job"]);
                assert_eq!(agg.without, false);
            }
            _ => panic!("Expected Aggregation, got {:?}", expr.expr_type),
        }
    }

    #[test]
    fn test_parse_aggregation_trailing_by() {
        // Test sum(http_requests_total) by (job)
        let expr = parse_promql("sum(http_requests_total) by (job)").unwrap();
        match &expr.expr_type {
            ExprType::Aggregation(agg) => {
                assert_eq!(agg.op.name(), "sum");
                assert_eq!(agg.grouping, vec!["job"]);
                assert_eq!(agg.without, false);
            }
            _ => panic!("Expected Aggregation, got {:?}", expr.expr_type),
        }
    }
}
