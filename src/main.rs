use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use trex::{DlFallbackMode, DlRuntimeOptions, ExtractOptions, ParseMode, RuntimeOptions};

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
                return Err(format!("잘못된 페이지 범위: {}", part));
            }
            let start: u32 = range[0]
                .parse()
                .map_err(|_| format!("잘못된 페이지 번호: {}", range[0]))?;
            let end: u32 = range[1]
                .parse()
                .map_err(|_| format!("잘못된 페이지 번호: {}", range[1]))?;
            for p in start..=end {
                result.push(p);
            }
        } else {
            // 단일 페이지
            let p: u32 = part
                .parse()
                .map_err(|_| format!("잘못된 페이지 번호: {}", part))?;
            result.push(p);
        }
    }
    Ok(result)
}

fn main() {
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
                    eprintln!("오류: {}", e);
                    std::process::exit(1);
                })
            });

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

            // 추출 실행
            match trex::extract_with_runtime_options(&pdf_path, &options, &runtime) {
                Ok(tables) => {
                    // 출력 생성
                    let output_str = match format {
                        OutputFormat::Json => trex::output::to_json(&tables).unwrap_or_else(|e| {
                            eprintln!("JSON 변환 오류: {}", e);
                            std::process::exit(1);
                        }),
                        OutputFormat::Csv => trex::output::to_csv(&tables).unwrap_or_else(|e| {
                            eprintln!("CSV 변환 오류: {}", e);
                            std::process::exit(1);
                        }),
                    };

                    // 출력 대상 결정 (파일 또는 stdout)
                    match output {
                        Some(path) => {
                            std::fs::write(&path, &output_str).unwrap_or_else(|e| {
                                eprintln!("파일 쓰기 오류: {}: {}", path.display(), e);
                                std::process::exit(1);
                            });
                            eprintln!("결과 저장: {}", path.display());
                        }
                        None => {
                            println!("{}", output_str);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("추출 오류: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
