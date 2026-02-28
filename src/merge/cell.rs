//! 셀 영역 병합 및 2D 배열 구성
//!
//! 탐지된 셀 영역(CellRegion)에 텍스트 박스를 매핑하여
//! 최종적으로 행/열 기반의 2D 문자열 배열을 생성한다.

use crate::{TextBox, Table, BBox, error::TrexError};
use crate::detect::CellRegion;

/// 셀 영역과 텍스트 박스를 매핑하여 2D 배열을 생성한다.
///
/// # Arguments
/// * `cells` - 탐지된 셀 영역 목록
/// * `text_boxes` - 페이지 내 텍스트 박스 목록
/// * `page` - 페이지 번호
/// * `table_index` - 테이블 인덱스
pub fn merge_cells(
    _cells: &[CellRegion],
    _text_boxes: &[TextBox],
    _page: u32,
    _table_index: usize,
) -> Result<Table, TrexError> {
    // Phase 1 구현 예정:
    // 1. 각 셀 내부에 포함되는 텍스트 박스 찾기 (contains 판정)
    // 2. 같은 셀에 여러 텍스트가 있으면 결합
    // 3. 첫 행을 헤더로, 나머지를 데이터로 분리
    // 4. Table 구조체로 반환
    todo!("셀 병합 구현 예정")
}

/// 바운딩 박스가 셀 영역 내부에 포함되는지 판정한다.
pub fn contains(cell: &CellRegion, bbox: &BBox) -> bool {
    // 텍스트 박스의 중심점이 셀 영역 내부에 있는지 체크
    let center_x = (bbox.x0 + bbox.x1) / 2.0;
    let center_y = (bbox.y0 + bbox.y1) / 2.0;

    center_x >= cell.x0 && center_x <= cell.x1
        && center_y >= cell.y0 && center_y <= cell.y1
}
