//! Strict PDF 2.0 rendering for controlled documents. Unsupported semantics fail explicitly.
#![forbid(unsafe_code)]
mod codecs;
mod content;
mod pdf;
mod render;
pub use content::TextRun;
pub use pdf::{Document, Error, Limits, LinkAnnotation, PageInfo};
pub use render::RenderedPage;
