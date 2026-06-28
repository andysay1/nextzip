use serde::{Deserialize, Serialize};

use crate::column::ColumnPlan;
use crate::schema::Schema;

pub const MAGIC: &[u8; 4] = b"NXZ1";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputFormat {
    Jsonl,
    Csv,
    Tsv,
    Logs,
    BinaryFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveHeader {
    pub version: u32,
    pub header_schema_version: u32,
    pub original_size: u64,
    pub original_hash: [u8; 32],
    pub format: InputFormat,
    pub exact_mode: bool,
    pub fallback_used: bool,
    pub schema: Schema,
    pub row_count: u64,
    pub column_plans: Vec<ColumnPlan>,
}
