mod column;
mod block;
mod writer;
mod reader;
mod block_format;

pub use column::{Column, ColumnBuilder, ColumnType};
pub use block::{Block, BlockMeta, BlockBuilder, BLOCK_MAGIC, BLOCK_VERSION};
pub use writer::BlockWriter;
pub use reader::BlockReader;
pub use reader::BlockManager as ColumnBlockManager;
pub use reader::SeriesIndexEntry;
pub use block_format::{
    BlockHeader, BlockType, CompressionType, ColumnData, ColumnType as BlockColumnType,
    BlockBuilder as ChronoBlockBuilder, BlockReader as ChronoBlockReader,
    BLOCK_FORMAT_VERSION, SeriesBlockData,
};

/// 降采样级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DownsampleLevel {
    /// 原始数据 (10s) - L0
    L0 = 0,
    /// 1分钟精度 - L1
    L1 = 1,
    /// 5分钟精度 - L2
    L2 = 2,
    /// 1小时精度 - L3
    L3 = 3,
    /// 1天精度 - L4
    L4 = 4,
}

impl DownsampleLevel {
    /// 获取降采样间隔（毫秒）
    pub fn interval_ms(&self) -> i64 {
        match self {
            DownsampleLevel::L0 => 10_000,        // 10s
            DownsampleLevel::L1 => 60_000,   // 1min
            DownsampleLevel::L2 => 300_000, // 5min
            DownsampleLevel::L3 => 3_600_000,  // 1h
            DownsampleLevel::L4 => 86_400_000,  // 1d
        }
    }

    /// 从整数创建
    pub fn from_u8(level: u8) -> Option<Self> {
        match level {
            0 => Some(DownsampleLevel::L0),
            1 => Some(DownsampleLevel::L1),
            2 => Some(DownsampleLevel::L2),
            3 => Some(DownsampleLevel::L3),
            4 => Some(DownsampleLevel::L4),
            _ => None,
        }
    }

    /// 获取下一个级别
    pub fn next(&self) -> Option<Self> {
        match self {
            DownsampleLevel::L0 => Some(DownsampleLevel::L1),
            DownsampleLevel::L1 => Some(DownsampleLevel::L2),
            DownsampleLevel::L2 => Some(DownsampleLevel::L3),
            DownsampleLevel::L3 => Some(DownsampleLevel::L4),
            DownsampleLevel::L4 => None,
        }
    }

    /// 获取上一个级别
    pub fn prev(&self) -> Option<Self> {
        match self {
            DownsampleLevel::L0 => None,
            DownsampleLevel::L1 => Some(DownsampleLevel::L0),
            DownsampleLevel::L2 => Some(DownsampleLevel::L1),
            DownsampleLevel::L3 => Some(DownsampleLevel::L2),
            DownsampleLevel::L4 => Some(DownsampleLevel::L3),
        }
    }

    /// 根据查询范围选择合适的降采样级别
    pub fn from_query_range(range_ms: i64) -> Self {
        let range_hours = range_ms / 3_600_000;
        
        if range_hours < 1 {
            DownsampleLevel::L0
        } else if range_hours < 24 {
            DownsampleLevel::L1
        } else if range_hours < 168 {
            DownsampleLevel::L2
        } else if range_hours < 720 {
            DownsampleLevel::L3
        } else {
            DownsampleLevel::L4
        }
    }

    /// 获取分辨率（毫秒）- 别名，与interval_ms相同
    pub fn resolution_ms(&self) -> i64 {
        self.interval_ms()
    }

    /// 获取保留天数
    pub fn retention_days(&self) -> i64 {
        match self {
            DownsampleLevel::L0 => 7,
            DownsampleLevel::L1 => 30,
            DownsampleLevel::L2 => 90,
            DownsampleLevel::L3 => 365,
            DownsampleLevel::L4 => 3650,
        }
    }

    /// 从分辨率毫秒值获取对应的DownsampleLevel
    pub fn from_resolution_ms(resolution_ms: i64) -> Option<Self> {
        match resolution_ms {
            10_000 => Some(DownsampleLevel::L0),
            60_000 => Some(DownsampleLevel::L1),
            300_000 => Some(DownsampleLevel::L2),
            3_600_000 => Some(DownsampleLevel::L3),
            86_400_000 => Some(DownsampleLevel::L4),
            _ => None,
        }
    }
}
