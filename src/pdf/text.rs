//! PDF 페이지에서 텍스트 박스(위치 + 내용) 추출
//!
//! PDF의 Content Stream을 파싱하여 각 텍스트 요소의
//! 바운딩 박스(x0, y0, x1, y1)와 문자열을 추출한다.

use crate::{BBox, TextBox, error::TrexError};
use lopdf::{
    Dictionary, Document, Object, ObjectId,
    content::{Content, Operation},
};
use std::collections::{BTreeMap, HashSet};

const DEFAULT_FONT_SIZE: f64 = 12.0;
const MIN_LINE_LENGTH: f64 = 3.0;
const POSITION_TOLERANCE: f64 = 1.5;

#[derive(Debug, Clone)]
struct TextState {
    in_text_object: bool,
    current_x: f64,
    current_y: f64,
    line_start_x: f64,
    font_size: f64,
    leading: f64,
    font_name: Option<Vec<u8>>,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            in_text_object: false,
            current_x: 0.0,
            current_y: 0.0,
            line_start_x: 0.0,
            font_size: DEFAULT_FONT_SIZE,
            leading: DEFAULT_FONT_SIZE * 1.2,
            font_name: None,
        }
    }
}

impl TextState {
    fn next_line(&mut self) {
        let line_gap = if self.leading > 0.0 {
            self.leading
        } else {
            self.font_size * 1.2
        };
        self.current_y -= line_gap;
        self.current_x = self.line_start_x;
    }
}

