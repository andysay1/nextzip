use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;

use crate::{detect_format, inspect_archive, pack, PackOptions};

#[derive(Debug, Clone, Serialize)]
pub struct BenchRow {
    pub file: String,
    pub format: String,
    pub original: usize,
    pub zstd: usize,
    pub gzip: usize,
    pub nextzip: usize,
    pub winner: String,
    pub ratio_vs_zstd: f64,
    pub fallback: bool,
}

pub fn bench_file(input: &Path) -> anyhow::Result<String> {
    Ok(format_legacy(&bench_one(input)?))
}

pub fn bench_path(input: &Path, json_output: Option<&Path>) -> anyhow::Result<String> {
    let rows = if input.is_dir() {
        let mut files = fs::read_dir(input)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| matches!(ext, "jsonl" | "csv" | "tsv" | "log" | "bin"))
            })
            .collect::<Vec<PathBuf>>();
        files.sort();
        files
            .iter()
            .map(|path| bench_one(path))
            .collect::<anyhow::Result<Vec<_>>>()?
    } else {
        vec![bench_one(input)?]
    };

    if let Some(path) = json_output {
        fs::write(path, serde_json::to_vec_pretty(&rows)?)?;
    }

    if rows.len() == 1 {
        Ok(format_legacy(&rows[0]))
    } else {
        Ok(format_markdown(&rows))
    }
}

fn bench_one(input: &Path) -> anyhow::Result<BenchRow> {
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
    .map(|(name, _)| name.to_string())
    .unwrap_or_else(|| "nextzip".to_string());
    let ratio_vs_zstd = zstd_bytes.len() as f64 / nextzip_bytes.len().max(1) as f64;
    let fallback = inspect_archive(&nextzip_bytes)?.contains("fallback: true");
    Ok(BenchRow {
        file: input.display().to_string(),
        format: format!("{:?}", detect_format(&bytes)),
        original: bytes.len(),
        zstd: zstd_bytes.len(),
        gzip: gzip_bytes.len(),
        nextzip: nextzip_bytes.len(),
        winner,
        ratio_vs_zstd,
        fallback,
    })
}

fn format_legacy(row: &BenchRow) -> String {
    format!(
        "file: {}\nformat: {}\noriginal: {} bytes\n\nzstd:     {} bytes\ngzip:     {} bytes\nnextzip:  {} bytes\n\nwinner: {}\nratio_vs_zstd: {:.2}x\nfallback: {}",
        row.file,
        row.format,
        row.original,
        row.zstd,
        row.gzip,
        row.nextzip,
        row.winner,
        row.ratio_vs_zstd,
        row.fallback
    )
}

fn format_markdown(rows: &[BenchRow]) -> String {
    let mut out = String::from(
        "| file | format | original | nextzip | zstd | gzip | vs zstd | winner | fallback |\n|---|---|---:|---:|---:|---:|---:|---|---|\n",
    );
    for row in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {:.2}x | {} | {} |\n",
            row.file,
            row.format,
            row.original,
            row.nextzip,
            row.zstd,
            row.gzip,
            row.ratio_vs_zstd,
            row.winner,
            row.fallback
        ));
    }
    out
}
