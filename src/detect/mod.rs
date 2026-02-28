//! 테이블 탐지 모듈
//!
//! Lattice/Stream 탐지기와 DL 라우터를 제공한다.

pub mod dl;
pub mod lattice;
pub mod stream;

use crate::pdf::text::Line;
use crate::{Table, TextBox, error::TrexError};
use std::collections::HashSet;

/// 테이블 탐지 결과
#[derive(Debug)]
pub struct DetectionResult {
    /// 탐지된 셀 그리드
    pub cells: Vec<Vec<CellRegion>>,
    /// 사용된 파싱 모드
    pub mode: DetectionMode,
}

/// 탐지에 사용된 모드
#[derive(Debug, Clone, Copy)]
pub enum DetectionMode {
    Lattice,
    Stream,
}

/// 단일 셀 영역
#[derive(Debug, Clone)]
pub struct CellRegion {
    /// 셀 바운딩 박스
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    /// 행 인덱스
    pub row: usize,
    /// 열 인덱스
    pub col: usize,
}

pub(crate) fn table_non_empty_cells(table: &Table) -> usize {
    table
        .headers
        .iter()
        .chain(table.rows.iter().flat_map(|row| row.iter()))
        .filter(|cell| !cell.trim().is_empty())
        .count()
}

pub(crate) fn table_density(table: &Table) -> f32 {
    let non_empty = table_non_empty_cells(table) as f32;
    if non_empty <= 0.0 {
        return 0.0;
    }

    let row_count = (table.rows.len() + 1) as f32;
    let col_count = table.headers.len().max(1) as f32;
    let total_cells = (row_count * col_count).max(1.0);
    (non_empty / total_cells).clamp(0.0, 1.0)
}

pub(crate) fn table_quality_score(table: &Table) -> f32 {
    let non_empty = table_non_empty_cells(table) as f32;
    if non_empty <= 0.0 {
        return 0.0;
    }

    let row_count = (table.rows.len() + 1) as f32;
    let col_count = table.headers.len().max(1) as f32;
    let density = table_density(table);

    let col_penalty = if col_count > 40.0 {
        (40.0 / col_count).clamp(0.2, 1.0)
    } else {
        1.0
    };
    let row_penalty = if row_count > 120.0 {
        (120.0 / row_count).clamp(0.2, 1.0)
    } else {
        1.0
    };

    non_empty * density * col_penalty * row_penalty
}

fn table_tokens(table: &Table) -> HashSet<String> {
    table
        .headers
        .iter()
        .chain(table.rows.iter().flat_map(|row| row.iter()))
        .map(|cell| cell.trim().to_lowercase())
        .filter(|cell| !cell.is_empty())
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }

    let intersection = a.intersection(b).count() as f32;
    let union = a.union(b).count() as f32;
    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

pub(crate) fn similar_table(left: &Table, right: &Table) -> bool {
    let left_rows = left.rows.len() + 1;
    let right_rows = right.rows.len() + 1;
    let left_cols = left.headers.len();
    let right_cols = right.headers.len();

    let shape_close = left_rows.abs_diff(right_rows) <= 1 && left_cols.abs_diff(right_cols) <= 1;

    let left_tokens = table_tokens(left);
    let right_tokens = table_tokens(right);
    let token_similarity = jaccard(&left_tokens, &right_tokens);

    token_similarity >= 0.85 || (shape_close && token_similarity >= 0.55)
}

pub(crate) fn merge_detected_tables(mut primary: Vec<Table>, secondary: Vec<Table>) -> Vec<Table> {
    let mut primary_tokens: HashSet<String> = primary.iter().flat_map(table_tokens).collect();

    for candidate in secondary {
        let is_duplicate = primary
            .iter()
            .any(|existing| similar_table(existing, &candidate));

        if is_duplicate {
            continue;
        }

        let candidate_tokens = table_tokens(&candidate);
        if candidate_tokens.is_empty() {
            continue;
        }

        let primary_density = if primary.is_empty() {
            0.0
        } else {
            primary.iter().map(table_density).sum::<f32>() / primary.len() as f32
        };
        let candidate_density = table_density(&candidate);
        if !primary.is_empty() && primary_density >= 0.2 && candidate_density < 0.05 {
            continue;
        }

        let novel_token_count = candidate_tokens.difference(&primary_tokens).count();
        let novelty_ratio = novel_token_count as f32 / candidate_tokens.len() as f32;

        if !primary.is_empty() && novelty_ratio < 0.2 {
            continue;
        }

        primary_tokens.extend(candidate_tokens);
        primary.push(candidate);
    }

    primary
}

/// 자동 모드 — Lattice와 Stream 결과를 결합하여 누락 테이블을 줄인다.
pub fn detect_auto(
    text_boxes: &[TextBox],
    lines: &[Line],
    page: u32,
) -> Result<Vec<Table>, TrexError> {
    let lattice_tables = lattice::detect(text_boxes, lines, page)?;
    let stream_tables = stream::detect(text_boxes, page)?;

    match (lattice_tables.is_empty(), stream_tables.is_empty()) {
        (true, true) => Ok(Vec::new()),
        (false, true) => Ok(lattice_tables),
        (true, false) => Ok(stream_tables),
        (false, false) => {
            let lattice_score: f32 = lattice_tables.iter().map(table_quality_score).sum();
            let stream_score: f32 = stream_tables.iter().map(table_quality_score).sum();

            let merged = if lattice_score >= stream_score {
                merge_detected_tables(lattice_tables, stream_tables)
            } else {
                merge_detected_tables(stream_tables, lattice_tables)
            };

            Ok(merged)
        }
    }
}
