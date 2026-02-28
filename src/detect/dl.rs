//! DL 라우터 기반 테이블 탐지
//!
//! 이 모듈은 페이지의 기하학적 피처를 기반으로
//! Lattice / Stream / Blend 전략 중 하나를 선택한다.
//! - `dl` feature + ONNX 모델 경로가 있으면 모델 추론 사용
//! - 그 외에는 내장 휴리스틱 라우터 사용

use crate::pdf::text::{Line, LineDirection};
use crate::{error::TrexError, DlFallbackMode, RuntimeOptions, Table, TextBox};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy)]
enum DlStrategy {
    Lattice,
    Stream,
    Blend,
}

#[derive(Debug, Clone, Copy)]
enum DecisionSource {
    #[cfg(feature = "dl")]
    Onnx,
    Heuristic,
}

#[derive(Debug, Clone, Copy)]
struct RouterDecision {
    strategy: DlStrategy,
    confidence: f32,
    source: DecisionSource,
}

#[derive(Debug, Clone)]
struct PageFeatures {
    text_count: f32,
    line_count: f32,
    horizontal_count: f32,
    vertical_count: f32,
    intersection_count: f32,
    mean_text_len: f32,
    median_text_width: f32,
    median_text_height: f32,
    text_span_x: f32,
    text_span_y: f32,
}

impl PageFeatures {
    fn from_page(text_boxes: &[TextBox], lines: &[Line]) -> Self {
        let text_count = text_boxes.len() as f32;
        let line_count = lines.len() as f32;

        let horizontal_count = lines
            .iter()
            .filter(|line| line.direction == LineDirection::Horizontal)
            .count() as f32;
        let vertical_count = lines
            .iter()
            .filter(|line| line.direction == LineDirection::Vertical)
            .count() as f32;

        let intersection_count = count_intersections(lines) as f32;

        let text_lengths: Vec<f32> = text_boxes
            .iter()
            .map(|tb| tb.text.trim().chars().count() as f32)
            .filter(|len| *len > 0.0)
            .collect();
        let mean_text_len = if text_lengths.is_empty() {
            0.0
        } else {
            text_lengths.iter().sum::<f32>() / text_lengths.len() as f32
        };

        let widths: Vec<f32> = text_boxes
            .iter()
            .map(|tb| (tb.bbox.x1 - tb.bbox.x0).abs() as f32)
            .filter(|w| *w > 0.0)
            .collect();
        let heights: Vec<f32> = text_boxes
            .iter()
            .map(|tb| (tb.bbox.y1 - tb.bbox.y0).abs() as f32)
            .filter(|h| *h > 0.0)
            .collect();

        let median_text_width = median(widths);
        let median_text_height = median(heights);

        let text_span_x = span(text_boxes.iter().map(|tb| tb.bbox.x0 as f32));
        let text_span_y = span(text_boxes.iter().map(|tb| tb.bbox.y0 as f32));

        Self {
            text_count,
            line_count,
            horizontal_count,
            vertical_count,
            intersection_count,
            mean_text_len,
            median_text_width,
            median_text_height,
            text_span_x,
            text_span_y,
        }
    }

    #[cfg(feature = "dl")]
    fn input_vector(&self) -> Vec<f32> {
        vec![
            self.text_count,
            self.line_count,
            self.horizontal_count,
            self.vertical_count,
            self.intersection_count,
            self.mean_text_len,
            self.median_text_width,
            self.median_text_height,
            self.text_span_x,
            self.text_span_y,
        ]
    }
}

fn span(values: impl Iterator<Item = f32>) -> f32 {
    let mut min_value = f32::INFINITY;
    let mut max_value = f32::NEG_INFINITY;

    for value in values {
        min_value = min_value.min(value);
        max_value = max_value.max(value);
    }

    if min_value.is_finite() && max_value.is_finite() {
        (max_value - min_value).max(0.0)
    } else {
        0.0
    }
}

