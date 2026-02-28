//! TREX 벤치마크
//!
//! 사용법: cargo bench

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use trex::{BBox, ExtractOptions, ParseMode, TextBox};

fn synthetic_text_boxes(rows: usize, cols: usize) -> Vec<TextBox> {
    let mut boxes = Vec::with_capacity(rows * cols);

    let base_x = 50.0;
    let base_y = 760.0;
    let col_width = 90.0;
    let row_height = 22.0;

    for row in 0..rows {
        for col in 0..cols {
            let x0 = base_x + col as f64 * col_width;
            let y1 = base_y - row as f64 * row_height;
            let y0 = y1 - 12.0;

            let text = if row == 0 {
                format!("H{}", col + 1)
            } else {
                format!("R{}C{}", row, col + 1)
            };

            boxes.push(TextBox {
                text,
                bbox: BBox {
                    x0,
                    y0,
                    x1: x0 + col_width * 0.7,
                    y1,
                },
                page: 1,
            });
        }
    }

    boxes
}

fn bench_stream_detection(c: &mut Criterion) {
    let text_boxes = synthetic_text_boxes(120, 8);

    c.bench_function("stream_detect_synthetic_120x8", |b| {
        b.iter(|| {
            let tables = trex::detect::stream::detect(black_box(&text_boxes), 1)
                .expect("stream detection should not fail on synthetic input");
            black_box(tables);
        });
    });
}

fn bench_extract_pdf_if_configured(c: &mut Criterion) {
    let Ok(pdf_path) = std::env::var("TREX_BENCH_PDF") else {
        return;
    };

    let options = ExtractOptions {
        pages: None,
        mode: ParseMode::Auto,
    };

    c.bench_function("extract_pdf_auto", |b| {
        b.iter(|| {
            let tables = trex::extract(black_box(&pdf_path), black_box(&options))
                .expect("PDF extraction should succeed for TREX_BENCH_PDF");
            black_box(tables);
        });
    });
}

criterion_group!(
    benches,
    bench_stream_detection,
    bench_extract_pdf_if_configured
);
criterion_main!(benches);
