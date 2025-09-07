use std::sync::Arc;

use flow_like::flow::node::NodeLogic;
use flow_like_types::{anyhow, bail, Result, Value as JsonValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use flow_like_storage::arrow::array::{
    ArrayRef, BooleanBuilder, Float64Builder, Int64Builder, LargeStringBuilder, StringBuilder, Date64Builder,
};
use flow_like_storage::arrow::datatypes::{DataType, Field, Schema as ArrowSchema};
use flow_like_storage::arrow::record_batch::RecordBatch;

use flow_like_storage::datafusion::datasource::memory::MemTable;
use flow_like_storage::datafusion::prelude::SessionContext;

use crate::data::path::FlowPath;

pub mod copy_worksheet;
pub mod get_sheet_names;
pub mod insert_column;
pub mod insert_row;
pub mod new_worksheet;
pub mod read_cell;
pub mod remove_column;
pub mod remove_row;
pub mod try_extract_tables;
pub mod write_cell;
pub mod write_cell_html;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ColKind {
    #[default]
    Unknown,
    Bool,
    Int,
    Float,
    Date,
    Utf8, // any non-coercible string observed
}

#[inline]
fn merge_kind(cur: ColKind, obs: ColKind) -> ColKind {
    use ColKind::*;
    match (cur, obs) {
        (Utf8, _) | (_, Utf8) => Utf8,
        (Unknown, x) => x,
        (Bool, Bool) => Bool,
        (Int, Int) => Int,
        (Float, Float) => Float,
        (Date, Date) => Date,
        // promotions
        (Bool, Int) | (Int, Bool) => Int,
        (Bool, Float) | (Float, Bool) | (Int, Float) | (Float, Int) => Float,
        // Mixing dates with anything else -> string fallback
    (Date, Bool | Int | Float) | (Bool | Int | Float, Date) => Utf8,
    (Date, Unknown) => Date,
        _ => Utf8,
    }
}

#[inline]
fn is_bool_str(s: &str) -> Option<bool> {
    match s.trim() {
        x if x.eq_ignore_ascii_case("true") => Some(true),
        x if x.eq_ignore_ascii_case("false") => Some(false),
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

#[inline]
fn is_nullish(s: &str) -> bool {
    matches!(s.trim(), "" | "null" | "NULL" | "NaN" | "N/A" | "na" | "Na")
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Cell {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Arc<str>),
    Date { iso: Arc<str>, ms: i64 }, // stored as milliseconds since Unix epoch UTC
}

// -------- Date Parsing Helpers --------
fn parse_date_string(s: &str) -> Option<(String, i64)> {
    // Fast pre-checks
    let t = s.trim();
    if t.is_empty() { return None; }

    // Accept a subset of common patterns. We attempt explicit layouts to avoid false positives.
    // We keep ordering from most to least constrained.
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

    // Helper to build ms from date + optional time
    fn to_ms(date: NaiveDate, time: NaiveTime) -> i64 {
        let dt = NaiveDateTime::new(date, time);
        dt.and_utc().timestamp_millis()
    }

    // ISO date / datetime variants
    if let Ok(date) = chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d") {
        return Some((format!("{}T00:00:00", date.format("%Y-%m-%d")), to_ms(date, NaiveTime::from_hms_opt(0,0,0)?)));
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S") {
        return Some((dt.format("%Y-%m-%dT%H:%M:%S").to_string(), dt.and_utc().timestamp_millis()));
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S") {
        return Some((dt.format("%Y-%m-%dT%H:%M:%S").to_string(), dt.and_utc().timestamp_millis()));
    }

    // European style: DD.MM.YYYY
    if t.len() >= 10 && t.chars().nth(2) == Some('.') && t.chars().nth(5) == Some('.') {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%d.%m.%Y") {
            return Some((format!("{}T00:00:00", d.format("%Y-%m-%d")), to_ms(d, NaiveTime::from_hms_opt(0,0,0)?)));
        }
    }
    // Slash formats: try unambiguous first (YYYY/MM/DD)
    if t.len() >= 10 && t.chars().nth(4) == Some('/') && t.chars().nth(7) == Some('/') {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%Y/%m/%d") {
            return Some((format!("{}T00:00:00", d.format("%Y-%m-%d")), to_ms(d, NaiveTime::from_hms_opt(0,0,0)?)));
        }
    }
    // MM/DD/YYYY or DD/MM/YYYY: attempt to disambiguate.
    if t.len() >= 10 && t.chars().nth(2) == Some('/') && t.chars().nth(5) == Some('/') {
        let a = &t[0..2];
        let b = &t[3..5];
        let year = &t[6..10];
        if a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()) && year.chars().all(|c| c.is_ascii_digit()) {
            let ai: u32 = a.parse().ok()?;
            let bi: u32 = b.parse().ok()?;
            // If one component > 12, that must be the day.
            if ai > 12 && bi <= 12 { // DD/MM/YYYY
                if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%d/%m/%Y") {
                    return Some((format!("{}T00:00:00", d.format("%Y-%m-%d")), to_ms(d, NaiveTime::from_hms_opt(0,0,0)?)));
                }
            } else if bi > 12 && ai <= 12 { // MM/DD/YYYY
                if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%m/%d/%Y") {
                    return Some((format!("{}T00:00:00", d.format("%Y-%m-%d")), to_ms(d, NaiveTime::from_hms_opt(0,0,0)?)));
                }
            } else if ai <= 12 && bi <= 12 { // ambiguous -> skip (avoid false positives)
            } else { // both >12 invalid -> skip
            }
        }
    }

    None
}

