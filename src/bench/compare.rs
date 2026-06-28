use std::fs;
use std::io::Write;
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;

use crate::{detect_format, pack, PackOptions};

pub fn bench_file(input: &Path) -> anyhow::Result<String> {
    let bytes = fs::read(input)?;
    let zstd_bytes = zstd::stream::encode_all(bytes.as_slice(), 3)?;
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&bytes)?;
    let gzip_bytes = gz.finish()?;
    let nextzip_bytes = pack(
        &bytes,
        PackOptions {
            exact: false,
            level: 3,
        },
    )?;
    let winner = [
        ("zstd", zstd_bytes.len()),
        ("gzip", gzip_bytes.len()),
        ("nextzip", nextzip_bytes.len()),
    ]
    .into_iter()
    .min_by_key(|(_, size)| *size)
    .map(|(name, _)| name)
    .unwrap_or("nextzip");
    let ratio_vs_zstd = zstd_bytes.len() as f64 / nextzip_bytes.len().max(1) as f64;
    let fallback = crate::inspect_archive(&nextzip_bytes)?.contains("fallback: true");
    Ok(format!(
        "file: {}\nformat: {:?}\noriginal: {} bytes\n\nzstd:     {} bytes\ngzip:     {} bytes\nnextzip:  {} bytes\n\nwinner: {}\nratio_vs_zstd: {:.2}x\nfallback: {}",
        input.display(),
        detect_format(&bytes),
        bytes.len(),
        zstd_bytes.len(),
        gzip_bytes.len(),
        nextzip_bytes.len(),
        winner,
        ratio_vs_zstd,
        fallback
    ))
}
