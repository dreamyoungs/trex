use napi::bindgen_prelude::{Buffer, Error, Result};
use napi_derive::napi;
use std::io::Write;
use std::path::{Path, PathBuf};

#[napi(string_enum)]
pub enum ParseModeOption {
    Auto,
    Lattice,
    Stream,
    Dl,
}

#[napi(string_enum)]
pub enum DlFallbackOption {
    Auto,
    Lattice,
    Stream,
}

#[napi(object)]
pub struct ExtractOptions {
    pub pages: Option<Vec<u32>>,
    pub mode: Option<ParseModeOption>,
    pub dl_model: Option<String>,
    pub dl_min_confidence: Option<f64>,
    pub dl_fallback: Option<DlFallbackOption>,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            pages: None,
            mode: Some(ParseModeOption::Auto),
            dl_model: None,
            dl_min_confidence: None,
            dl_fallback: Some(DlFallbackOption::Auto),
        }
    }
}

#[napi(object)]
pub struct Table {
    pub page: u32,
    pub table_index: u32,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[napi]
pub fn extract(pdf_path: String, options: Option<ExtractOptions>) -> Result<Vec<Table>> {
    let tables = run_extract(Path::new(&pdf_path), options.unwrap_or_default())?;
    Ok(convert_tables(tables))
}

#[napi(js_name = "extractCsv")]
pub fn extract_csv(pdf_path: String, options: Option<ExtractOptions>) -> Result<String> {
    let tables = run_extract(Path::new(&pdf_path), options.unwrap_or_default())?;
    trex::output::to_csv(&tables).map_err(to_napi_error)
}

#[napi(js_name = "extractFromBuffer")]
pub fn extract_from_buffer(
    pdf_buffer: Buffer,
    options: Option<ExtractOptions>,
) -> Result<Vec<Table>> {
    let mut temp_file = tempfile::NamedTempFile::new().map_err(to_napi_error)?;
    temp_file.write_all(&pdf_buffer).map_err(to_napi_error)?;
    temp_file.flush().map_err(to_napi_error)?;

    let tables = run_extract(temp_file.path(), options.unwrap_or_default())?;
    Ok(convert_tables(tables))
}

#[napi(js_name = "extractCsvFromBuffer")]
pub fn extract_csv_from_buffer(
    pdf_buffer: Buffer,
    options: Option<ExtractOptions>,
) -> Result<String> {
    let mut temp_file = tempfile::NamedTempFile::new().map_err(to_napi_error)?;
    temp_file.write_all(&pdf_buffer).map_err(to_napi_error)?;
    temp_file.flush().map_err(to_napi_error)?;

    let tables = run_extract(temp_file.path(), options.unwrap_or_default())?;
    trex::output::to_csv(&tables).map_err(to_napi_error)
}

fn run_extract(path: &Path, options: ExtractOptions) -> Result<Vec<trex::Table>> {
    let extract_options = trex::ExtractOptions {
        pages: options.pages,
        mode: map_parse_mode(options.mode.unwrap_or(ParseModeOption::Auto)),
    };

    let runtime = trex::RuntimeOptions {
        dl: trex::DlRuntimeOptions {
            model_path: options.dl_model.map(PathBuf::from),
            min_confidence: options.dl_min_confidence.unwrap_or(0.55) as f32,
            fallback_mode: map_dl_fallback(options.dl_fallback.unwrap_or(DlFallbackOption::Auto)),
        },
    };

    trex::extract_with_runtime_options(path, &extract_options, &runtime).map_err(to_napi_error)
}

fn map_parse_mode(mode: ParseModeOption) -> trex::ParseMode {
    match mode {
        ParseModeOption::Auto => trex::ParseMode::Auto,
        ParseModeOption::Lattice => trex::ParseMode::Lattice,
        ParseModeOption::Stream => trex::ParseMode::Stream,
        ParseModeOption::Dl => trex::ParseMode::Dl,
    }
}

fn map_dl_fallback(mode: DlFallbackOption) -> trex::DlFallbackMode {
    match mode {
        DlFallbackOption::Auto => trex::DlFallbackMode::Auto,
        DlFallbackOption::Lattice => trex::DlFallbackMode::Lattice,
        DlFallbackOption::Stream => trex::DlFallbackMode::Stream,
    }
}

fn convert_tables(tables: Vec<trex::Table>) -> Vec<Table> {
    tables
        .into_iter()
        .map(|table| Table {
            page: table.page,
            table_index: u32::try_from(table.table_index).unwrap_or(u32::MAX),
            headers: table.headers,
            rows: table.rows,
        })
        .collect()
}

fn to_napi_error(error: impl std::fmt::Display) -> Error {
    Error::from_reason(error.to_string())
}
