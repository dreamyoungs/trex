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
pub mod i18n;
pub mod merge;
pub mod output;
pub mod pdf;
pub mod telemetry;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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
    /// DL 라우터 기반 탐지 — 모델 추론 결과로 Lattice/Stream/혼합 전략 선택
    Dl,
    /// 자동 선택 — Lattice를 먼저 시도하고, 결과 없으면 Stream으로 전환
    #[default]
    Auto,
}

/// DL 모드에서 추론 신뢰도가 낮거나 모델 사용이 불가능할 때의 폴백 전략
#[derive(Debug, Clone, Copy, Default)]
pub enum DlFallbackMode {
    /// 기존 Auto 규칙(Lattice -> Stream)
    #[default]
    Auto,
    /// 강제 Lattice
    Lattice,
    /// 강제 Stream
    Stream,
}

/// DL 런타임 옵션
#[derive(Debug, Clone)]
pub struct DlRuntimeOptions {
    /// ONNX 라우터 모델 경로 (`dl` feature 활성화 시 사용)
    pub model_path: Option<PathBuf>,
    /// 라우터 신뢰도 최소값
    pub min_confidence: f32,
    /// 모델 미사용/저신뢰 시 폴백 전략
    pub fallback_mode: DlFallbackMode,
}

impl Default for DlRuntimeOptions {
    fn default() -> Self {
        Self {
            model_path: None,
            min_confidence: 0.55,
            fallback_mode: DlFallbackMode::Auto,
        }
    }
}

/// 추출 실행에 영향을 주는 런타임 옵션
#[derive(Debug, Clone, Default)]
pub struct RuntimeOptions {
    pub dl: DlRuntimeOptions,
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
pub fn extract<P: AsRef<Path>>(path: P, options: &ExtractOptions) -> Result<Vec<Table>, TrexError> {
    extract_with_runtime_options(path, options, &RuntimeOptions::default())
}

/// PDF 파일에서 표를 추출한다.
///
/// 기본 추출 옵션에 더해, DL 모델 경로/신뢰도 등 런타임 옵션을 함께 전달할 수 있다.
pub fn extract_with_runtime_options<P: AsRef<Path>>(
    path: P,
    options: &ExtractOptions,
    runtime: &RuntimeOptions,
) -> Result<Vec<Table>, TrexError> {
    let doc = pdf::reader::PdfDocument::open(path)?;
    let pages = resolve_pages(options.pages.as_deref(), doc.page_count())?;

    let mut all_tables = Vec::new();

    for page in pages {
        let text_boxes = pdf::text::extract_text_boxes(doc.inner(), page)?;

        let mut page_tables = match options.mode {
            ParseMode::Lattice => {
                let lines = pdf::text::extract_lines(doc.inner(), page)?;
                detect::lattice::detect(&text_boxes, &lines, page)?
            }
            ParseMode::Stream => detect::stream::detect(&text_boxes, page)?,
            ParseMode::Dl => {
                let lines = pdf::text::extract_lines(doc.inner(), page)?;
                detect::dl::detect(&text_boxes, &lines, page, runtime)?
            }
            ParseMode::Auto => {
                let lines = pdf::text::extract_lines(doc.inner(), page)?;
                detect::detect_auto(&text_boxes, &lines, page)?
            }
        };

        for (table_index, table) in page_tables.iter_mut().enumerate() {
            table.page = page;
            table.table_index = table_index;
        }

        all_tables.extend(page_tables);
    }

    Ok(all_tables)
}

fn resolve_pages(requested_pages: Option<&[u32]>, page_count: u32) -> Result<Vec<u32>, TrexError> {
    if page_count == 0 {
        return Ok(Vec::new());
    }

    match requested_pages {
        Some(pages) if !pages.is_empty() => {
            let mut unique_pages = BTreeSet::new();

            for &page in pages {
                if page == 0 || page > page_count {
                    let detail = if i18n::is_korean() {
                        format!(
                            "요청한 페이지 {}가 범위를 벗어났습니다. (1-{})",
                            page, page_count
                        )
                    } else {
                        format!(
                            "Requested page {} is out of range. (valid: 1-{})",
                            page, page_count
                        )
                    };
                    return Err(TrexError::PdfParse(detail));
                }
                unique_pages.insert(page);
            }

            Ok(unique_pages.into_iter().collect())
        }
        _ => Ok((1..=page_count).collect()),
    }
}
