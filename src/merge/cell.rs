//! 셀 영역 병합 및 2D 배열 구성
//!
//! 탐지된 셀 영역(CellRegion)에 텍스트 박스를 매핑하여
//! 최종적으로 행/열 기반의 2D 문자열 배열을 생성한다.

use crate::detect::CellRegion;
use crate::{BBox, Table, TextBox, error::TrexError, i18n};

/// 셀 영역과 텍스트 박스를 매핑하여 2D 배열을 생성한다.
///
/// # Arguments
/// * `cells` - 탐지된 셀 영역 목록
/// * `text_boxes` - 페이지 내 텍스트 박스 목록
/// * `page` - 페이지 번호
/// * `table_index` - 테이블 인덱스
pub fn merge_cells(
    cells: &[CellRegion],
    text_boxes: &[TextBox],
    page: u32,
    table_index: usize,
) -> Result<Table, TrexError> {
    if cells.is_empty() {
        return Err(TrexError::Merge(
            i18n::text("병합할 셀 영역이 없습니다", "No cell regions to merge").to_string(),
        ));
    }

    let row_count = cells
        .iter()
        .map(|cell| cell.row)
        .max()
        .map_or(0, |max_row| max_row + 1);
    let col_count = cells
        .iter()
        .map(|cell| cell.col)
        .max()
        .map_or(0, |max_col| max_col + 1);

    if row_count == 0 || col_count == 0 {
        return Err(TrexError::Merge(
            i18n::text("유효하지 않은 셀 그리드입니다", "Invalid cell grid").to_string(),
        ));
    }

    let mut grid = vec![vec![String::new(); col_count]; row_count];

    for cell in cells {
        let mut matches: Vec<&TextBox> = text_boxes
            .iter()
            .filter(|tb| contains(cell, &tb.bbox))
            .collect();

        matches.sort_by(|a, b| {
            let ay = (a.bbox.y0 + a.bbox.y1) / 2.0;
            let by = (b.bbox.y0 + b.bbox.y1) / 2.0;
            by.total_cmp(&ay)
                .then_with(|| a.bbox.x0.total_cmp(&b.bbox.x0))
        });

        let merged_text = matches
            .into_iter()
            .map(|tb| tb.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        grid[cell.row][cell.col] = merged_text;
    }

    let headers = grid.first().cloned().unwrap_or_default();
    let rows = if grid.len() > 1 {
        grid.into_iter().skip(1).collect()
    } else {
        Vec::new()
    };

    Ok(Table {
        page,
        table_index,
        headers,
        rows,
    })
}

/// 바운딩 박스가 셀 영역 내부에 포함되는지 판정한다.
pub fn contains(cell: &CellRegion, bbox: &BBox) -> bool {
    // 텍스트 박스의 중심점이 셀 영역 내부에 있는지 체크
    let center_x = (bbox.x0 + bbox.x1) / 2.0;
    let center_y = (bbox.y0 + bbox.y1) / 2.0;

    center_x >= cell.x0 && center_x <= cell.x1 && center_y >= cell.y0 && center_y <= cell.y1
}
