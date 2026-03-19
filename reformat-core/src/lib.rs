//! # reformat-core
//!
//! Core library for code transformation and reformatting.
//!
//! Provides case format conversion, whitespace cleaning, emoji transformation,
//! file renaming, and file grouping with broken reference detection.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use reformat_core::{WhitespaceCleaner, WhitespaceOptions};
//!
//! let options = WhitespaceOptions::default();
//! let cleaner = WhitespaceCleaner::new(options);
//! let (files, lines) = cleaner.process(std::path::Path::new("src")).unwrap();
//! println!("Cleaned {} lines in {} files", lines, files);
//! ```

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
pub use refs::{
    ApplyResult, FixRecord, ReferenceFix, ReferenceFixer, ReferenceScanner, ScanOptions,
};
pub use rename::{CaseTransform, FileRenamer, RenameOptions, SpaceReplace, TimestampFormat};
pub use whitespace::{WhitespaceCleaner, WhitespaceOptions};

// Re-export Result type
pub type Result<T> = anyhow::Result<T>;
