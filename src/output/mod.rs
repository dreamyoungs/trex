//! 출력 모듈 — 추출 결과를 JSON 또는 CSV로 변환

pub mod csv;
pub mod json;

use crate::{Table, error::TrexError};

/// 테이블 목록을 JSON 문자열로 변환한다.
pub fn to_json(tables: &[Table]) -> Result<String, TrexError> {
    json::to_string(tables)
}

/// 테이블 목록을 CSV 문자열로 변환한다.
pub fn to_csv(tables: &[Table]) -> Result<String, TrexError> {
    csv::to_string(tables)
}
