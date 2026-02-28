//! Lattice 모드 — 격자선 기반 테이블 탐지
//!
//! PDF의 벡터 그래픽(수평선/수직선)에서 교차점을 계산하여
//! 셀 그리드를 구성한다. OpenCV 없이 동작한다.
//!
//! ## 알고리즘 개요
//!
//! 1. 수평선과 수직선을 분류
//! 2. 유사한 좌표의 선분을 허용 오차 기반으로 그룹핑
//! 3. 교차점(Intersection) 계산
//! 4. 교차점으로부터 셀 그리드 생성
//! 5. 각 셀 내부의 텍스트 박스 매핑

use crate::{TextBox, Table, error::TrexError};
use crate::pdf::text::Line;

/// 격자선 기반으로 테이블을 탐지한다.
///
/// # Arguments
/// * `text_boxes` - 페이지 내 텍스트 박스 목록
/// * `lines` - 페이지 내 선분 목록
/// * `page` - 페이지 번호
pub fn detect(
    _text_boxes: &[TextBox],
    _lines: &[Line],
    _page: u32,
) -> Result<Vec<Table>, TrexError> {
    // Phase 1 구현 예정:
    // 1. 수평선/수직선 분류
    // 2. 교차점 계산 (허용 오차 기반 스냅핑)
    // 3. 셀 그리드 생성
    // 4. 텍스트 매핑
    todo!("Lattice 모드 구현 예정")
}
