//! TREX 에러 타입 정의

/// TREX에서 발생할 수 있는 모든 에러를 분류한다.
#[derive(Debug, thiserror::Error)]
pub enum TrexError {
    /// PDF 파일을 읽거나 파싱하는 중 발생한 에러
    #[error("{0}")]
    PdfParse(String),

    /// 테이블 탐지 중 발생한 에러
    #[error("{0}")]
    Detection(String),

    /// 셀 병합 중 발생한 에러
    #[error("{0}")]
    Merge(String),

    /// 출력 변환 중 발생한 에러
    #[error("{0}")]
    Output(String),

    /// DL 추론 파이프라인 에러
    #[error("{0}")]
    Dl(String),

    /// 파일 I/O 에러
    #[error("{0}")]
    Io(#[from] std::io::Error),

    /// JSON 직렬화/역직렬화 에러
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}