fn median(mut values: Vec<f32>) -> f32 {
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

fn count_intersections(lines: &[Line]) -> usize {
    let horizontal: Vec<&Line> = lines
        .iter()
        .filter(|line| line.direction == LineDirection::Horizontal)
        .collect();
    let vertical: Vec<&Line> = lines
        .iter()
        .filter(|line| line.direction == LineDirection::Vertical)
        .collect();

    let mut count = 0usize;

    for h in &horizontal {
        let hx0 = h.x0.min(h.x1);
        let hx1 = h.x0.max(h.x1);
        let hy = (h.y0 + h.y1) / 2.0;

        for v in &vertical {
            let vx = (v.x0 + v.x1) / 2.0;
            let vy0 = v.y0.min(v.y1);
            let vy1 = v.y0.max(v.y1);

            if vx >= hx0 - 1.0 && vx <= hx1 + 1.0 && hy >= vy0 - 1.0 && hy <= vy1 + 1.0 {
                count += 1;
            }
        }
    }

    count
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn heuristic_logits(features: &PageFeatures) -> [f32; 3] {
    let line_strength = sigmoid((features.line_count - 24.0) / 16.0);
    let intersection_strength = sigmoid((features.intersection_count - 36.0) / 24.0);
    let text_strength = sigmoid((features.text_count - 28.0) / 20.0);
    let wide_layout = sigmoid((features.text_span_x - 220.0) / 120.0);
    let tall_layout = sigmoid((features.text_span_y - 140.0) / 90.0);
    let char_density = sigmoid((features.mean_text_len - 6.0) / 4.0);
    let cell_like_width = sigmoid((features.median_text_width - 24.0) / 16.0);
    let cell_like_height = sigmoid((features.median_text_height - 8.0) / 4.0);
    let axis_balance = {
        let sum = features.horizontal_count + features.vertical_count + 1.0;
        1.0 - ((features.horizontal_count - features.vertical_count).abs() / sum)
    };

    let lattice = 1.6 * line_strength
        + 1.1 * intersection_strength
        + 0.35 * axis_balance
        + 0.2 * (cell_like_width * cell_like_height)
        + 0.25 * wide_layout;

    let stream = 1.7 * text_strength
        + 0.9 * (1.0 - line_strength)
        + 0.35 * char_density
        + 0.2 * tall_layout
        + 0.2 * wide_layout;

    let blend = 1.2 * line_strength.min(text_strength)
        + 0.8 * intersection_strength
        + 0.4 * text_strength
        + 0.25 * axis_balance;

    [lattice, stream, blend]
}

fn softmax(logits: [f32; 3]) -> [f32; 3] {
    let max_logit = logits
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, |left, right| left.max(right));

    let exps = [
        (logits[0] - max_logit).exp(),
        (logits[1] - max_logit).exp(),
        (logits[2] - max_logit).exp(),
    ];
    let sum = exps[0] + exps[1] + exps[2];

    if sum <= 0.0 {
        [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]
    } else {
        [exps[0] / sum, exps[1] / sum, exps[2] / sum]
    }
}

fn decision_from_logits(logits: [f32; 3], source: DecisionSource) -> RouterDecision {
    let probs = softmax(logits);
    let (idx, confidence) = probs
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(idx, prob)| (idx, *prob))
        .unwrap_or((0, 0.0));

    let strategy = match idx {
        0 => DlStrategy::Lattice,
        1 => DlStrategy::Stream,
        _ => DlStrategy::Blend,
    };

    RouterDecision {
        strategy,
        confidence,
        source,
    }
}

fn infer_decision(
    features: &PageFeatures,
    runtime: &RuntimeOptions,
) -> Result<RouterDecision, TrexError> {
    #[cfg(feature = "dl")]
    {
        if let Some(model_path) = runtime.dl.model_path.as_deref() {
            let logits = infer_with_onnx(model_path, features)?;
            return Ok(decision_from_logits(logits, DecisionSource::Onnx));
        }
    }

    #[cfg(not(feature = "dl"))]
    {
        if runtime.dl.model_path.is_some() {
            return Err(TrexError::Dl(
                "DL 모델을 사용하려면 `--features dl`로 빌드해야 합니다".to_string(),
            ));
        }
    }

    let logits = heuristic_logits(features);
    Ok(decision_from_logits(logits, DecisionSource::Heuristic))
}

#[cfg(feature = "dl")]
fn infer_with_onnx(
    model_path: &std::path::Path,
    features: &PageFeatures,
) -> Result<[f32; 3], TrexError> {
    use tract_onnx::prelude::*;

    let input_vector = features.input_vector();
    let input = tract_ndarray::Array2::<f32>::from_shape_vec((1, input_vector.len()), input_vector)
        .map_err(|e| TrexError::Dl(format!("DL 입력 텐서 생성 실패: {}", e)))?;

    let model = tract_onnx::onnx()
        .model_for_path(model_path)
        .map_err(|e| TrexError::Dl(format!("ONNX 모델 로딩 실패: {}", e)))?
        .with_input_fact(
            0,
            InferenceFact::dt_shape(f32::datum_type(), tvec!(1, input.ncols() as i64)),
        )
        .map_err(|e| TrexError::Dl(format!("ONNX 입력 shape 설정 실패: {}", e)))?
        .into_optimized()
        .map_err(|e| TrexError::Dl(format!("ONNX 최적화 실패: {}", e)))?
        .into_runnable()
        .map_err(|e| TrexError::Dl(format!("ONNX 런너 생성 실패: {}", e)))?;

    let outputs = model
        .run(tvec!(input.into_tensor().into()))
        .map_err(|e| TrexError::Dl(format!("ONNX 추론 실패: {}", e)))?;

    let first = outputs
        .first()
        .ok_or_else(|| TrexError::Dl("ONNX 출력 텐서가 비어 있습니다".to_string()))?;
    let view = first
        .to_array_view::<f32>()
        .map_err(|e| TrexError::Dl(format!("ONNX 출력 해석 실패: {}", e)))?;

    let values: Vec<f32> = view.iter().copied().collect();
    if values.len() < 3 {
        return Err(TrexError::Dl(format!(
            "ONNX 출력 차원이 부족합니다. expected>=3, actual={}",
            values.len()
        )));
    }

    Ok([values[0], values[1], values[2]])
}