impl From<JsonValue> for Cell {
    fn from(v: JsonValue) -> Self {
        use JsonValue::*;
        match v {
            Null => Cell::Null,
            Bool(b) => Cell::Bool(b),
            Number(n) => {
                if let Some(i) = n.as_i64() {
                    Cell::Int(i)
                } else if let Some(u) = n.as_u64() {
                    // avoid lossy cast; preserve as string if it doesn't fit i64
                    if u <= i64::MAX as u64 {
                        Cell::Int(u as i64)
                    } else {
                        Cell::Str(Arc::<str>::from(u.to_string()))
                    }
                } else if let Some(f) = n.as_f64() {
                    Cell::Float(f)
                } else {
                    Cell::Str(Arc::<str>::from(n.to_string()))
                }
            }
            String(s) => {
                if let Some((iso, ms)) = parse_date_string(&s) {
                    Cell::Date { iso: Arc::<str>::from(iso), ms }
                } else {
                    Cell::Str(Arc::<str>::from(s))
                }
            }
            _ => Cell::Str(Arc::<str>::from(
                flow_like_types::json::to_string(&v).unwrap_or_default(),
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CSVTable {
    headers: Arc<[Arc<str>]>,
    rows: Vec<Box<[Cell]>>,
    source: Option<FlowPath>,
}

impl CSVTable {
    /// Build from serde values (keeps pins fully serializable).
    pub fn new(headers: Vec<String>, rows: Vec<Vec<JsonValue>>, source: Option<FlowPath>) -> Self {
        let headers: Arc<[Arc<str>]> = headers
            .into_iter()
            .map(|s| Arc::<str>::from(s))
            .collect::<Vec<_>>()
            .into();

        let rows: Vec<Box<[Cell]>> = rows
            .into_iter()
            .map(|r| r.into_iter().map(Cell::from).collect::<Vec<_>>().into_boxed_slice())
            .collect();

        Self { headers, rows, source }
    }

    #[inline]
    fn ncols(&self) -> usize {
        self.headers.len()
    }

    #[inline]
    fn nrows(&self) -> usize {
        self.rows.len()
    }

    /// Infer per-column kind, and collect string byte stats for LargeUtf8 decisions.
    fn infer_col_kind_and_string_stats(&self, col_idx: usize) -> (ColKind, u64 /*total_bytes*/, u32 /*max_len*/) {
        let mut kind = ColKind::Unknown;
        let mut total_bytes: u64 = 0;
        let mut max_len: u32 = 0;
        let mut coercible = true; // switch to false once a non-coercible string is seen

        for row in &self.rows {
            let Some(cell) = row.get(col_idx) else { continue };
            match cell {
                Cell::Null => {}
                Cell::Bool(_) => kind = merge_kind(kind, ColKind::Bool),
                Cell::Int(_) => kind = merge_kind(kind, ColKind::Int),
                Cell::Float(_) => kind = merge_kind(kind, ColKind::Float),
                Cell::Date { .. } => kind = merge_kind(kind, ColKind::Date),
                Cell::Str(s) => {
                    if is_nullish(s) {
                        continue;
                    }
                    let len = s.len() as u32;
                    max_len = max_len.max(len);
                    total_bytes = total_bytes.saturating_add(len as u64);

                    if coercible {
                        if is_bool_str(s).is_some() {
                            kind = merge_kind(kind, ColKind::Bool);
                        } else if parse_date_string(s).is_some() {
                            kind = merge_kind(kind, ColKind::Date);
                        } else if s.parse::<i64>().is_ok() {
                            kind = merge_kind(kind, ColKind::Int);
                        } else if s.parse::<f64>().is_ok() {
                            kind = merge_kind(kind, ColKind::Float);
                        } else {
                            kind = ColKind::Utf8;
                            coercible = false;
                        }
                    } else {
                        // already Utf8; just continue stats
                    }
                }
            }
        }

        if let ColKind::Unknown = kind {
            kind = ColKind::Utf8; // empty column → Utf8 nullable
        }
        (kind, total_bytes, max_len)
    }

    fn inferred_arrow_type(&self, col_idx: usize) -> DataType {
        const I32_MAX: u64 = i32::MAX as u64;
        let (kind, total_bytes, max_len) = self.infer_col_kind_and_string_stats(col_idx);
        match kind {
            ColKind::Bool => DataType::Boolean,
            ColKind::Int => DataType::Int64,
            ColKind::Float => DataType::Float64,
            ColKind::Date => DataType::Date64,
            ColKind::Utf8 | ColKind::Unknown => {
                if total_bytes >= I32_MAX || (max_len as u64) >= I32_MAX {
                    DataType::LargeUtf8
                } else {
                    DataType::Utf8
                }
            }
        }
    }

    pub fn arrow_schema(&self) -> Arc<ArrowSchema> {
        let fields = (0..self.ncols())
            .map(|col_idx| {
                let dt = self.inferred_arrow_type(col_idx);
                // always nullable → safe unions/joins
                Field::new(self.headers[col_idx].as_ref(), dt, true)
            })
            .collect::<Vec<_>>();
        Arc::new(ArrowSchema::new(fields))
    }

    pub fn to_record_batch(&self) -> Result<RecordBatch> {
        use DataType::*;

        let schema = self.arrow_schema();
        let nrows = self.nrows();
        let mut arrays: Vec<ArrayRef> = Vec::with_capacity(self.ncols());

        for col_idx in 0..self.ncols() {
            match schema.field(col_idx).data_type() {
                Boolean => {
                    let mut b = BooleanBuilder::with_capacity(nrows);
                    for row in &self.rows {
                        match row.get(col_idx) {
                            Some(Cell::Bool(x)) => b.append_value(*x),
                            Some(Cell::Int(i)) => b.append_value(*i != 0),
                            Some(Cell::Float(f)) => b.append_value(*f != 0.0),
                            Some(Cell::Date { .. }) => b.append_null(),
                            Some(Cell::Str(s)) => {
                                if let Some(x) = is_bool_str(s) {
                                    b.append_value(x)
                                } else if let Ok(i) = s.parse::<i64>() {
                                    b.append_value(i != 0)
                                } else if let Ok(f) = s.parse::<f64>() {
                                    b.append_value(f != 0.0)
                                } else {
                                    b.append_null()
                                }
                            }
                            Some(Cell::Null) | None => b.append_null(),
                        }
                    }
                    arrays.push(Arc::new(b.finish()));
                }

                Int64 => {
                    let mut b = Int64Builder::with_capacity(nrows);
                    for row in &self.rows {
                        match row.get(col_idx) {
                            Some(Cell::Int(i)) => b.append_value(*i),
                            Some(Cell::Float(f)) => {
                                if f.is_finite() && f.fract() == 0.0 {
                                    // exact integer in f64’s range
                                    let v = *f as i64;
                                    if (v as f64) == *f { b.append_value(v) } else { b.append_null() }
                                } else {
                                    b.append_null()
                                }
                            }
                            Some(Cell::Bool(bv)) => b.append_value(if *bv { 1 } else { 0 }),
                            Some(Cell::Date { .. }) => b.append_null(),
                            Some(Cell::Str(s)) => {
                                if let Ok(i) = s.parse::<i64>() { b.append_value(i) } else { b.append_null() }
                            }
                            Some(Cell::Null) | None => b.append_null(),
                        }
                    }
                    arrays.push(Arc::new(b.finish()));
                }

                Float64 => {
                    let mut b = Float64Builder::with_capacity(nrows);
                    for row in &self.rows {
                        match row.get(col_idx) {
                            Some(Cell::Float(f)) => b.append_value(*f),
                            Some(Cell::Int(i)) => b.append_value(*i as f64),
                            Some(Cell::Bool(bv)) => b.append_value(if *bv { 1.0 } else { 0.0 }),
                            Some(Cell::Date { .. }) => b.append_null(),
                            Some(Cell::Str(s)) => {
                                if let Ok(f) = s.parse::<f64>() { b.append_value(f) }
                                else if let Some(x) = is_bool_str(s) { b.append_value(if x { 1.0 } else { 0.0 }) }
                                else { b.append_null() }
                            }
                            Some(Cell::Null) | None => b.append_null(),
                        }
                    }
                    arrays.push(Arc::new(b.finish()));
                }

                Date64 => {
                    let mut b = Date64Builder::with_capacity(nrows);
                    for row in &self.rows {
                        match row.get(col_idx) {
                            Some(Cell::Date { ms, .. }) => b.append_value(*ms),
                            Some(Cell::Str(s)) => {
                                if let Some((_, ms)) = parse_date_string(s) { b.append_value(ms) } else { b.append_null() }
                            }
                            _ => b.append_null(),
                        }
                    }
                    arrays.push(Arc::new(b.finish()));
                }

                Utf8 => {
                    let mut b = StringBuilder::with_capacity(nrows, nrows * 8);
                    for row in &self.rows {
                        match row.get(col_idx) {
                            Some(Cell::Null) | None => b.append_null(),
                            Some(Cell::Str(s)) => b.append_value(s.as_ref()),
                            Some(Cell::Bool(v)) => b.append_value(if *v { "true" } else { "false" }),
                            Some(Cell::Int(i)) => b.append_value(i.to_string()),
                            Some(Cell::Float(f)) => b.append_value(f.to_string()),
                            Some(Cell::Date { iso, .. }) => b.append_value(iso.as_ref()),
                        }
                    }
                    arrays.push(Arc::new(b.finish()));
                }

                LargeUtf8 => {
                    let mut b = LargeStringBuilder::with_capacity(nrows, nrows * 8);
                    for row in &self.rows {
                        match row.get(col_idx) {
                            Some(Cell::Null) | None => b.append_null(),
                            Some(Cell::Str(s)) => b.append_value(s.as_ref()),
                            Some(Cell::Bool(v)) => b.append_value(if *v { "true" } else { "false" }),
                            Some(Cell::Int(i)) => b.append_value(i.to_string()),
                            Some(Cell::Float(f)) => b.append_value(f.to_string()),
                            Some(Cell::Date { iso, .. }) => b.append_value(iso.as_ref()),
                        }
                    }
                    arrays.push(Arc::new(b.finish()));
                }

                other => {
                    // Shouldn’t happen with our inference, but keep a safe fallback
                    bail!("Unsupported inferred data type in CSVTable: {other:?}")
                }
            }
        }

        RecordBatch::try_new(schema, arrays).map_err(|e| anyhow!(e))
    }

    pub fn to_memtable(&self) -> Result<Arc<MemTable>> {
        let batch = self.to_record_batch()?;
        let schema = batch.schema();
        let mem = MemTable::try_new(schema, vec![vec![batch]])?;
        Ok(Arc::new(mem))
    }

    pub fn register_with_datafusion(
        &self,
        ctx: &SessionContext,
        table_name: &str,
    ) -> Result<()> {
        let mem = self.to_memtable()?;
        ctx.register_table(table_name, mem)?;
        Ok(())
    }
}

/// Parses a 1-based row index from a string.
pub fn parse_row_1_based(s: &str) -> flow_like_types::Result<u32> {
    let trimmed = s.trim();
    let n: u32 = trimmed
        .parse()
        .map_err(|e| flow_like_types::anyhow!("Invalid row '{}': {}", s, e))?;
    if n == 0 {
        return Err(flow_like_types::anyhow!("Row must be 1-based (>=1): {}", s));
    }
    Ok(n)
}

/// Parses a 1-based column index from a string. Accepts letters (A, AA, XFD) or a number.
pub fn parse_col_1_based(s: &str) -> flow_like_types::Result<u32> {
    let trimmed = s.trim();

    // If it's a number, use it directly (1-based)
    if let Ok(n) = trimmed.parse::<u32>() {
        if n == 0 {
            return Err(flow_like_types::anyhow!(
                "Column number must be 1-based (>=1): {}",
                s
            ));
        }
        return Ok(n);
    }

    // Otherwise treat as letters
    let mut acc: u32 = 0;
    for ch in trimmed.to_ascii_uppercase().chars() {
        if !(('A'..='Z').contains(&ch)) {
            return Err(flow_like_types::anyhow!(
                "Invalid column '{}': only letters A-Z or a positive number are allowed",
                s
            ));
        }
        let v = (ch as u32) - ('A' as u32) + 1; // A=1
        acc = acc
            .checked_mul(26)
            .and_then(|x| x.checked_add(v))
            .ok_or_else(|| flow_like_types::anyhow!("Column index overflow for '{}': too large", s))?;
    }

    if acc == 0 {
        return Err(flow_like_types::anyhow!("Column must not be empty"));
    }

    Ok(acc)
}


pub async fn register_functions() -> Vec<Arc<dyn NodeLogic>> {
    let mut nodes: Vec<Arc<dyn NodeLogic>> = vec![];
    nodes.push(Arc::new(try_extract_tables::ExtractExcelTablesNode::new()));
    nodes.push(Arc::new(read_cell::ReadCellNode::new()));
    nodes.push(Arc::new(write_cell::WriteCellNode::new()));
    nodes.push(Arc::new(write_cell_html::WriteCellHtmlNode::new()));

    nodes
}
