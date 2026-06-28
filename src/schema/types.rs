use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
    Null,
    Bool,
    Integer,
    Float,
    String,
    Enum,
    Timestamp,
    JsonRaw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub column_type: ColumnType,
    pub nullable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Schema {
    pub columns: Vec<ColumnSchema>,
}
