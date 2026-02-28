use std::path::PathBuf;

use trex::{ExtractOptions, ParseMode, RuntimeOptions, TrexError};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("pdfs")
        .join(name)
}

#[test]
fn twotables_pdf_should_extract_two_tables_in_auto_mode() {
    let path = fixture_path("twotables.pdf");
    let options = ExtractOptions {
        pages: None,
        mode: ParseMode::Auto,
    };

    let tables = trex::extract(path, &options).expect("twotables.pdf extraction should succeed");
    assert_eq!(tables.len(), 2, "twotables.pdf should yield two tables");
}

#[test]
fn password_protected_pdf_should_return_not_supported_error() {
    let path = fixture_path("health_protected.pdf");
    let options = ExtractOptions::default();

    let error = trex::extract(path, &options).expect_err("encrypted PDF should fail");

    match error {
        TrexError::PdfParse(message) => {
            assert!(
                message.contains("암호화된 PDF는 현재 지원하지 않습니다"),
                "unexpected encrypted PDF error: {}",
                message
            );
        }
        other => panic!("unexpected error variant: {}", other),
    }
}

#[test]
fn dl_mode_without_model_should_still_extract_tables() {
    let path = fixture_path("twotables.pdf");
    let options = ExtractOptions {
        pages: None,
        mode: ParseMode::Dl,
    };

    let tables = trex::extract_with_runtime_options(path, &options, &RuntimeOptions::default())
        .expect("dl mode without model should fallback and succeed");

    assert!(!tables.is_empty(), "dl mode fallback should return tables");
}

#[cfg(not(feature = "dl"))]
#[test]
fn dl_model_path_requires_dl_feature() {
    let path = fixture_path("twotables.pdf");
    let options = ExtractOptions {
        pages: None,
        mode: ParseMode::Dl,
    };

    let mut runtime = RuntimeOptions::default();
    runtime.dl.model_path = Some(fixture_path("twotables.pdf"));

    let error = trex::extract_with_runtime_options(path, &options, &runtime)
        .expect_err("model path should error without dl feature");

    match error {
        TrexError::Dl(message) => {
            assert!(
                message.contains("--features dl"),
                "unexpected dl feature error message: {}",
                message
            );
        }
        other => panic!("unexpected error variant: {}", other),
    }
}
