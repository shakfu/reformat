//! Core library for code transformation and case conversion
//!
//! This library provides the fundamental building blocks for transforming code,
//! including case format conversion, pattern matching, and file processing.

pub mod case;
pub mod changes;
pub mod combined;
pub mod converter;
pub mod emoji;
pub mod group;
pub mod refs;
pub mod rename;
pub mod whitespace;

// Re-export commonly used types
pub use case::CaseFormat;
pub use changes::{Change, ChangeRecord};
pub use combined::{CombinedOptions, CombinedProcessor, CombinedStats};
pub use converter::CaseConverter;
pub use emoji::{EmojiOptions, EmojiTransformer};
pub use group::{FileGrouper, GroupOptions, GroupResult, GroupStats};
pub use refs::{ApplyResult, FixRecord, ReferenceFix, ReferenceFixer, ReferenceScanner, ScanOptions};
pub use rename::{CaseTransform, FileRenamer, RenameOptions, SpaceReplace, TimestampFormat};
pub use whitespace::{WhitespaceCleaner, WhitespaceOptions};

// Re-export Result type
pub type Result<T> = anyhow::Result<T>;
