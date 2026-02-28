use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::time::Instant;
use trex::{DlFallbackMode, DlRuntimeOptions, ExtractOptions, ParseMode, RuntimeOptions, i18n};

mod server;

/// TREX — Table Rust EXtractor
///
/// PDF에서 표(Table)를 추출하여 JSON 또는 CSV로 출력한다.
#[derive(Parser)]
#[command(name = "trex", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// PDF 파일에서 표를 추출한다.
    Extract {
        /// PDF 파일 경로
        pdf_path: PathBuf,

        /// 처리할 페이지 (예: 1,3,5 또는 1-10)
        #[arg(long)]
        pages: Option<String>,

        /// 파싱 모드
        #[arg(long, value_enum, default_value = "auto")]
        mode: CliParseMode,

        /// 출력 형식
        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,

        /// 출력 파일 경로 (미지정 시 stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// DL 라우터 ONNX 모델 경로 (`--mode dl`에서 사용)
        #[arg(long)]
        dl_model: Option<PathBuf>,

        /// DL 라우터 최소 신뢰도 (0.0 ~ 1.0)
        #[arg(long, default_value_t = 0.55)]
        dl_min_confidence: f32,

        /// DL 저신뢰/모델 미사용 시 폴백 모드
        #[arg(long, value_enum, default_value = "auto")]
        dl_fallback: CliDlFallbackMode,

        /// 추출 이벤트를 NDJSON으로 기록할 파일 경로
        #[arg(long)]
        event_log: Option<PathBuf>,

        /// 문서 식별자 (미지정 시 pdf 경로 문자열 사용)
        #[arg(long)]
        event_document_key: Option<String>,

        /// 고객사(tenant) 식별자
        #[arg(long)]
        event_tenant_id: Option<String>,

        /// 요청 식별자
        #[arg(long)]
        event_request_id: Option<String>,

        /// 사용자 피드백 태그 (예: missing_table)
        #[arg(long)]
        event_feedback_tag: Option<String>,

        /// 해당 요청 데이터를 학습에 사용해도 되는지 여부
        #[arg(long)]
        event_training_opt_in: bool,
    },

    /// REST API 서버를 실행한다.
    Serve {
        /// 바인딩할 호스트
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// 바인딩할 포트
        #[arg(long, default_value_t = 8080)]
        port: u16,

        /// 기본 DL 라우터 ONNX 모델 경로
        #[arg(long)]
        dl_model: Option<PathBuf>,

        /// 기본 DL 라우터 최소 신뢰도 (0.0 ~ 1.0)
        #[arg(long, default_value_t = 0.55)]
        dl_min_confidence: f32,

        /// 기본 DL 저신뢰/모델 미사용 시 폴백 모드
        #[arg(long, value_enum, default_value = "auto")]
        dl_fallback: CliDlFallbackMode,
    },
}

/// CLI용 파싱 모드
#[derive(Clone, Copy, ValueEnum)]
enum CliParseMode {
    /// 격자선 기반 탐지
    Lattice,
    /// 좌표 기반 추론
    Stream,
    /// DL 라우터 기반 탐지
    Dl,
    /// 자동 선택
    Auto,
}

/// CLI용 DL 폴백 모드
#[derive(Clone, Copy, ValueEnum)]
enum CliDlFallbackMode {
    Auto,
    Lattice,
    Stream,
}

/// 출력 형식
#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Csv,
}

