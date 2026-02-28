//! CSV 출력 변환

use crate::{Table, error::TrexError};

/// 테이블 목록을 CSV 문자열로 변환한다.
/// 여러 테이블이 있으면 빈 줄로 구분한다.
pub fn to_string(tables: &[Table]) -> Result<String, TrexError> {
    let mut output = String::new();

    for (i, table) in tables.iter().enumerate() {
        if i > 0 {
            // 테이블 간 구분선
            output.push('\n');
        }

        // 헤더 행
        output.push_str(&table.headers.join(","));
        output.push('\n');

        // 데이터 행
        for row in &table.rows {
            // CSV 이스케이프: 쉼표나 따옴표가 포함된 값 처리
            let escaped: Vec<String> = row
                .iter()
                .map(|cell| {
                    if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                        format!("\"{}\"", cell.replace('"', "\"\""))
                    } else {
                        cell.clone()
                    }
                })
                .collect();
            output.push_str(&escaped.join(","));
            output.push('\n');
        }
    }

    Ok(output)
}
