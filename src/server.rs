use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use trex::{DlFallbackMode, ExtractOptions, ParseMode, RuntimeOptions, TrexError, i18n};

#[derive(Clone)]
pub struct ServeConfig {
    pub host: String,
    pub port: u16,
    pub runtime: RuntimeOptions,
}

#[derive(Clone)]
struct AppState {
    runtime: RuntimeOptions,
}

#[derive(Debug, Clone, Copy)]
enum ResponseFormat {
    Json,
    Csv,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn run(config: ServeConfig) -> Result<(), String> {
    let state = AppState {
        runtime: config.runtime,
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/extract", post(extract))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e| {
            format!(
                "{}: {}",
                i18n::text("잘못된 서버 주소", "Invalid server address"),
                e
            )
        })?;

    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        format!(
            "{}: {}",
            i18n::text("서버 바인딩 실패", "Failed to bind server"),
            e
        )
    })?;

    eprintln!(
        "{}: http://{}",
        i18n::text("TREX REST API 서버 시작", "TREX REST API started"),
        addr
    );

    axum::serve(listener, app).await.map_err(|e| {
        format!(
            "{}: {}",
            i18n::text("서버 실행 실패", "Server runtime failed"),
            e
        )
    })
}

async fn root() -> impl IntoResponse {
    Json(HealthResponse { status: "trex" })
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse { status: "ok" })
}

async fn extract(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let request_language = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .and_then(i18n::language_from_accept_language)
        .unwrap_or_else(i18n::current_language);

    let mut fields = HashMap::<String, String>::new();
    let mut file_data: Option<Vec<u8>> = None;

    loop {
        let next_field = match multipart.next_field().await {
            Ok(next_field) => next_field,
            Err(error) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "{}: {}",
                        localized(
                            request_language,
                            "multipart 파싱 실패",
                            "Failed to parse multipart body"
                        ),
                        error
                    ),
                );
            }
        };

        let Some(field) = next_field else {
            break;
        };

        let Some(name) = field.name().map(str::to_string) else {
            continue;
        };

        if name == "file" {
            let bytes = match field.bytes().await {
                Ok(bytes) => bytes,
                Err(error) => {
                    return error_response(
                        StatusCode::BAD_REQUEST,
                        format!(
                            "{}: {}",
                            localized(
                                request_language,
                                "업로드 파일 읽기 실패",
                                "Failed to read uploaded file"
                            ),
                            error
                        ),
                    );
                }
            };
            file_data = Some(bytes.to_vec());
            continue;
        }

        let value = match field.text().await {
            Ok(value) => value,
            Err(error) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "{} ({}): {}",
                        localized(
                            request_language,
                            "요청 필드 파싱 실패",
                            "Failed to parse request field"
                        ),
                        name,
                        error
                    ),
                );
            }
        };
        fields.insert(name, value);
    }

    let Some(file_data) = file_data else {
        return error_response(
            StatusCode::BAD_REQUEST,
            localized(
                request_language,
                "file 필드가 필요합니다",
                "`file` field is required",
            )
            .to_string(),
        );
    };

    let mode = match fields.get("mode") {
        Some(value) => match parse_mode(value) {
            Some(mode) => mode,
            None => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "{}: {}",
                        localized(
                            request_language,
                            "지원하지 않는 mode 값",
                            "Unsupported mode value"
                        ),
                        value
                    ),
                );
            }
        },
        None => ParseMode::Auto,
    };

    let format = match fields.get("format") {
        Some(value) => match parse_format(value) {
            Some(format) => format,
            None => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "{}: {}",
                        localized(
                            request_language,
                            "지원하지 않는 format 값",
                            "Unsupported format value"
                        ),
                        value
                    ),
                );
            }
        },
        None => ResponseFormat::Json,
    };

    let pages = match fields.get("pages") {
        Some(value) => match parse_pages(value, request_language) {
            Ok(pages) => Some(pages),
            Err(error) => return error_response(StatusCode::BAD_REQUEST, error),
        },
        None => None,
    };

    let mut runtime = state.runtime.clone();

    if let Some(value) = fields.get("dl_min_confidence") {
        let parsed = match value.parse::<f32>() {
            Ok(parsed) => parsed,
            Err(_) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "{}: {}",
                        localized(
                            request_language,
                            "잘못된 dl_min_confidence 값",
                            "Invalid dl_min_confidence value"
                        ),
                        value
                    ),
                );
            }
        };
        runtime.dl.min_confidence = parsed;
    }

    if let Some(value) = fields.get("dl_fallback") {
        let Some(fallback) = parse_dl_fallback(value) else {
            return error_response(
                StatusCode::BAD_REQUEST,
                format!(
                    "{}: {}",
                    localized(
                        request_language,
                        "지원하지 않는 dl_fallback 값",
                        "Unsupported dl_fallback value"
                    ),
                    value
                ),
            );
        };
        runtime.dl.fallback_mode = fallback;
    }

    let options = ExtractOptions { pages, mode };

    let extraction_result =
        tokio::task::spawn_blocking(move || -> Result<Vec<trex::Table>, TrexError> {
            let mut temp_file = tempfile::NamedTempFile::new()?;
            temp_file.write_all(&file_data)?;
            temp_file.flush()?;

            i18n::with_language(request_language, || {
                trex::extract_with_runtime_options(temp_file.path(), &options, &runtime)
            })
        })
        .await;

    let tables = match extraction_result {
        Ok(Ok(tables)) => tables,
        Ok(Err(error)) => return trex_error_response(error, request_language),
        Err(error) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "{}: {}",
                    localized(
                        request_language,
                        "추출 작업 실패",
                        "Extraction worker failed"
                    ),
                    error
                ),
            );
        }
    };

    match format {
        ResponseFormat::Json => match trex::output::to_json(&tables) {
            Ok(body) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                body,
            )
                .into_response(),
            Err(error) => trex_error_response(error, request_language),
        },
        ResponseFormat::Csv => match trex::output::to_csv(&tables) {
            Ok(body) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                body,
            )
                .into_response(),
            Err(error) => trex_error_response(error, request_language),
        },
    }
}