/// 페이지 문자열을 파싱한다 (예: "1,3,5" → [1,3,5], "1-5" → [1,2,3,4,5])
fn parse_pages(pages_str: &str) -> Result<Vec<u32>, String> {
    let mut result = Vec::new();
    for part in pages_str.split(',') {
        let part = part.trim();
        if part.contains('-') {
            // 범위 표현 (예: 1-5)
            let range: Vec<&str> = part.split('-').collect();
            if range.len() != 2 {
                return Err(format!(
                    "{}: {}",
                    i18n::text("잘못된 페이지 범위", "Invalid page range"),
                    part
                ));
            }
            let start: u32 = range[0].parse().map_err(|_| {
                format!(
                    "{}: {}",
                    i18n::text("잘못된 페이지 번호", "Invalid page number"),
                    range[0]
                )
            })?;
            let end: u32 = range[1].parse().map_err(|_| {
                format!(
                    "{}: {}",
                    i18n::text("잘못된 페이지 번호", "Invalid page number"),
                    range[1]
                )
            })?;
            for p in start..=end {
                result.push(p);
            }
        } else {
            // 단일 페이지
            let p: u32 = part.parse().map_err(|_| {
                format!(
                    "{}: {}",
                    i18n::text("잘못된 페이지 번호", "Invalid page number"),
                    part
                )
            })?;
            result.push(p);
        }
    }
    Ok(result)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Extract {
            pdf_path,
            pages,
            mode,
            format,
            output,
            dl_model,
            dl_min_confidence,
            dl_fallback,
            event_log,
            event_document_key,
            event_tenant_id,
            event_request_id,
            event_feedback_tag,
            event_training_opt_in,
        } => {
            // 파싱 모드 변환
            let parse_mode = match mode {
                CliParseMode::Lattice => ParseMode::Lattice,
                CliParseMode::Stream => ParseMode::Stream,
                CliParseMode::Dl => ParseMode::Dl,
                CliParseMode::Auto => ParseMode::Auto,
            };

            let fallback_mode = match dl_fallback {
                CliDlFallbackMode::Auto => DlFallbackMode::Auto,
                CliDlFallbackMode::Lattice => DlFallbackMode::Lattice,
                CliDlFallbackMode::Stream => DlFallbackMode::Stream,
            };

            // 페이지 옵션 파싱
            let pages = pages.as_deref().map(|p| {
                parse_pages(p).unwrap_or_else(|e| {
                    eprintln!("{}: {}", i18n::text("오류", "Error"), e);
                    std::process::exit(1);
                })
            });

            let pages_for_event = pages.clone();
            let document_key = event_document_key.unwrap_or_else(|| pdf_path.display().to_string());
            let dl_model_for_event = dl_model.as_ref().map(|path| path.display().to_string());

            let options = ExtractOptions {
                pages,
                mode: parse_mode,
            };

            let runtime = RuntimeOptions {
                dl: DlRuntimeOptions {
                    model_path: dl_model,
                    min_confidence: dl_min_confidence,
                    fallback_mode,
                },
            };

            let started = Instant::now();

            // 추출 실행
            match trex::extract_with_runtime_options(&pdf_path, &options, &runtime) {
                Ok(tables) => {
                    if let Some(log_path) = event_log.as_ref() {
                        let event = trex::telemetry::success_event(
                            trex::telemetry::EventContext {
                                tenant_id: event_tenant_id.clone(),
                                request_id: event_request_id.clone(),
                                document_key: document_key.clone(),
                                training_opt_in: event_training_opt_in,
                                feedback_tag: event_feedback_tag.clone(),
                                requested_mode: parse_mode,
                                pages_requested: pages_for_event.clone(),
                                dl_model_path: dl_model_for_event.clone(),
                                dl_min_confidence,
                                dl_fallback: fallback_mode,
                                duration_ms: started.elapsed().as_millis(),
                            },
                            &tables,
                        );

                        if let Err(log_error) = trex::telemetry::append_event(log_path, &event) {
                            eprintln!(
                                "{}: {}",
                                i18n::text("이벤트 로그 저장 오류", "Failed to write event log"),
                                log_error
                            );
                        }
                    }

                    // 출력 생성
                    let output_str = match format {
                        OutputFormat::Json => trex::output::to_json(&tables).unwrap_or_else(|e| {
                            eprintln!(
                                "{}: {}",
                                i18n::text("JSON 변환 오류", "JSON conversion error"),
                                e
                            );
                            std::process::exit(1);
                        }),
                        OutputFormat::Csv => trex::output::to_csv(&tables).unwrap_or_else(|e| {
                            eprintln!(
                                "{}: {}",
                                i18n::text("CSV 변환 오류", "CSV conversion error"),
                                e
                            );
                            std::process::exit(1);
                        }),
                    };

                    // 출력 대상 결정 (파일 또는 stdout)
                    match output {
                        Some(path) => {
                            std::fs::write(&path, &output_str).unwrap_or_else(|e| {
                                eprintln!(
                                    "{}: {}: {}",
                                    i18n::text("파일 쓰기 오류", "File write error"),
                                    path.display(),
                                    e
                                );
                                std::process::exit(1);
                            });
                            eprintln!(
                                "{}: {}",
                                i18n::text("결과 저장", "Saved output"),
                                path.display()
                            );
                        }
                        None => {
                            println!("{}", output_str);
                        }
                    }
                }
                Err(e) => {
                    if let Some(log_path) = event_log.as_ref() {
                        let event = trex::telemetry::error_event(
                            trex::telemetry::EventContext {
                                tenant_id: event_tenant_id,
                                request_id: event_request_id,
                                document_key,
                                training_opt_in: event_training_opt_in,
                                feedback_tag: event_feedback_tag,
                                requested_mode: parse_mode,
                                pages_requested: pages_for_event,
                                dl_model_path: dl_model_for_event,
                                dl_min_confidence,
                                dl_fallback: fallback_mode,
                                duration_ms: started.elapsed().as_millis(),
                            },
                            &e,
                        );

                        if let Err(log_error) = trex::telemetry::append_event(log_path, &event) {
                            eprintln!(
                                "{}: {}",
                                i18n::text("이벤트 로그 저장 오류", "Failed to write event log"),
                                log_error
                            );
                        }
                    }

                    eprintln!("{}: {}", i18n::text("추출 오류", "Extraction error"), e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Serve {
            host,
            port,
            dl_model,
            dl_min_confidence,
            dl_fallback,
        } => {
            let fallback_mode = match dl_fallback {
                CliDlFallbackMode::Auto => DlFallbackMode::Auto,
                CliDlFallbackMode::Lattice => DlFallbackMode::Lattice,
                CliDlFallbackMode::Stream => DlFallbackMode::Stream,
            };

            let runtime = RuntimeOptions {
                dl: DlRuntimeOptions {
                    model_path: dl_model,
                    min_confidence: dl_min_confidence,
                    fallback_mode,
                },
            };

            if let Err(error) = server::run(server::ServeConfig {
                host,
                port,
                runtime,
            })
            .await
            {
                eprintln!("{}: {}", i18n::text("서버 오류", "Server error"), error);
                std::process::exit(1);
            }
        }
    }
}
