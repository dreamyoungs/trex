//! 테이블 탐지 모듈
//!
//! Lattice(격자선 기반)와 Stream(좌표 기반) 두 가지 모드를 제공한다.

pub mod lattice;
pub mod stream;

use crate::{TextBox, Table, error::TrexError};
use crate::pdf::text::Line;

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

/// 자동 모드 — Lattice를 먼저 시도하고, 결과 없으면 Stream으로 전환한다.
pub fn detect_auto(
    text_boxes: &[TextBox],
    lines: &[Line],
    page: u32,
) -> Result<Vec<Table>, TrexError> {
    // Lattice 시도
    let tables = lattice::detect(text_boxes, lines, page)?;
    if !tables.is_empty() {
        return Ok(tables);
    }

    // Lattice 결과 없으면 Stream으로 전환
    stream::detect(text_boxes, page)
}
