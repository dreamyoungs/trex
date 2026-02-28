//! # TREX — Table Rust EXtractor
//!
//! PDF에서 표(Table)를 추출하여 구조화된 2D 데이터로 변환하는 Rust 엔진.
//!
//! ## 사용 예시
//!
//! ```rust,no_run
//! use trex::{extract, ExtractOptions, ParseMode};
//!
//! let options = ExtractOptions {
//!     pages: None,           // 전체 페이지
//!     mode: ParseMode::Auto, // 자동 모드 선택
//! };
//!
//! let tables = extract("invoice.pdf", &options).unwrap();
//! for table in &tables {
//!     println!("Page {}: {}행 x {}열", table.page, table.rows.len(), table.headers.len());
//! }
//! ```

pub mod detect;
pub mod error;
pub mod merge;
pub mod output;
pub mod pdf;

use std::path::Path;

pub use error::TrexError;

/// 추출 결과 — 하나의 테이블을 나타낸다.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Table {
    /// 테이블이 위치한 페이지 번호 (1-indexed)
    pub page: u32,
    /// 페이지 내 테이블 순서 (0-indexed)
    pub table_index: usize,
    /// 헤더 목록
    pub headers: Vec<String>,
    /// 데이터 행 (각 행은 문자열 배열)
    pub rows: Vec<Vec<String>>,
}

/// PDF에서 텍스트/선분의 물리적 위치를 나타내는 바운딩 박스.
#[derive(Debug, Clone)]
pub struct BBox {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

/// PDF에서 추출된 텍스트 요소.
#[derive(Debug, Clone)]
pub struct TextBox {
    /// 텍스트 내용
    pub text: String,
    /// 위치 정보
    pub bbox: BBox,
    /// 소속 페이지 (1-indexed)
    pub page: u32,
}

/// 파싱 모드 선택
#[derive(Debug, Clone, Copy, Default)]
pub enum ParseMode {
    /// 격자선이 있는 표 — 벡터 그래픽에서 선분을 탐지
    Lattice,
    /// 격자선이 없는 표 — 텍스트 좌표 기반 군집화
    Stream,
    /// 자동 선택 — Lattice를 먼저 시도하고, 결과 없으면 Stream으로 전환
    #[default]
    Auto,
}

/// 추출 옵션
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// 처리할 페이지 목록 (None이면 전체 페이지)
    pub pages: Option<Vec<u32>>,
    /// 파싱 모드
    pub mode: ParseMode,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            pages: None,
            mode: ParseMode::Auto,
        }
    }
}

/// PDF 파일에서 표를 추출한다.
///
/// # Arguments
///
/// * `path` - PDF 파일 경로
/// * `options` - 추출 옵션 (페이지, 파싱 모드 등)
///
/// # Returns
///
/// 추출된 테이블 목록. 표가 없으면 빈 배열을 반환한다.
pub fn extract<P: AsRef<Path>>(_path: P, _options: &ExtractOptions) -> Result<Vec<Table>, TrexError> {
    // Phase 1 구현 예정
    todo!("PDF 추출 엔진 구현 예정")
}
