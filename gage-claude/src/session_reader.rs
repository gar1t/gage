use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

/// Iterator over parsed NDJSON session lines.
///
/// Yields `(line_number, parsed_value)` pairs where `line_number` is
/// the 1-based position in the file (including empty and malformed
/// lines in the count). Empty lines and lines that fail JSON parsing
/// are skipped — they consume a line number but produce no item.
pub struct SessionReader {
    reader: io::Lines<BufReader<File>>,
    line_num: u32,
}

impl SessionReader {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self {
            reader: BufReader::new(file).lines(),
            line_num: 0,
        })
    }
}

impl Iterator for SessionReader {
    type Item = io::Result<(u32, Value)>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = match self.reader.next()? {
                Ok(l) => l,
                Err(e) => return Some(Err(e)),
            };
            self.line_num += 1;

            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<Value>(&line) {
                Ok(v) => return Some(Ok((self.line_num, v))),
                Err(_) => continue,
            }
        }
    }
}
