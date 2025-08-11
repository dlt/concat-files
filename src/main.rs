use anyhow::{bail, Context, Result};
use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let root_dir = args.get(1).map(String::as_str).unwrap_or(".");
    let out_dir = args.get(2).map(String::as_str).unwrap_or("./_out");
    let delim_char = args.get(3).and_then(|s| s.chars().next()).unwrap_or(',');

    if !delim_char.is_ascii() {
        bail!("Delimiter must be a single ASCII character");
    }
    let delim = delim_char as u8;

    let root = Path::new(root_dir).canonicalize().with_context(|| format!("root_dir '{}'", root_dir))?;
    let out = Path::new(out_dir).canonicalize().unwrap_or_else(|_| PathBuf::from(out_dir));
    if !out.exists() {
        fs::create_dir_all(&out).with_context(|| format!("create output dir '{}'", out.display()))?;
    }

    // Enumerate immediate subdirectories
    let mut subdirs: Vec<PathBuf> = fs::read_dir(&root)
        .with_context(|| format!("listing '{}'", root.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();

    // Stable order
    subdirs.sort();

    if subdirs.is_empty() {
        eprintln!("No subdirectories under {}", root.display());
        return Ok(());
    }

    for dir in subdirs {
        let dir_name = dir.file_name().and_then(OsStr::to_str).unwrap_or("unknown");
        // Collect *.csv in this directory (non-recursive)
        let mut csvs: Vec<PathBuf> = fs::read_dir(&dir)
            .with_context(|| format!("reading '{}'", dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && is_csv(p))
            .collect();

        if csvs.is_empty() {
            println!("Skipping '{}': no CSV files", dir_name);
            continue;
        }

        // Deterministic order by path
        csvs.sort();

        // Output path
        let out_path = out.join(format!("{}.csv", dir_name));
        let tmp_path = out_path.with_extension("csv.tmp");

        // Open writer
        let mut wtr = WriterBuilder::new()
            .delimiter(delim)
            .from_path(&tmp_path)
            .with_context(|| format!("create '{}'", tmp_path.display()))?;

        // Determine canonical header from first file
        let first = &csvs[0];
        let (canonical, first_count) = read_header(first, delim)
            .with_context(|| format!("read header '{}'", first.display()))?;

        if canonical.is_empty() {
            eprintln!("WARNING: Empty header in '{}'; skipping directory '{}'", first.display(), dir_name);
            continue;
        }

        // Write canonical header once
        wtr.write_record(&canonical)?;

        // Concatenate files
        for (idx, file) in csvs.iter().enumerate() {
            let mut rdr = ReaderBuilder::new()
                .has_headers(true) // header read and skipped automatically via headers() below
                .delimiter(delim)
                .from_reader(BufReader::new(
                    File::open(file).with_context(|| format!("open '{}'", file.display()))?,
                ));

            // Original header (strip BOM)
            let mut hdr = rdr.headers()?.clone();
            strip_bom(&mut hdr);

            if idx == 0 {
                // first file: use as-is, but we still normalize order to be consistent
                let map = build_mapping(&canonical, &hdr);
                warn_on_mismatch(file, &canonical, &hdr);
                for result in rdr.records() {
                    let rec = result.with_context(|| format!("read row in '{}'", file.display()))?;
                    let out = map_record(&canonical, &hdr, &rec, &map);
                    wtr.write_record(out)?;
                }
            } else {
                // subsequent files: reorder to canonical
                let map = build_mapping(&canonical, &hdr);
                warn_on_mismatch(file, &canonical, &hdr);
                for result in rdr.records() {
                    let rec = result.with_context(|| format!("read row in '{}'", file.display()))?;
                    let out = map_record(&canonical, &hdr, &rec, &map);
                    wtr.write_record(out)?;
                }
            }
        }

        wtr.flush()?;
        // Replace atomically
        fs::rename(&tmp_path, &out_path)
            .with_context(|| format!("move '{}' -> '{}'", tmp_path.display(), out_path.display()))?;
        println!("Wrote: {}", out_path.display());
    }

    println!("All done. Outputs in: {}", out.display());
    Ok(())
}

/// True if file extension looks like .csv (case-insensitive).
fn is_csv(p: &Path) -> bool {
    p.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("csv"))
        .unwrap_or(false)
}

/// Read the header (first row) of a CSV. Returns (header, count).
fn read_header(path: &Path, delim: u8) -> Result<(StringRecord, usize)> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .delimiter(delim)
        .from_reader(BufReader::new(File::open(path)?));

    let mut hdr = rdr.headers()?.clone();
    strip_bom(&mut hdr);
    let count = hdr.len();
    Ok((hdr, count))
}

/// Remove UTF-8 BOM if present in the first header cell.
fn strip_bom(hdr: &mut StringRecord) {
    if let Some(first) = hdr.get(0) {
        const BOM: &str = "\u{feff}";
        if let Some(stripped) = first.strip_prefix(BOM) {
            // Rebuild the record with the first field stripped of BOM.
            let mut rebuilt = StringRecord::new();
            for (i, field) in hdr.iter().enumerate() {
                if i == 0 {
                    rebuilt.push_field(stripped);
                } else {
                    rebuilt.push_field(field);
                }
            }
            *hdr = rebuilt;
        }
    }
}

/// Build a mapping from canonical columns -> indices in the file header (or None if missing).
fn build_mapping(canonical: &StringRecord, file_hdr: &StringRecord) -> Vec<Option<usize>> {
    canonical
        .iter()
        .map(|name| file_hdr.iter().position(|h| h == name))
        .collect()
}

/// Create an output row aligned to the canonical order.
/// Missing cols become "", extra cols are ignored.
fn map_record<'a>(
    canonical: &StringRecord,
    file_hdr: &StringRecord,
    rec: &'a StringRecord,
    map: &[Option<usize>],
) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::with_capacity(canonical.len());
    for (i, m) in map.iter().enumerate() {
        match m {
            Some(src_idx) => {
                // Defensive: if row is short (ragged), use empty
                out.push(rec.get(*src_idx).unwrap_or(""));
            }
            None => {
                // Missing column in file -> empty cell
                out.push("");
            }
        }
    }
    // Extra columns in file that are not in canonical are ignored by design.
    out
}

/// Log warnings when the set/order of columns differs from canonical.
fn warn_on_mismatch(file: &Path, canonical: &StringRecord, file_hdr: &StringRecord) {
    if canonical == file_hdr {
        return;
    }
    // Check set differences
    use std::collections::HashSet;
    let canon_set: HashSet<&str> = canonical.iter().collect();
    let file_set: HashSet<&str> = file_hdr.iter().collect();

    let missing: Vec<&str> = canon_set.difference(&file_set).copied().collect();
    let extra: Vec<&str> = file_set.difference(&canon_set).copied().collect();

    if !missing.is_empty() || !extra.is_empty() {
        eprintln!(
            "WARNING: Header mismatch in '{}'. Missing: [{}] | Extra: [{}]. Columns will be reordered; missing -> empty; extra -> ignored.",
            file.display(),
            missing.join(", "),
            extra.join(", ")
        );
    } else {
        // Same set, different order
        eprintln!(
            "INFO: Column order differs in '{}'. Reordering to canonical.",
            file.display()
        );
    }
}
