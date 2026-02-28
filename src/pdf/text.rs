//! PDF 페이지에서 텍스트 박스(위치 + 내용) 추출
//!
//! PDF의 Content Stream을 파싱하여 각 텍스트 요소의
//! 바운딩 박스(x0, y0, x1, y1)와 문자열을 추출한다.

use crate::{TextBox, error::TrexError};

/// 특정 페이지에서 텍스트 박스 목록을 추출한다.
///
/// # Arguments
/// * `doc` - PDF 문서 참조
/// * `page_number` - 페이지 번호 (1-indexed)
///
/// # Returns
/// 텍스트 박스 목록 (위치 정보 포함)
pub fn extract_text_boxes(
    _doc: &lopdf::Document,
    _page_number: u32,
) -> Result<Vec<TextBox>, TrexError> {
    // Phase 1 구현 예정:
    // 1. Content Stream에서 텍스트 연산자(Tj, TJ, ', ") 파싱
    // 2. 텍스트 행렬(Tm)에서 위치 계산
    // 3. 폰트 크기로 바운딩 박스 추정
    todo!("텍스트 박스 추출 구현 예정")
}

/// 특정 페이지에서 선분(Line) 목록을 추출한다.
/// Lattice 모드에서 격자선 탐지에 사용된다.
///
/// # Arguments
/// * `doc` - PDF 문서 참조
/// * `page_number` - 페이지 번호 (1-indexed)
///
/// # Returns
/// 선분 목록 (시작점, 끝점)
pub fn extract_lines(
    _doc: &lopdf::Document,
    _page_number: u32,
) -> Result<Vec<Line>, TrexError> {
    // Phase 1 구현 예정:
    // 1. Content Stream에서 path 연산자(m, l, re) 파싱
    // 2. 수평선과 수직선 분류
    // 3. 허용 오차 기반 스냅핑
    todo!("선분 추출 구현 예정")
}

/// PDF에서 추출된 선분
#[derive(Debug, Clone)]
pub struct Line {
    /// 시작점 X
    pub x0: f64,
    /// 시작점 Y
    pub y0: f64,
    /// 끝점 X
    pub x1: f64,
    /// 끝점 Y
    pub y1: f64,
    /// 선의 방향
    pub direction: LineDirection,
}

/// 선분 방향
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineDirection {
    /// 수평선
    Horizontal,
    /// 수직선
    Vertical,
}