fn parse_mode(value: &str) -> Option<ParseMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(ParseMode::Auto),
        "lattice" => Some(ParseMode::Lattice),
        "stream" => Some(ParseMode::Stream),
        "dl" => Some(ParseMode::Dl),
        _ => None,
    }
}

fn parse_format(value: &str) -> Option<ResponseFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "json" => Some(ResponseFormat::Json),
        "csv" => Some(ResponseFormat::Csv),
        _ => None,
    }
}

fn parse_dl_fallback(value: &str) -> Option<DlFallbackMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(DlFallbackMode::Auto),
        "lattice" => Some(DlFallbackMode::Lattice),
        "stream" => Some(DlFallbackMode::Stream),
        _ => None,
    }
}

fn parse_pages(value: &str, language: i18n::Language) -> Result<Vec<u32>, String> {
    let mut pages = Vec::new();

    for raw in value.split(',') {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }

        if token.contains('-') {
            let range: Vec<&str> = token.split('-').collect();
            if range.len() != 2 {
                return Err(format!(
                    "{}: {}",
                    localized(language, "잘못된 페이지 범위", "Invalid page range"),
                    token
                ));
            }

            let start = range[0].trim().parse::<u32>().map_err(|_| {
                format!(
                    "{}: {}",
                    localized(language, "잘못된 페이지 번호", "Invalid page number"),
                    range[0]
                )
            })?;
            let end = range[1].trim().parse::<u32>().map_err(|_| {
                format!(
                    "{}: {}",
                    localized(language, "잘못된 페이지 번호", "Invalid page number"),
                    range[1]
                )
            })?;

            if start == 0 || end == 0 || end < start {
                return Err(format!(
                    "{}: {}",
                    localized(language, "잘못된 페이지 범위", "Invalid page range"),
                    token
                ));
            }

            for page in start..=end {
                pages.push(page);
            }
            continue;
        }

        let page = token.parse::<u32>().map_err(|_| {
            format!(
                "{}: {}",
                localized(language, "잘못된 페이지 번호", "Invalid page number"),
                token
            )
        })?;
        if page == 0 {
            return Err(format!(
                "{}: {}",
                localized(language, "잘못된 페이지 번호", "Invalid page number"),
                token
            ));
        }
        pages.push(page);
    }

    if pages.is_empty() {
        return Err(
            localized(language, "pages 값이 비어 있습니다", "pages value is empty").to_string(),
        );
    }

    Ok(pages)
}

fn trex_error_response(error: TrexError, language: i18n::Language) -> Response {
    let status = match error {
        TrexError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    error_response(
        status,
        format!(
            "{}: {}",
            localized(language, "추출 오류", "Extraction error"),
            error
        ),
    )
}

fn error_response(status: StatusCode, message: String) -> Response {
    (status, Json(ErrorResponse { error: message })).into_response()
}

fn localized<'a>(language: i18n::Language, ko: &'a str, en: &'a str) -> &'a str {
    match language {
        i18n::Language::Korean => ko,
        i18n::Language::English => en,
    }
}
