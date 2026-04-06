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
    
    // 检查是否是函数调用
    if let Some(paren_idx) = query.find('(') {
        let func_name = &query[..paren_idx].trim();
        if let Some(func) = Function::from_name(func_name) {
            // 解析函数参数
            let args_str = &query[paren_idx + 1..query.len() - 1];
            let args = parse_args(args_str)?;
            return Ok(Expr::new(ExprType::Call(Call { func, args })));
        }
    }
    
    // 检查是否是矩阵选择器
    if let Some(bracket_idx) = query.find('[') {
        let vector_part = &query[..bracket_idx];
        let range_part = &query[bracket_idx + 1..query.len() - 1];
        let range = parse_duration(range_part)?;
        
        let vector_selector = parse_vector_selector(vector_part)?;
        return Ok(Expr::new(ExprType::MatrixSelector(MatrixSelector {
            vector_selector,
            range,
        })));
    }
    
    // 解析向量选择器
    let vector_selector = parse_vector_selector(query)?;
    Ok(Expr::new(ExprType::VectorSelector(vector_selector)))
}

fn parse_vector_selector(s: &str) -> Result<VectorSelector> {
    let s = s.trim();
    
    // 解析名称和标签
    let mut name = None;
    let mut matchers = Vec::new();
    
    if let Some(brace_idx) = s.find('{') {
        // 有标签匹配器
        name = if brace_idx > 0 {
            Some(s[..brace_idx].trim().to_string())
        } else {
            None
        };
        
        let matchers_str = &s[brace_idx + 1..s.len() - 1];
        matchers = parse_matchers(matchers_str)?;
    } else {
        // 只有名称
        name = Some(s.to_string());
    }
    
    Ok(VectorSelector {
        name,
        matchers: Matchers::new(matchers),
        offset: None,
        at: None,
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
}
