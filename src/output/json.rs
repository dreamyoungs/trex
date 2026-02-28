//! JSON 출력 변환

use crate::{Table, error::TrexError};

/// 테이블 목록을 JSON 문자열로 변환한다.
/// 이쁘게 들여쓰기된 형태로 출력한다.
pub fn to_string(tables: &[Table]) -> Result<String, TrexError> {
    serde_json::to_string_pretty(tables).map_err(TrexError::Json)
}
