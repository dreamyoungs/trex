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

use super::CellRegion;
use crate::merge::cell::merge_cells;
use crate::pdf::text::{Line, LineDirection};
use crate::{Table, TextBox, error::TrexError};
use std::collections::{HashMap, HashSet, VecDeque};

const COORD_TOLERANCE: f64 = 2.0;
const MIN_CELL_SIZE: f64 = 4.0;

fn is_meaningful_table(table: &Table) -> bool {
    let col_count = table.headers.len();
    let row_count = table.rows.len() + 1;
    if col_count < 2 || row_count < 2 {
        return false;
    }

    let rows_iter = std::iter::once(&table.headers).chain(table.rows.iter());
    let non_empty_cells = rows_iter
        .clone()
        .flat_map(|row| row.iter())
        .filter(|cell| !cell.trim().is_empty())
        .count();

    let dense_rows = rows_iter
        .filter(|row| row.iter().filter(|cell| !cell.trim().is_empty()).count() >= 2)
        .count();

    non_empty_cells >= 4 && dense_rows >= 2
}

fn overlap_len(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    let start = a0.max(b0);
    let end = a1.min(b1);
    (end - start).max(0.0)
}

fn axis_gap(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    if a1 < b0 {
        b0 - a1
    } else if b1 < a0 {
        a0 - b1
    } else {
        0.0
    }
}

fn are_adjacent(left: &CellRegion, right: &CellRegion) -> bool {
    let horizontal_neighbor = left.row.abs_diff(right.row) <= 1
        && left.col.abs_diff(right.col) <= 2
        && overlap_len(left.y0, left.y1, right.y0, right.y1) > 0.0
        && axis_gap(left.x0, left.x1, right.x0, right.x1) <= COORD_TOLERANCE * 2.0;

    let vertical_neighbor = left.col.abs_diff(right.col) <= 1
        && left.row.abs_diff(right.row) <= 2
        && overlap_len(left.x0, left.x1, right.x0, right.x1) > 0.0
        && axis_gap(left.y0, left.y1, right.y0, right.y1) <= COORD_TOLERANCE * 2.0;

    horizontal_neighbor || vertical_neighbor
}

fn component_bounds(cells: &[CellRegion], indices: &[usize]) -> (f64, f64, f64, f64) {
    let mut x0 = f64::INFINITY;
    let mut y0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut y1 = f64::NEG_INFINITY;

    for &idx in indices {
        let cell = &cells[idx];
        x0 = x0.min(cell.x0);
        y0 = y0.min(cell.y0);
        x1 = x1.max(cell.x1);
        y1 = y1.max(cell.y1);
    }

    (x0, y0, x1, y1)
}

