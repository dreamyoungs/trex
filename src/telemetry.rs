//! 추출 이벤트 텔레메트리
//!
//! 운영 서비스에서 실패 케이스를 수집해 학습 루프로 연결할 수 있도록
//! NDJSON 기반 이벤트를 기록한다.

use crate::{DlFallbackMode, ParseMode, Table, TrexError};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct EventContext {
    pub tenant_id: Option<String>,
    pub request_id: Option<String>,
    pub document_key: String,
    pub training_opt_in: bool,
    pub feedback_tag: Option<String>,
    pub requested_mode: ParseMode,
    pub pages_requested: Option<Vec<u32>>,
    pub dl_model_path: Option<String>,
    pub dl_min_confidence: f32,
    pub dl_fallback: DlFallbackMode,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageMetric {
    pub page: u32,
    pub table_count: usize,
    pub non_empty_cells: usize,
    pub mean_density: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractionEvent {
    pub event_version: u8,
    pub ts_unix_ms: u128,
    pub status: String,
    pub tenant_id: Option<String>,
    pub request_id: Option<String>,
    pub document_key: String,
    pub training_opt_in: bool,
    pub feedback_tag: Option<String>,
    pub requested_mode: String,
    pub pages_requested: Option<Vec<u32>>,
    pub dl_model_path: Option<String>,
    pub dl_min_confidence: f32,
    pub dl_fallback: String,
    pub duration_ms: u128,
    pub table_count: Option<usize>,
    pub page_metrics: Option<Vec<PageMetric>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

pub fn success_event(context: EventContext, tables: &[Table]) -> ExtractionEvent {
    let page_metrics = build_page_metrics(tables);

    ExtractionEvent {
        event_version: 1,
        ts_unix_ms: now_unix_ms(),
        status: "ok".to_string(),
        tenant_id: context.tenant_id,
        request_id: context.request_id,
        document_key: context.document_key,
        training_opt_in: context.training_opt_in,
        feedback_tag: context.feedback_tag,
        requested_mode: parse_mode_name(context.requested_mode).to_string(),
        pages_requested: context.pages_requested,
        dl_model_path: context.dl_model_path,
        dl_min_confidence: context.dl_min_confidence,
        dl_fallback: dl_fallback_name(context.dl_fallback).to_string(),
        duration_ms: context.duration_ms,
        table_count: Some(tables.len()),
        page_metrics: Some(page_metrics),
        error_code: None,
        error_message: None,
    }
}

pub fn error_event(context: EventContext, error: &TrexError) -> ExtractionEvent {
    ExtractionEvent {
        event_version: 1,
        ts_unix_ms: now_unix_ms(),
        status: "error".to_string(),
        tenant_id: context.tenant_id,
        request_id: context.request_id,
        document_key: context.document_key,
        training_opt_in: context.training_opt_in,
        feedback_tag: context.feedback_tag,
        requested_mode: parse_mode_name(context.requested_mode).to_string(),
        pages_requested: context.pages_requested,
        dl_model_path: context.dl_model_path,
        dl_min_confidence: context.dl_min_confidence,
        dl_fallback: dl_fallback_name(context.dl_fallback).to_string(),
        duration_ms: context.duration_ms,
        table_count: None,
        page_metrics: None,
        error_code: Some(error_code(error).to_string()),
        error_message: Some(error.to_string()),
    }
}

pub fn append_event<P: AsRef<Path>>(path: P, event: &ExtractionEvent) -> Result<(), TrexError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, event)?;
    file.write_all(b"\n")?;

    Ok(())
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn parse_mode_name(mode: ParseMode) -> &'static str {
    match mode {
        ParseMode::Lattice => "lattice",
        ParseMode::Stream => "stream",
        ParseMode::Dl => "dl",
        ParseMode::Auto => "auto",
    }
}

fn dl_fallback_name(mode: DlFallbackMode) -> &'static str {
    match mode {
        DlFallbackMode::Auto => "auto",
        DlFallbackMode::Lattice => "lattice",
        DlFallbackMode::Stream => "stream",
    }
}

fn error_code(error: &TrexError) -> &'static str {
    match error {
        TrexError::PdfParse(_) => "pdf_parse",
        TrexError::Detection(_) => "detection",
        TrexError::Merge(_) => "merge",
        TrexError::Output(_) => "output",
        TrexError::Dl(_) => "dl",
        TrexError::Io(_) => "io",
        TrexError::Json(_) => "json",
    }
}

fn build_page_metrics(tables: &[Table]) -> Vec<PageMetric> {
    let mut grouped = BTreeMap::<u32, (usize, usize, f32)>::new();

    for table in tables {
        let entry = grouped.entry(table.page).or_insert((0, 0, 0.0));
        entry.0 += 1;

        let row_count = table.rows.len() + 1;
        let col_count = table.headers.len();
        let total_cells = (row_count * col_count).max(1);

        let non_empty_cells = table
            .headers
            .iter()
            .chain(table.rows.iter().flat_map(|row| row.iter()))
            .filter(|cell| !cell.trim().is_empty())
            .count();

        entry.1 += non_empty_cells;
        entry.2 += non_empty_cells as f32 / total_cells as f32;
    }

    grouped
        .into_iter()
        .map(
            |(page, (table_count, non_empty_cells, density_sum))| PageMetric {
                page,
                table_count,
                non_empty_cells,
                mean_density: if table_count > 0 {
                    density_sum / table_count as f32
                } else {
                    0.0
                },
            },
        )
        .collect()
}
