//! PDF 파일 로딩 및 페이지 분리
//!
//! `lopdf` 크레이트를 사용하여 PDF 파일을 로딩하고,
//! 페이지 단위로 분리하여 처리할 수 있도록 한다.

use crate::error::TrexError;
use std::path::Path;

/// PDF 문서를 나타내는 구조체
pub struct PdfDocument {
    /// lopdf 문서 객체
    inner: lopdf::Document,
}

impl PdfDocument {
    /// PDF 파일을 로딩한다.
    ///
    /// # Arguments
    /// * `path` - PDF 파일 경로
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, TrexError> {
        let doc = lopdf::Document::load(path)
            .map_err(|e| TrexError::PdfParse(format!("PDF 로딩 실패: {}", e)))?;

        Ok(Self { inner: doc })
    }

    /// 전체 페이지 수를 반환한다.
    pub fn page_count(&self) -> u32 {
        self.inner.get_pages().len() as u32
    }

    /// 특정 페이지의 내부 객체 ID를 반환한다.
    pub fn page_ids(&self) -> Vec<(u32, lopdf::ObjectId)> {
        self.inner.get_pages().into_iter().collect()
    }

    /// 내부 lopdf 문서에 대한 참조를 반환한다.
    pub fn inner(&self) -> &lopdf::Document {
        &self.inner
    }
}