/// 특정 페이지에서 텍스트 박스 목록을 추출한다.
///
/// # Arguments
/// * `doc` - PDF 문서 참조
/// * `page_number` - 페이지 번호 (1-indexed)
///
/// # Returns
/// 텍스트 박스 목록 (위치 정보 포함)
pub fn extract_text_boxes(
    doc: &lopdf::Document,
    page_number: u32,
) -> Result<Vec<TextBox>, TrexError> {
    let pages = doc.get_pages();
    let page_id = pages
        .get(&page_number)
        .copied()
        .ok_or_else(|| TrexError::PdfParse(format!("페이지 {}를 찾을 수 없습니다", page_number)))?;

    let fonts: BTreeMap<Vec<u8>, &Dictionary> = doc.get_page_fonts(page_id).unwrap_or_default();
    let operations = decode_page_operations(doc, page_id);

    let mut text_boxes = Vec::new();
    let mut state = TextState::default();

    for op in &operations {
        match op.operator.as_str() {
            "BT" => {
                state.in_text_object = true;
            }
            "ET" => {
                state.in_text_object = false;
            }
            "Tf" => {
                if let Some(font_name) = op.operands.first().and_then(|obj| obj.as_name().ok()) {
                    state.font_name = Some(font_name.to_vec());
                }

                if let Some(font_size) = op.operands.get(1).and_then(object_as_f64) {
                    state.font_size = font_size.abs().max(1.0);
                }
            }
            "TL" => {
                if let Some(leading) = op.operands.first().and_then(object_as_f64) {
                    state.leading = leading.abs();
                }
            }
            "Tm" => {
                if op.operands.len() >= 6 {
                    if let Some(x) = op.operands.get(4).and_then(object_as_f64) {
                        state.current_x = x;
                        state.line_start_x = x;
                    }
                    if let Some(y) = op.operands.get(5).and_then(object_as_f64) {
                        state.current_y = y;
                    }
                }
            }
            "Td" => {
                if let (Some(tx), Some(ty)) = (
                    op.operands.first().and_then(object_as_f64),
                    op.operands.get(1).and_then(object_as_f64),
                ) {
                    state.current_x += tx;
                    state.current_y += ty;
                    state.line_start_x = state.current_x;
                }
            }
            "TD" => {
                if let (Some(tx), Some(ty)) = (
                    op.operands.first().and_then(object_as_f64),
                    op.operands.get(1).and_then(object_as_f64),
                ) {
                    state.leading = ty.abs();
                    state.current_x += tx;
                    state.current_y += ty;
                    state.line_start_x = state.current_x;
                }
            }
            "T*" => {
                state.next_line();
            }
            "Tj" => {
                if !state.in_text_object {
                    continue;
                }
                let text = op
                    .operands
                    .first()
                    .map(|operand| {
                        decode_text_operand(doc, &fonts, state.font_name.as_deref(), operand)
                    })
                    .unwrap_or_default();
                push_text_box(&mut text_boxes, &mut state, text, page_number);
            }
            "TJ" => {
                if !state.in_text_object {
                    continue;
                }
                let text = op
                    .operands
                    .first()
                    .map(|operand| {
                        decode_text_operand(doc, &fonts, state.font_name.as_deref(), operand)
                    })
                    .unwrap_or_default();
                push_text_box(&mut text_boxes, &mut state, text, page_number);
            }
            "'" => {
                if !state.in_text_object {
                    continue;
                }
                state.next_line();
                let text = op
                    .operands
                    .first()
                    .map(|operand| {
                        decode_text_operand(doc, &fonts, state.font_name.as_deref(), operand)
                    })
                    .unwrap_or_default();
                push_text_box(&mut text_boxes, &mut state, text, page_number);
            }
            "\"" => {
                if !state.in_text_object {
                    continue;
                }
                state.next_line();
                let text = op
                    .operands
                    .get(2)
                    .map(|operand| {
                        decode_text_operand(doc, &fonts, state.font_name.as_deref(), operand)
                    })
                    .unwrap_or_default();
                push_text_box(&mut text_boxes, &mut state, text, page_number);
            }
            _ => {}
        }
    }

    Ok(text_boxes)
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
pub fn extract_lines(doc: &lopdf::Document, page_number: u32) -> Result<Vec<Line>, TrexError> {
    let pages = doc.get_pages();
    let page_id = pages
        .get(&page_number)
        .copied()
        .ok_or_else(|| TrexError::PdfParse(format!("페이지 {}를 찾을 수 없습니다", page_number)))?;

    let operations = decode_page_operations(doc, page_id);

    let mut segments = Vec::new();
    let mut current_point: Option<(f64, f64)> = None;
    let mut subpath_start: Option<(f64, f64)> = None;

    for op in &operations {
        match op.operator.as_str() {
            "m" => {
                if let Some((x, y)) = point_from_operands(&op.operands) {
                    current_point = Some((x, y));
                    subpath_start = Some((x, y));
                }
            }
            "l" => {
                if let (Some((x1, y1)), Some((x0, y0))) =
                    (point_from_operands(&op.operands), current_point)
                {
                    segments.push((x0, y0, x1, y1));
                    current_point = Some((x1, y1));
                }
            }
            "h" => {
                if let (Some((sx, sy)), Some((cx, cy))) = (subpath_start, current_point) {
                    segments.push((cx, cy, sx, sy));
                    current_point = Some((sx, sy));
                }
            }
            "re" => {
                if op.operands.len() >= 4 {
                    let x = op.operands.first().and_then(object_as_f64).unwrap_or(0.0);
                    let y = op.operands.get(1).and_then(object_as_f64).unwrap_or(0.0);
                    let w = op.operands.get(2).and_then(object_as_f64).unwrap_or(0.0);
                    let h = op.operands.get(3).and_then(object_as_f64).unwrap_or(0.0);

                    let x2 = x + w;
                    let y2 = y + h;

                    segments.push((x, y, x2, y));
                    segments.push((x2, y, x2, y2));
                    segments.push((x2, y2, x, y2));
                    segments.push((x, y2, x, y));

                    current_point = Some((x, y));
                    subpath_start = Some((x, y));
                }
            }
            _ => {}
        }
    }

    Ok(normalize_segments(segments))
}

fn decode_page_operations(doc: &Document, page_id: ObjectId) -> Vec<Operation> {
    let page_resources = page_resources(doc, page_id);
    let mut flattened = Vec::new();

    if let Ok(content_data) = doc.get_page_content(page_id) {
        if let Ok(content) = Content::decode(&content_data) {
            let mut visiting = HashSet::new();
            flatten_operations(
                doc,
                &content.operations,
                page_resources.as_ref(),
                Matrix::identity(),
                &mut visiting,
                0,
                &mut flattened,
            );
            if !flattened.is_empty() {
                return flattened;
            }
        }
    }

    for stream_id in doc.get_page_contents(page_id) {
        if let Some(ops) = decode_stream_operations(doc, stream_id) {
            let mut visiting = HashSet::new();
            flatten_operations(
                doc,
                &ops,
                page_resources.as_ref(),
                Matrix::identity(),
                &mut visiting,
                0,
                &mut flattened,
            );
        }
    }

    flattened
}

fn decode_stream_operations(doc: &Document, stream_id: ObjectId) -> Option<Vec<Operation>> {
    let stream = doc.get_object(stream_id).ok()?.as_stream().ok()?;
    let bytes = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    Content::decode(&bytes)
        .ok()
        .map(|content| content.operations)
}

#[derive(Debug, Clone, Copy)]
struct Matrix {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Matrix {
    fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    fn concat(self, rhs: Matrix) -> Matrix {
        Matrix {
            a: self.a * rhs.a + self.c * rhs.b,
            b: self.b * rhs.a + self.d * rhs.b,
            c: self.a * rhs.c + self.c * rhs.d,
            d: self.b * rhs.c + self.d * rhs.d,
            e: self.a * rhs.e + self.c * rhs.f + self.e,
            f: self.b * rhs.e + self.d * rhs.f + self.f,
        }
    }

    fn transform_point(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    fn transform_vector(self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y, self.b * x + self.d * y)
    }
}

#[derive(Debug)]
struct FormXObject {
    id: ObjectId,
    operations: Vec<Operation>,
    resources: Option<Dictionary>,
    matrix: Matrix,
}

fn page_resources(doc: &Document, page_id: ObjectId) -> Option<Dictionary> {
    doc.get_page_resources(page_id)
        .ok()
        .and_then(|(resources, _)| resources.cloned())
}

fn flatten_operations(
    doc: &Document,
    operations: &[Operation],
    resources: Option<&Dictionary>,
    base_matrix: Matrix,
    visiting_forms: &mut HashSet<ObjectId>,
    depth: usize,
    out: &mut Vec<Operation>,
) {
    if depth > 8 {
        return;
    }

    let mut ctm = base_matrix;
    let mut stack = Vec::new();

    for operation in operations {
        match operation.operator.as_str() {
            "q" => {
                stack.push(ctm);
            }
            "Q" => {
                if let Some(previous) = stack.pop() {
                    ctm = previous;
                }
            }
            "cm" => {
                if let Some(matrix) = matrix_from_operands(&operation.operands) {
                    ctm = ctm.concat(matrix);
                }
            }
            "Do" => {
                let Some(name) = operation
                    .operands
                    .first()
                    .and_then(|obj| obj.as_name().ok())
                else {
                    continue;
                };

                let Some(form) = resolve_form_xobject(doc, resources, name) else {
                    continue;
                };

                if !visiting_forms.insert(form.id) {
                    continue;
                }

                let next_resources = form.resources.as_ref().or(resources);
                let next_matrix = ctm.concat(form.matrix);

                flatten_operations(
                    doc,
                    &form.operations,
                    next_resources,
                    next_matrix,
                    visiting_forms,
                    depth + 1,
                    out,
                );

                visiting_forms.remove(&form.id);
            }
            _ => out.push(transform_operation(operation, ctm)),
        }
    }
}

fn resolve_form_xobject(
    doc: &Document,
    resources: Option<&Dictionary>,
    name: &[u8],
) -> Option<FormXObject> {
    let resources = resources?;
    let xobject_object = resources.get(b"XObject").ok()?;
    let xobject_dict = resolve_dictionary(doc, xobject_object)?;
    let xobject_entry = xobject_dict.get(name).ok()?;

    let stream_id = match xobject_entry {
        Object::Reference(id) => *id,
        _ => return None,
    };

    let stream = doc.get_object(stream_id).ok()?.as_stream().ok()?;
    let subtype = stream.dict.get(b"Subtype").ok()?.as_name().ok()?;
    if subtype != b"Form" {
        return None;
    }

    let bytes = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    let operations = Content::decode(&bytes).ok()?.operations;

    let form_resources = stream
        .dict
        .get(b"Resources")
        .ok()
        .and_then(|obj| resolve_dictionary(doc, obj));

    let form_matrix = stream
        .dict
        .get(b"Matrix")
        .ok()
        .and_then(matrix_from_object)
        .unwrap_or_else(Matrix::identity);

    Some(FormXObject {
        id: stream_id,
        operations,
        resources: form_resources,
        matrix: form_matrix,
    })
}

fn resolve_dictionary(doc: &Document, object: &Object) -> Option<Dictionary> {
    match object {
        Object::Dictionary(dict) => Some(dict.clone()),
        Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .and_then(|obj| obj.as_dict().ok().cloned()),
        _ => None,
    }
}

fn matrix_from_operands(operands: &[Object]) -> Option<Matrix> {
    if operands.len() < 6 {
        return None;
    }

    Some(Matrix {
        a: operands.first().and_then(object_as_f64)?,
        b: operands.get(1).and_then(object_as_f64)?,
        c: operands.get(2).and_then(object_as_f64)?,
        d: operands.get(3).and_then(object_as_f64)?,
        e: operands.get(4).and_then(object_as_f64)?,
        f: operands.get(5).and_then(object_as_f64)?,
    })
}

fn matrix_from_object(object: &Object) -> Option<Matrix> {
    let array = object.as_array().ok()?;
    matrix_from_operands(array)
}

fn transform_operation(operation: &Operation, matrix: Matrix) -> Operation {
    let mut transformed = operation.clone();

    match transformed.operator.as_str() {
        "m" | "l" => {
            if transformed.operands.len() >= 2 {
                if let (Some(x), Some(y)) = (
                    transformed.operands.first().and_then(object_as_f64),
                    transformed.operands.get(1).and_then(object_as_f64),
                ) {
                    let (nx, ny) = matrix.transform_point(x, y);
                    transformed.operands[0] = Object::Real(nx as f32);
                    transformed.operands[1] = Object::Real(ny as f32);
                }
            }
        }
        "re" => {
            if transformed.operands.len() >= 4 {
                if let (Some(x), Some(y), Some(w), Some(h)) = (
                    transformed.operands.first().and_then(object_as_f64),
                    transformed.operands.get(1).and_then(object_as_f64),
                    transformed.operands.get(2).and_then(object_as_f64),
                    transformed.operands.get(3).and_then(object_as_f64),
                ) {
                    let (nx, ny) = matrix.transform_point(x, y);
                    let (vw_x, _) = matrix.transform_vector(w, 0.0);
                    let (_, vh_y) = matrix.transform_vector(0.0, h);
                    transformed.operands[0] = Object::Real(nx as f32);
                    transformed.operands[1] = Object::Real(ny as f32);
                    transformed.operands[2] = Object::Real(vw_x as f32);
                    transformed.operands[3] = Object::Real(vh_y as f32);
                }
            }
        }
        "Tm" => {
            if transformed.operands.len() >= 6 {
                if let (Some(x), Some(y)) = (
                    transformed.operands.get(4).and_then(object_as_f64),
                    transformed.operands.get(5).and_then(object_as_f64),
                ) {
                    let (nx, ny) = matrix.transform_point(x, y);
                    transformed.operands[4] = Object::Real(nx as f32);
                    transformed.operands[5] = Object::Real(ny as f32);
                }
            }
        }
        "Td" | "TD" => {
            if transformed.operands.len() >= 2 {
                if let (Some(x), Some(y)) = (
                    transformed.operands.first().and_then(object_as_f64),
                    transformed.operands.get(1).and_then(object_as_f64),
                ) {
                    let (nx, ny) = matrix.transform_vector(x, y);
                    transformed.operands[0] = Object::Real(nx as f32);
                    transformed.operands[1] = Object::Real(ny as f32);
                }
            }
        }
        _ => {}
    }

    transformed
}

fn object_as_f64(object: &Object) -> Option<f64> {
    match object {
        Object::Integer(value) => Some(*value as f64),
        Object::Real(value) => Some(*value as f64),
        _ => None,
    }
}

fn point_from_operands(operands: &[Object]) -> Option<(f64, f64)> {
    let x = operands.first().and_then(object_as_f64)?;
    let y = operands.get(1).and_then(object_as_f64)?;
    Some((x, y))
}

fn decode_text_operand(
    doc: &Document,
    fonts: &BTreeMap<Vec<u8>, &Dictionary>,
    current_font: Option<&[u8]>,
    operand: &Object,
) -> String {
    match operand {
        Object::String(bytes, _) => decode_text_bytes(doc, fonts, current_font, bytes),
        Object::Array(items) => {
            let mut text = String::new();

            for item in items {
                match item {
                    Object::String(bytes, _) => {
                        text.push_str(&decode_text_bytes(doc, fonts, current_font, bytes));
                    }
                    Object::Integer(value) if *value < -120 => {
                        if !text.ends_with(' ') {
                            text.push(' ');
                        }
                    }
                    Object::Real(value) if *value < -120.0 => {
                        if !text.ends_with(' ') {
                            text.push(' ');
                        }
                    }
                    _ => {}
                }
            }

            text
        }
        _ => String::new(),
    }
}

fn decode_text_bytes(
    doc: &Document,
    fonts: &BTreeMap<Vec<u8>, &Dictionary>,
    current_font: Option<&[u8]>,
    bytes: &[u8],
) -> String {
    if let Some(font_name) = current_font {
        if let Some(font_dict) = fonts.get(font_name) {
            if let Ok(encoding) = font_dict.get_font_encoding(doc) {
                if let Ok(text) = Document::decode_text(&encoding, bytes) {
                    return text;
                }
            }
        }
    }

    String::from_utf8_lossy(bytes).to_string()
}

fn normalize_text(raw_text: &str) -> String {
    raw_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn estimate_text_width(text: &str, font_size: f64) -> f64 {
    let char_count = text.chars().count() as f64;
    (char_count * font_size * 0.55).max(font_size * 0.5)
}

fn push_text_box(
    text_boxes: &mut Vec<TextBox>,
    state: &mut TextState,
    text: String,
    page_number: u32,
) {
    let text = normalize_text(&text);
    if text.is_empty() {
        return;
    }

    let font_size = state.font_size.max(1.0);
    let width = estimate_text_width(&text, font_size);

    let x0 = state.current_x;
    let y0 = state.current_y;

    text_boxes.push(TextBox {
        text,
        bbox: BBox {
            x0,
            y0,
            x1: x0 + width,
            y1: y0 + font_size,
        },
        page: page_number,
    });

    state.current_x += width;
}

#[derive(Debug, Clone, Copy)]
struct AxisSpan {
    coord: f64,
    start: f64,
    end: f64,
}

fn merge_spans(
    mut spans: Vec<AxisSpan>,
    coord_tolerance: f64,
    gap_tolerance: f64,
) -> Vec<AxisSpan> {
    if spans.is_empty() {
        return spans;
    }

    spans.sort_by(|a, b| {
        a.coord
            .total_cmp(&b.coord)
            .then_with(|| a.start.total_cmp(&b.start))
    });

    let mut merged: Vec<AxisSpan> = Vec::new();

    for span in spans {
        if let Some(last) = merged.last_mut() {
            let same_coord = (last.coord - span.coord).abs() <= coord_tolerance;
            let connected = span.start <= last.end + gap_tolerance;

            if same_coord && connected {
                last.coord = (last.coord + span.coord) / 2.0;
                last.start = last.start.min(span.start);
                last.end = last.end.max(span.end);
                continue;
            }
        }

        merged.push(span);
    }

    merged
}

fn normalize_segments(segments: Vec<(f64, f64, f64, f64)>) -> Vec<Line> {
    let mut horizontal_spans = Vec::new();
    let mut vertical_spans = Vec::new();

    for (x0, y0, x1, y1) in segments {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();

        if dy <= POSITION_TOLERANCE && dx >= MIN_LINE_LENGTH {
            horizontal_spans.push(AxisSpan {
                coord: (y0 + y1) / 2.0,
                start: x0.min(x1),
                end: x0.max(x1),
            });
        } else if dx <= POSITION_TOLERANCE && dy >= MIN_LINE_LENGTH {
            vertical_spans.push(AxisSpan {
                coord: (x0 + x1) / 2.0,
                start: y0.min(y1),
                end: y0.max(y1),
            });
        }
    }

    let horizontal_spans = merge_spans(horizontal_spans, POSITION_TOLERANCE, 2.0);
    let vertical_spans = merge_spans(vertical_spans, POSITION_TOLERANCE, 2.0);

    let mut lines = Vec::with_capacity(horizontal_spans.len() + vertical_spans.len());

    for span in horizontal_spans {
        lines.push(Line {
            x0: span.start,
            y0: span.coord,
            x1: span.end,
            y1: span.coord,
            direction: LineDirection::Horizontal,
        });
    }

    for span in vertical_spans {
        lines.push(Line {
            x0: span.coord,
            y0: span.start,
            x1: span.coord,
            y1: span.end,
            direction: LineDirection::Vertical,
        });
    }

    lines
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
