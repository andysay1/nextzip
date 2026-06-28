use serde::{Deserialize, Serialize};

use crate::schema::ColumnType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnCodec {
    Raw,
    Dictionary,
    Delta,
    DeltaOfDelta,
    Rle,
    BitPack,
    FrameOfReference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnPlan {
    pub name: String,
    pub column_type: ColumnType,
    pub codec: ColumnCodec,
    pub original_len: u64,
    pub encoded_len_estimate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedColumn {
    pub name: String,
    pub codec: ColumnCodec,
    pub data: Vec<u8>,
}
