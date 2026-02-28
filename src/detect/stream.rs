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

use crate::{Table, TextBox, error::TrexError};

const MIN_TABLE_ROWS: usize = 2;
const MIN_TABLE_COLS: usize = 2;
const ROW_TOLERANCE_FLOOR: f64 = 2.0;
const COL_TOLERANCE_FLOOR: f64 = 4.0;
const ROW_SPLIT_GAP_FACTOR: f64 = 3.5;
const ROW_SPLIT_GAP_MIN: f64 = 32.0;
const MIN_SPLIT_SECTION_ROWS: usize = 4;

#[derive(Debug)]
struct RowGroup<'a> {
    anchor_y: f64,
    boxes: Vec<&'a TextBox>,
}

fn center_x(tb: &TextBox) -> f64 {
    (tb.bbox.x0 + tb.bbox.x1) / 2.0
}

fn center_y(tb: &TextBox) -> f64 {
    (tb.bbox.y0 + tb.bbox.y1) / 2.0
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;

    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

fn cluster_points(mut points: Vec<f64>, tolerance: f64) -> Vec<f64> {
    if points.is_empty() {
        return points;
    }

    points.sort_by(|a, b| a.total_cmp(b));

    let mut clusters = Vec::new();
    let mut sum = points[0];
    let mut count = 1.0;

    for point in points.into_iter().skip(1) {
        let center = sum / count;
        if (point - center).abs() <= tolerance {
            sum += point;
            count += 1.0;
        } else {
            clusters.push(center);
            sum = point;
            count = 1.0;
        }
    }

    clusters.push(sum / count);
    clusters
}

fn nearest_column(columns: &[f64], x: f64) -> usize {
    columns
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| (x - **left).abs().total_cmp(&(x - **right).abs()))
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn normalize_cell(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn trim_grid(mut grid: Vec<Vec<String>>) -> Vec<Vec<String>> {
    for row in &mut grid {
        for cell in row {
            *cell = normalize_cell(cell);
        }
    }

    grid.retain(|row| row.iter().any(|cell| !cell.is_empty()));
    if grid.is_empty() {
        return grid;
    }

    let column_count = grid[0].len();
    let keep_columns: Vec<usize> = (0..column_count)
        .filter(|&col| grid.iter().any(|row| !row[col].is_empty()))
        .collect();

    grid.into_iter()
        .map(|row| keep_columns.iter().map(|&col| row[col].clone()).collect())
        .collect()
}

fn table_non_empty_cells(table: &Table) -> usize {
    table
        .headers
        .iter()
        .chain(table.rows.iter().flat_map(|row| row.iter()))
        .filter(|cell| !cell.trim().is_empty())
        .count()
}

fn build_table(rows: &[RowGroup<'_>], page: u32, table_index: usize) -> Option<Table> {
    if rows.len() < MIN_TABLE_ROWS {
        return None;
    }

    let column_tolerance = {
        let widths: Vec<f64> = rows
            .iter()
            .flat_map(|row| row.boxes.iter())
            .map(|tb| (tb.bbox.x1 - tb.bbox.x0).abs())
            .filter(|w| *w > 0.0)
            .collect();
        (median(widths) * 0.75).max(COL_TOLERANCE_FLOOR)
    };

    let mut columns = cluster_points(
        rows.iter()
            .flat_map(|row| row.boxes.iter().map(|tb| center_x(tb)))
            .collect(),
        column_tolerance,
    );
    columns.sort_by(|a, b| a.total_cmp(b));

    if columns.len() < MIN_TABLE_COLS {
        return None;
    }

    let mut grid = vec![vec![String::new(); columns.len()]; rows.len()];

    for (row_index, row) in rows.iter().enumerate() {
        for tb in &row.boxes {
            let value = tb.text.trim();
            if value.is_empty() {
                continue;
            }

            let col_index = nearest_column(&columns, center_x(tb));
            let cell = &mut grid[row_index][col_index];

            if cell.is_empty() {
                cell.push_str(value);
            } else {
                cell.push(' ');
                cell.push_str(value);
            }
        }
    }

    let grid = trim_grid(grid);
    if grid.len() < MIN_TABLE_ROWS {
        return None;
    }

    let column_count = grid.first().map_or(0, Vec::len);
    if column_count < MIN_TABLE_COLS {
        return None;
    }

    let non_empty = grid
        .iter()
        .flat_map(|row| row.iter())
        .filter(|cell| !cell.is_empty())
        .count();
    if non_empty < MIN_TABLE_ROWS + MIN_TABLE_COLS {
        return None;
    }

    Some(Table {
        page,
        table_index,
        headers: grid.first().cloned().unwrap_or_default(),
        rows: grid.into_iter().skip(1).collect(),
    })
}

fn candidate_split_index(rows: &[RowGroup<'_>]) -> Option<usize> {
    if rows.len() < MIN_SPLIT_SECTION_ROWS * 2 {
        return None;
    }

    let gaps: Vec<f64> = rows
        .windows(2)
        .map(|pair| pair[0].anchor_y - pair[1].anchor_y)
        .filter(|gap| *gap > 0.0)
        .collect();

    if gaps.is_empty() {
        return None;
    }

    let threshold = (median(gaps.clone()) * ROW_SPLIT_GAP_FACTOR).max(ROW_SPLIT_GAP_MIN);

    gaps.iter()
        .enumerate()
        .filter(|(idx, gap)| {
            **gap > threshold
                && (*idx + 1) >= MIN_SPLIT_SECTION_ROWS
                && (rows.len() - (*idx + 1)) >= MIN_SPLIT_SECTION_ROWS
        })
        .max_by(|(_, left_gap), (_, right_gap)| left_gap.total_cmp(right_gap))
        .map(|(idx, _)| idx + 1)
}

/// 좌표 기반으로 테이블을 추론한다.
///
/// # Arguments
/// * `text_boxes` - 페이지 내 텍스트 박스 목록
/// * `page` - 페이지 번호
pub fn detect(text_boxes: &[TextBox], page: u32) -> Result<Vec<Table>, TrexError> {
    let mut candidates: Vec<&TextBox> = text_boxes
        .iter()
        .filter(|tb| !tb.text.trim().is_empty())
        .collect();

    if candidates.len() < 4 {
        return Ok(Vec::new());
    }

    candidates.sort_by(|a, b| center_y(b).total_cmp(&center_y(a)));

    let row_tolerance = {
        let heights: Vec<f64> = candidates
            .iter()
            .map(|tb| (tb.bbox.y1 - tb.bbox.y0).abs())
            .filter(|h| *h > 0.0)
            .collect();
        (median(heights) * 0.6).max(ROW_TOLERANCE_FLOOR)
    };

    let mut rows: Vec<RowGroup<'_>> = Vec::new();

    for tb in candidates {
        let y = center_y(tb);

        if let Some(row) = rows
            .iter_mut()
            .find(|row| (row.anchor_y - y).abs() <= row_tolerance)
        {
            let len = row.boxes.len() as f64;
            row.anchor_y = (row.anchor_y * len + y) / (len + 1.0);
            row.boxes.push(tb);
        } else {
            rows.push(RowGroup {
                anchor_y: y,
                boxes: vec![tb],
            });
        }
    }

    rows.sort_by(|a, b| b.anchor_y.total_cmp(&a.anchor_y));
    for row in &mut rows {
        row.boxes
            .sort_by(|left, right| left.bbox.x0.total_cmp(&right.bbox.x0));
    }

    if rows.len() < MIN_TABLE_ROWS {
        return Ok(Vec::new());
    }

    if let Some(split) = candidate_split_index(&rows) {
        let top = build_table(&rows[..split], page, 0);
        let bottom = build_table(&rows[split..], page, 1);

        if let (Some(top), Some(bottom)) = (top, bottom) {
            if table_non_empty_cells(&top) >= 8 && table_non_empty_cells(&bottom) >= 8 {
                return Ok(vec![top, bottom]);
            }
        }
    }

    match build_table(&rows, page, 0) {
        Some(table) => Ok(vec![table]),
        None => Ok(Vec::new()),
    }
}
