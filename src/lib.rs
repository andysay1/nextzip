pub mod archive;
pub mod bench;
pub mod checksum;
pub mod cli;
pub mod codecs;
pub mod column;
pub mod detect;
pub mod entropy;
pub mod error;
pub mod formats;
pub mod header;
pub mod schema;

pub use archive::{
    inspect_archive, inspect_archive_json, inspect_archive_report, pack, pack_file, unpack,
    unpack_file, ArchiveInspection, PackOptions, SizeBreakdown,
};
pub use detect::detect_format;
pub use header::InputFormat;
