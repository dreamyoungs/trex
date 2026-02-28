//! Stream 모드 — 좌표 기반 테이블 추론
//!
//! 격자선이 없는 표에서 텍스트 박스의 좌표를 분석하여
//! 가상의 행(Row)과 열(Column)을 추론한다.
//!
//! ## 알고리즘 개요
//!
//! 1. Y좌표 기반으로 텍스트 박스를 행(Row)으로 그룹핑
//! 2. X좌표 분포를 분석하여 열(Column) 경계 추론
//! 3. 가상의 셀 그리드 생성
//! 4. 비어있는 셀은 빈 문자열로 채움

use crate::{TextBox, Table, error::TrexError};

/// 좌표 기반으로 테이블을 추론한다.
///
/// # Arguments
/// * `text_boxes` - 페이지 내 텍스트 박스 목록
/// * `page` - 페이지 번호
pub fn detect(
    _text_boxes: &[TextBox],
    _page: u32,
) -> Result<Vec<Table>, TrexError> {
    // Phase 1 구현 예정:
    // 1. Y좌표 허용 오차 그룹핑 → 행 분리
    // 2. X좌표 히스토그램 분석 → 열 경계 추론
    // 3. 그리드 매핑
    todo!("Stream 모드 구현 예정")
}