fn fallback_detect(
    text_boxes: &[TextBox],
    lines: &[Line],
    page: u32,
    fallback: DlFallbackMode,
) -> Result<Vec<Table>, TrexError> {
    match fallback {
        DlFallbackMode::Auto => super::detect_auto(text_boxes, lines, page),
        DlFallbackMode::Lattice => super::lattice::detect(text_boxes, lines, page),
        DlFallbackMode::Stream => super::stream::detect(text_boxes, page),
    }
}

fn table_non_empty_cells(table: &Table) -> usize {
    table
        .headers
        .iter()
        .chain(table.rows.iter().flat_map(|row| row.iter()))
        .filter(|cell| !cell.trim().is_empty())
        .count()
}

fn table_tokens(table: &Table) -> HashSet<String> {
    table
        .headers
        .iter()
        .chain(table.rows.iter().flat_map(|row| row.iter()))
        .map(|cell| cell.trim().to_lowercase())
        .filter(|cell| !cell.is_empty())
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count() as f32;
    let union = a.union(b).count() as f32;
    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn similar_table(left: &Table, right: &Table) -> bool {
    let left_rows = left.rows.len() + 1;
    let right_rows = right.rows.len() + 1;
    let left_cols = left.headers.len();
    let right_cols = right.headers.len();

    let shape_close = left_rows.abs_diff(right_rows) <= 1 && left_cols.abs_diff(right_cols) <= 1;

    let left_tokens = table_tokens(left);
    let right_tokens = table_tokens(right);
    let token_similarity = jaccard(&left_tokens, &right_tokens);

    token_similarity >= 0.85 || (shape_close && token_similarity >= 0.55)
}

fn merge_tables(mut primary: Vec<Table>, secondary: Vec<Table>) -> Vec<Table> {
    for candidate in secondary {
        let is_dup = primary
            .iter()
            .any(|existing| similar_table(existing, &candidate));
        if !is_dup {
            primary.push(candidate);
        }
    }
    primary
}

fn blend_detect(
    text_boxes: &[TextBox],
    lines: &[Line],
    page: u32,
) -> Result<Vec<Table>, TrexError> {
    let lattice = super::lattice::detect(text_boxes, lines, page)?;
    let stream = super::stream::detect(text_boxes, page)?;

    if lattice.is_empty() {
        return Ok(stream);
    }
    if stream.is_empty() {
        return Ok(lattice);
    }

    let lattice_score: usize = lattice.iter().map(table_non_empty_cells).sum();
    let stream_score: usize = stream.iter().map(table_non_empty_cells).sum();

    if lattice_score >= stream_score {
        Ok(merge_tables(lattice, stream))
    } else {
        Ok(merge_tables(stream, lattice))
    }
}

fn run_strategy(
    strategy: DlStrategy,
    text_boxes: &[TextBox],
    lines: &[Line],
    page: u32,
) -> Result<Vec<Table>, TrexError> {
    match strategy {
        DlStrategy::Lattice => super::lattice::detect(text_boxes, lines, page),
        DlStrategy::Stream => super::stream::detect(text_boxes, page),
        DlStrategy::Blend => blend_detect(text_boxes, lines, page),
    }
}

/// DL 라우터 기반으로 테이블을 탐지한다.
pub fn detect(
    text_boxes: &[TextBox],
    lines: &[Line],
    page: u32,
    runtime: &RuntimeOptions,
) -> Result<Vec<Table>, TrexError> {
    let features = PageFeatures::from_page(text_boxes, lines);
    let decision = infer_decision(&features, runtime)?;
    let min_confidence = runtime.dl.min_confidence.clamp(0.0, 1.0);

    let _source = decision.source;

    let mut tables = if decision.confidence >= min_confidence {
        run_strategy(decision.strategy, text_boxes, lines, page)?
    } else {
        fallback_detect(text_boxes, lines, page, runtime.dl.fallback_mode)?
    };

    if tables.is_empty() {
        tables = fallback_detect(text_boxes, lines, page, runtime.dl.fallback_mode)?;
    }

    Ok(tables)
}
