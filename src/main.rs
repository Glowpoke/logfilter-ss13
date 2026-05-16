use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process;

/// A log entry with extracted information.
struct LogEntry {
    timestamp: String,                 // e.g. "[2025-08-06 06:50:39]"
    key: Option<String>,               // extracted key, may be "*no key*" or absent
    coords: Option<(i32, i32, i32)>,   // (x,y,z)
    full_line: String,                 // original line including newline
}

/// All possible filter settings.
struct Filters {
    key_filter: Option<Vec<String>>,
    point_filter: Option<(i32, i32, i32)>,
    point_range: (i32, i32, i32),       // tolerance in x, y, z
    proximity_filter: Option<(String, String)>,
    follow_filter: Option<String>,
}

/// Extract the timestamp part out of a log line.
fn extract_timestamp(line: &str) -> Option<&str> {
    if line.starts_with('[') {
        if let Some(end) = line.find(']') {
            return Some(&line[..=end]);
        }
    }
    None
}

/// Extract the key (the part before "/(name)") from a log line.
fn extract_key(line: &str) -> Option<String> {
    let after_ts = if let Some(idx) = line.find("] ") {
        &line[idx + 2..]
    } else {
        return None;
    };

    let slash_paren = after_ts.find("/(")?;
    let before_slash = &after_ts[..slash_paren];

    let start = before_slash
        .rfind(|c: char| c == ' ' || c == ':')
        .map(|i| i + 1)
        .unwrap_or(0);
    let key = before_slash[start..].trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

/// Extract (x,y,z) coordinates from the end of a log line.
fn extract_coords(line: &str) -> Option<(i32, i32, i32)> {
    let line = line.trim_end();
    if !line.ends_with("))") {
        return None;
    }
    let inner = &line[..line.len() - 2];
    let open = inner.rfind('(')?;
    let coords_str = &inner[open + 1..];
    let mut parts = coords_str.split(',');
    let x: i32 = parts.next()?.trim().parse().ok()?;
    let y: i32 = parts.next()?.trim().parse().ok()?;
    let z: i32 = parts.next()?.trim().parse().ok()?;
    Some((x, y, z))
}

/// Check whether two coordinates are within the given range tolerance.
fn coords_within_range(a: (i32, i32, i32), b: (i32, i32, i32), range: (i32, i32, i32)) -> bool {
    (a.0 - b.0).abs() <= range.0 && (a.1 - b.1).abs() <= range.1 && (a.2 - b.2).abs() <= range.2
}

/// Read a file and return its lines as Vec<LogEntry>.
fn load_file(path: &str) -> Vec<LogEntry> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Skipping '{}': {}", path, e);
            return Vec::new();
        }
    };
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        let timestamp = match extract_timestamp(&line) {
            Some(t) => t.to_string(),
            None => continue,
        };
        let key = extract_key(&line);
        let coords = extract_coords(&line);
        entries.push(LogEntry {
            timestamp,
            key,
            coords,
            full_line: line,
        });
    }
    entries
}