fn split_cell_components(cells: &[CellRegion]) -> Vec<Vec<CellRegion>> {
    if cells.is_empty() {
        return Vec::new();
    }

    let mut adjacency = vec![Vec::new(); cells.len()];
    for left in 0..cells.len() {
        for right in (left + 1)..cells.len() {
            if are_adjacent(&cells[left], &cells[right]) {
                adjacency[left].push(right);
                adjacency[right].push(left);
            }
        }
    }

    let mut visited = HashSet::new();
    let mut components: Vec<Vec<usize>> = Vec::new();

    for start in 0..cells.len() {
        if visited.contains(&start) {
            continue;
        }

        let mut queue = VecDeque::new();
        let mut component = Vec::new();

        visited.insert(start);
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            component.push(current);
            for &next in &adjacency[current] {
                if visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }

        components.push(component);
    }

    components.sort_by(|left, right| {
        let (lx0, _, _, ly1) = component_bounds(cells, left);
        let (rx0, _, _, ry1) = component_bounds(cells, right);
        ry1.total_cmp(&ly1).then_with(|| lx0.total_cmp(&rx0))
    });

    components
        .into_iter()
        .map(|indices| {
            let mut row_values: Vec<usize> = indices.iter().map(|&idx| cells[idx].row).collect();
            let mut col_values: Vec<usize> = indices.iter().map(|&idx| cells[idx].col).collect();

            row_values.sort_unstable();
            row_values.dedup();
            col_values.sort_unstable();
            col_values.dedup();

            let row_index: HashMap<usize, usize> = row_values
                .iter()
                .enumerate()
                .map(|(new_idx, old_idx)| (*old_idx, new_idx))
                .collect();
            let col_index: HashMap<usize, usize> = col_values
                .iter()
                .enumerate()
                .map(|(new_idx, old_idx)| (*old_idx, new_idx))
                .collect();

            let mut component_cells: Vec<CellRegion> = indices
                .into_iter()
                .map(|idx| {
                    let cell = &cells[idx];
                    CellRegion {
                        x0: cell.x0,
                        y0: cell.y0,
                        x1: cell.x1,
                        y1: cell.y1,
                        row: *row_index.get(&cell.row).unwrap_or(&0),
                        col: *col_index.get(&cell.col).unwrap_or(&0),
                    }
                })
                .collect();

            component_cells.sort_by(|left, right| {
                left.row
                    .cmp(&right.row)
                    .then_with(|| left.col.cmp(&right.col))
            });
            component_cells
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct AxisLine {
    coord: f64,
    start: f64,
    end: f64,
}

fn merge_axis_lines(mut lines: Vec<AxisLine>) -> Vec<AxisLine> {
    if lines.is_empty() {
        return lines;
    }

    lines.sort_by(|a, b| {
        a.coord
            .total_cmp(&b.coord)
            .then_with(|| a.start.total_cmp(&b.start))
    });

    let mut merged: Vec<AxisLine> = Vec::new();

    for line in lines {
        if let Some(last) = merged.last_mut() {
            let same_coord = (last.coord - line.coord).abs() <= COORD_TOLERANCE;
            let connected = line.start <= last.end + COORD_TOLERANCE;

            if same_coord && connected {
                last.coord = (last.coord + line.coord) / 2.0;
                last.start = last.start.min(line.start);
                last.end = last.end.max(line.end);
                continue;
            }
        }

        merged.push(line);
    }

    merged
}

fn cluster_coords(mut coords: Vec<f64>) -> Vec<f64> {
    if coords.is_empty() {
        return coords;
    }

    coords.sort_by(|a, b| a.total_cmp(b));

    let mut result = Vec::new();
    let mut cluster_start = coords[0];
    let mut cluster_count = 1.0;

    for coord in coords.into_iter().skip(1) {
        let cluster_center = cluster_start / cluster_count;
        if (coord - cluster_center).abs() <= COORD_TOLERANCE {
            cluster_start += coord;
            cluster_count += 1.0;
        } else {
            result.push(cluster_center);
            cluster_start = coord;
            cluster_count = 1.0;
        }
    }

    result.push(cluster_start / cluster_count);
    result
}

fn has_horizontal_edge(lines: &[AxisLine], y: f64, x_left: f64, x_right: f64) -> bool {
    lines.iter().any(|line| {
        (line.coord - y).abs() <= COORD_TOLERANCE
            && line.start <= x_left + COORD_TOLERANCE
            && line.end >= x_right - COORD_TOLERANCE
    })
}

fn has_vertical_edge(lines: &[AxisLine], x: f64, y_bottom: f64, y_top: f64) -> bool {
    lines.iter().any(|line| {
        (line.coord - x).abs() <= COORD_TOLERANCE
            && line.start <= y_bottom + COORD_TOLERANCE
            && line.end >= y_top - COORD_TOLERANCE
    })
}

/// 격자선 기반으로 테이블을 탐지한다.
///
/// # Arguments
/// * `text_boxes` - 페이지 내 텍스트 박스 목록
/// * `lines` - 페이지 내 선분 목록
/// * `page` - 페이지 번호
pub fn detect(text_boxes: &[TextBox], lines: &[Line], page: u32) -> Result<Vec<Table>, TrexError> {
    if lines.is_empty() {
        return Ok(Vec::new());
    }

    let horizontal_lines = merge_axis_lines(
        lines
            .iter()
            .filter(|line| line.direction == LineDirection::Horizontal)
            .map(|line| AxisLine {
                coord: (line.y0 + line.y1) / 2.0,
                start: line.x0.min(line.x1),
                end: line.x0.max(line.x1),
            })
            .collect(),
    );

    let vertical_lines = merge_axis_lines(
        lines
            .iter()
            .filter(|line| line.direction == LineDirection::Vertical)
            .map(|line| AxisLine {
                coord: (line.x0 + line.x1) / 2.0,
                start: line.y0.min(line.y1),
                end: line.y0.max(line.y1),
            })
            .collect(),
    );

    if horizontal_lines.len() < 2 || vertical_lines.len() < 2 {
        return Ok(Vec::new());
    }

    let mut x_coords = cluster_coords(vertical_lines.iter().map(|line| line.coord).collect());
    let mut y_coords = cluster_coords(horizontal_lines.iter().map(|line| line.coord).collect());

    if x_coords.len() < 2 || y_coords.len() < 2 {
        return Ok(Vec::new());
    }

    x_coords.sort_by(|a, b| a.total_cmp(b));
    y_coords.sort_by(|a, b| b.total_cmp(a));

    let mut cells = Vec::new();

    for row in 0..(y_coords.len() - 1) {
        let y_top = y_coords[row];
        let y_bottom = y_coords[row + 1];

        if y_top - y_bottom < MIN_CELL_SIZE {
            continue;
        }

        for col in 0..(x_coords.len() - 1) {
            let x_left = x_coords[col];
            let x_right = x_coords[col + 1];

            if x_right - x_left < MIN_CELL_SIZE {
                continue;
            }

            let closed_cell = has_horizontal_edge(&horizontal_lines, y_top, x_left, x_right)
                && has_horizontal_edge(&horizontal_lines, y_bottom, x_left, x_right)
                && has_vertical_edge(&vertical_lines, x_left, y_bottom, y_top)
                && has_vertical_edge(&vertical_lines, x_right, y_bottom, y_top);

            if closed_cell {
                cells.push(CellRegion {
                    x0: x_left,
                    y0: y_bottom,
                    x1: x_right,
                    y1: y_top,
                    row,
                    col,
                });
            }
        }
    }

    if cells.is_empty() {
        return Ok(Vec::new());
    }

    let components = split_cell_components(&cells);
    let mut tables = Vec::new();

    for component in components {
        let row_count = component
            .iter()
            .map(|cell| cell.row)
            .max()
            .map_or(0, |max_row| max_row + 1);
        let col_count = component
            .iter()
            .map(|cell| cell.col)
            .max()
            .map_or(0, |max_col| max_col + 1);

        if row_count < 2 || col_count < 2 {
            continue;
        }

        let table = merge_cells(&component, text_boxes, page, tables.len())?;
        if is_meaningful_table(&table) {
            tables.push(table);
        }
    }

    Ok(tables)
}
