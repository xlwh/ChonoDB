mod bloom;
mod inverted;
mod bitmap;

pub use bloom::BloomFilter;
pub use inverted::InvertedIndex;
pub use bitmap::{BitmapIndex, RoaringBitmap, BitmapStats};

use crate::model::TimeSeriesId;
use crate::error::Result;

pub trait Index {
    fn add(&mut self, key: &str, series_id: TimeSeriesId) -> Result<()>;
    fn remove(&mut self, key: &str, series_id: TimeSeriesId) -> Result<()>;
    fn lookup(&self, key: &str) -> Result<Vec<TimeSeriesId>>;
}