/// Apply key and point filters to a single entry (stateless).
fn passes_basic_filters(entry: &LogEntry, filters: &Filters) -> bool {
    // Key filter
    if let Some(ref keys) = filters.key_filter {
        match &entry.key {
            Some(k) if keys.contains(k) => {}
            _ => return false,
        }
    }
    // Point filter
    if let Some((px, py, pz)) = filters.point_filter {
        let (rx, ry, rz) = filters.point_range;
        match entry.coords {
            Some((x, y, z)) => {
                if (x - px).abs() > rx || (y - py).abs() > ry || (z - pz).abs() > rz {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

/// Apply per‑file proximity filter (two keys must share a location within range).
fn apply_proximity_filter(entries: Vec<LogEntry>, filters: &Filters) -> Vec<LogEntry> {
    if let Some((ref key_a, ref key_b)) = filters.proximity_filter {
        let range = filters.point_range;
        let coords_a: Vec<(i32, i32, i32)> = entries
            .iter()
            .filter(|e| e.key.as_deref() == Some(key_a.as_str()))
            .filter_map(|e| e.coords)
            .collect();
        let coords_b: Vec<(i32, i32, i32)> = entries
            .iter()
            .filter(|e| e.key.as_deref() == Some(key_b.as_str()))
            .filter_map(|e| e.coords)
            .collect();

        let mut in_range_a = std::collections::HashSet::new();
        for &c_a in &coords_a {
            for &c_b in &coords_b {
                if coords_within_range(c_a, c_b, range) {
                    in_range_a.insert(c_a);
                    break;
                }
            }
        }
        let mut in_range_b = std::collections::HashSet::new();
        for &c_b in &coords_b {
            for &c_a in &coords_a {
                if coords_within_range(c_a, c_b, range) {
                    in_range_b.insert(c_b);
                    break;
                }
            }
        }

        entries
            .into_iter()
            .filter(|e| {
                if let Some(ref k) = e.key {
                    if k == key_a {
                        e.coords.map_or(false, |c| in_range_a.contains(&c))
                    } else if k == key_b {
                        e.coords.map_or(false, |c| in_range_b.contains(&c))
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .collect()
    } else {
        entries
    }
}

/// Apply the chronological follow filter.
/// Keeps ALL log lines whose coordinates are within `range` of the followed key’s
/// **current** location (updates when the key moves).
fn apply_follow_filter_chronological(mut entries: Vec<LogEntry>, filters: &Filters) -> Vec<LogEntry> {
    let follow_key = match filters.follow_filter {
        Some(ref k) => k.clone(),
        None => return entries,
    };
    let range = filters.point_range;

    // Sort chronologically – essential for stateful location tracking.
    entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let mut current_location: Option<(i32, i32, i32)> = None;
    let mut result = Vec::new();

    for entry in entries {
        // Update the followed key’s location if this entry belongs to it.
        if entry.key.as_deref() == Some(follow_key.as_str()) {
            if let Some(coord) = entry.coords {
                match current_location {
                    None => current_location = Some(coord),
                    Some(loc) if !coords_within_range(coord, loc, range) => {
                        current_location = Some(coord);
                    }
                    _ => {} // same location, no change
                }
            }
        }

        // Keep the entry if we have a current location and the entry is within it.
        if let Some(loc) = current_location {
            if let Some(coord) = entry.coords {
                if coords_within_range(coord, loc, range) {
                    result.push(entry);
                }
            }
            // entries without coordinates are silently dropped.
        }
        // Before the key appears, nothing is kept.
    }

    result
}

/// Merge all entries and write chronologically.
fn merge_and_write(entries: Vec<LogEntry>, output_path: &str) {
    let mut all = entries;
    all.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let mut out = match File::create(output_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Cannot create output file '{}': {}", output_path, e);
            process::exit(1);
        }
    };
    for entry in all {
        let _ = writeln!(out, "{}", entry.full_line);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: {} -i <file1> [file2 ...] [-o <output>] [OPTIONS]\n\
             Options:\n  -k, --key key1,key2,...\n  -p, --point x,y,z\n  -r, --range dx,dy,dz (default 15,15,0)\n  \
             -P, --proximity keyA keyB\n  -f, --follow key\n\
             Note: -p, -P, and -f are mutually exclusive.",
            args[0]
        );
        process::exit(1);
    }

    let mut input_files = Vec::new();
    let mut output_file = String::new();
    let mut filters = Filters {
        key_filter: None,
        point_filter: None,
        point_range: (15, 15, 0),
        proximity_filter: None,
        follow_filter: None,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-i" => {
                i += 1;
                while i < args.len() && !args[i].starts_with('-') {
                    input_files.push(args[i].clone());
                    i += 1;
                }
                continue;
            }
            "-o" => {
                i += 1;
                if i < args.len() && !args[i].starts_with('-') {
                    output_file = args[i].clone();
                    i += 1;
                } else {
                    eprintln!("Missing output file after -o");
                    process::exit(1);
                }
                continue;
            }
            "-k" | "--key" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Missing key list after -k/--key");
                    process::exit(1);
                }
                let keys: Vec<String> = args[i].split(',').map(|s| s.trim().to_string()).collect();
                filters.key_filter = Some(keys);
            }
            "-p" | "--point" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Missing point after -p/--point");
                    process::exit(1);
                }
                let parts: Vec<&str> = args[i].split(',').collect();
                if parts.len() != 3 {
                    eprintln!("Point must be x,y,z");
                    process::exit(1);
                }
                let x: i32 = parts[0].trim().parse().expect("Invalid x coordinate");
                let y: i32 = parts[1].trim().parse().expect("Invalid y coordinate");
                let z: i32 = parts[2].trim().parse().expect("Invalid z coordinate");
                filters.point_filter = Some((x, y, z));
            }
            "-r" | "--range" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Missing range after -r/--range");
                    process::exit(1);
                }
                let parts: Vec<&str> = args[i].split(',').collect();
                if parts.len() != 3 {
                    eprintln!("Range must be dx,dy,dz");
                    process::exit(1);
                }
                let dx: i32 = parts[0].trim().parse().expect("Invalid dx");
                let dy: i32 = parts[1].trim().parse().expect("Invalid dy");
                let dz: i32 = parts[2].trim().parse().expect("Invalid dz");
                filters.point_range = (dx, dy, dz);
            }
            "-P" | "--proximity" => {
                i += 1;
                if i + 1 >= args.len() || args[i].starts_with('-') || args[i + 1].starts_with('-') {
                    eprintln!("-P/--proximity requires two key arguments");
                    process::exit(1);
                }
                let key_a = args[i].clone();
                let key_b = args[i + 1].clone();
                filters.proximity_filter = Some((key_a, key_b));
                i += 1;
            }
            "-f" | "--follow" => {
                i += 1;
                if i >= args.len() || args[i].starts_with('-') {
                    eprintln!("Missing key after -f/--follow");
                    process::exit(1);
                }
                filters.follow_filter = Some(args[i].clone());
            }
            other => {
                eprintln!("Unknown flag: {}", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    if input_files.is_empty() {
        eprintln!("No input files provided (use -i <files>)");
        process::exit(1);
    }
    if output_file.is_empty() {
        output_file = "filtered_log.log".to_string();
    }

    // Mutual exclusivity check
    let mut location_filters = 0;
    if filters.point_filter.is_some() { location_filters += 1; }
    if filters.proximity_filter.is_some() { location_filters += 1; }
    if filters.follow_filter.is_some() { location_filters += 1; }
    if location_filters > 1 {
        eprintln!("Error: --point (-p), --proximity (-P), and --follow (-f) are mutually exclusive.");
        process::exit(1);
    }

    // Load, apply basic filters, then per‑file proximity if active.
    let mut all_entries: Vec<LogEntry> = Vec::new();
    for path in &input_files {
        let entries = load_file(path);
        let filtered: Vec<LogEntry> = entries
            .into_iter()
            .filter(|e| passes_basic_filters(e, &filters))
            .collect();
        let processed = if filters.proximity_filter.is_some() {
            apply_proximity_filter(filtered, &filters)
        } else {
            filtered
        };
        all_entries.extend(processed);
    }

    // Apply chronological follow filter across all entries (if active).
    if filters.follow_filter.is_some() {
        all_entries = apply_follow_filter_chronological(all_entries, &filters);
    }

    merge_and_write(all_entries, &output_file);
    println!("Done. Output written to {}", output_file);
}