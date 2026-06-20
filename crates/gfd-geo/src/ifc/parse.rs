//! Minimal ISO 10303-21 (STEP physical file) tokenizer for IFC: splits the DATA
//! section into `#id = TYPE(args)` records and offers argument accessors (entity
//! refs, numeric literals, quoted strings) that ignore string/ref contents.

use std::collections::HashMap;

use crate::{GeoError, GeoResult};

/// One parsed entity instance: its uppercased type and raw argument string
/// (the text between the outer parentheses).
#[derive(Debug, Clone)]
pub struct Record {
    pub ty: String,
    pub args: String,
}

impl Record {
    /// All `#N` entity references in argument order.
    pub fn refs(&self) -> Vec<u32> {
        let b = self.args.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\'' {
                // skip quoted string (IFC escapes '' )
                i += 1;
                while i < b.len() {
                    if b[i] == b'\'' {
                        if i + 1 < b.len() && b[i + 1] == b'\'' { i += 2; continue; }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            } else if b[i] == b'#' {
                let s = i + 1;
                let mut j = s;
                while j < b.len() && b[j].is_ascii_digit() { j += 1; }
                if j > s {
                    if let Ok(n) = self.args[s..j].parse::<u32>() { out.push(n); }
                }
                i = j;
            } else {
                i += 1;
            }
        }
        out
    }

    /// Numeric literals in order, skipping quoted strings and `#N` refs (so the
    /// digits inside refs/labels are never mistaken for data).
    pub fn floats(&self) -> Vec<f64> {
        let cleaned = strip(&self.args);
        let mut out = Vec::new();
        let bytes = cleaned.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i] as char;
            let starts = c.is_ascii_digit() || ((c == '-' || c == '+' || c == '.') && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit());
            if starts {
                let s = i;
                i += 1;
                while i < bytes.len() {
                    let d = bytes[i] as char;
                    if d.is_ascii_digit() || d == '.' || d == 'e' || d == 'E'
                        || ((d == '+' || d == '-') && matches!(bytes[i - 1] as char, 'e' | 'E'))
                    {
                        i += 1;
                    } else {
                        break;
                    }
                }
                if let Ok(v) = cleaned[s..i].parse::<f64>() { out.push(v); }
            } else {
                i += 1;
            }
        }
        out
    }

    /// Top-level field `idx`, if it's a quoted string, with quotes stripped.
    pub fn string_arg(&self, idx: usize) -> Option<String> {
        let f = split_top_level(&self.args);
        let s = f.get(idx)?.trim();
        if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
            Some(s[1..s.len() - 1].replace("''", "'"))
        } else {
            None
        }
    }
}

/// Remove quoted strings and `#N` refs from an arg string so numeric scanning
/// sees only real numbers.
fn strip(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\'' {
            out.push(' ');
            i += 1;
            while i < b.len() {
                if b[i] == b'\'' {
                    if i + 1 < b.len() && b[i + 1] == b'\'' { i += 2; continue; }
                    i += 1;
                    break;
                }
                i += 1;
            }
        } else if b[i] == b'#' {
            out.push(' ');
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() { i += 1; }
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

/// Split an argument string into top-level comma-separated fields, respecting
/// nested parens and quoted strings.
pub fn split_top_level(args: &str) -> Vec<String> {
    let b = args.as_bytes();
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        match c {
            '\'' => {
                cur.push(c);
                i += 1;
                while i < b.len() {
                    cur.push(b[i] as char);
                    if b[i] == b'\'' {
                        if i + 1 < b.len() && b[i + 1] == b'\'' { cur.push('\''); i += 2; continue; }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            '(' => { depth += 1; cur.push(c); }
            ')' => { depth -= 1; cur.push(c); }
            ',' if depth == 0 => { fields.push(cur.trim().to_string()); cur.clear(); }
            _ => cur.push(c),
        }
        i += 1;
    }
    if !cur.trim().is_empty() {
        fields.push(cur.trim().to_string());
    }
    fields
}

/// The entity store: id → record.
pub struct Store {
    map: HashMap<u32, Record>,
}

impl Store {
    pub fn parse(text: &str) -> GeoResult<Store> {
        // Restrict to the DATA section if present.
        let data = match (text.find("DATA;"), text.rfind("ENDSEC;")) {
            (Some(a), Some(_)) => &text[a + 5..],
            _ => text,
        };
        let mut map = HashMap::new();
        for record in data.split(';') {
            let r = record.trim();
            if !r.starts_with('#') {
                continue;
            }
            let Some(eq) = r.find('=') else { continue };
            let Ok(id) = r[1..eq].trim().parse::<u32>() else { continue };
            let rhs = r[eq + 1..].trim();
            let Some(paren) = rhs.find('(') else { continue };
            let ty = rhs[..paren].trim().to_ascii_uppercase();
            // args = between the first '(' and the matching last ')'.
            let inner = &rhs[paren..];
            let args = inner.strip_prefix('(').and_then(|s| s.strip_suffix(')')).unwrap_or(inner).to_string();
            if ty.is_empty() {
                continue; // complex instances: ignored for envelope extraction
            }
            map.insert(id, Record { ty, args });
        }
        if map.is_empty() {
            return Err(GeoError::Parse("no IFC entity records found".into()));
        }
        Ok(Store { map })
    }

    pub fn get(&self, id: u32) -> Option<&Record> {
        self.map.get(&id)
    }
    pub fn ty_of(&self, id: u32) -> Option<&str> {
        self.map.get(&id).map(|r| r.ty.as_str())
    }
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &Record)> {
        self.map.iter()
    }
    pub fn values(&self) -> impl Iterator<Item = &Record> {
        self.map.values()
    }
}
