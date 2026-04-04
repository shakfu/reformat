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
pub mod config;
pub mod converter;
pub mod emoji;
pub mod endings;
pub mod group;
pub mod header;
pub mod indent;
pub mod refs;
pub mod rename;
pub mod replace;
pub mod whitespace;

// Re-export commonly used types
pub use case::CaseFormat;
pub use changes::{Change, ChangeRecord};
pub use combined::{CombinedOptions, CombinedProcessor, CombinedStats};
pub use config::{Preset, ReformatConfig};
pub use converter::CaseConverter;
pub use emoji::{EmojiOptions, EmojiTransformer};
pub use endings::{EndingsNormalizer, EndingsOptions, LineEnding};
pub use group::{FileGrouper, GroupOptions, GroupResult, GroupStats};
pub use header::{HeaderManager, HeaderOptions};
pub use indent::{IndentNormalizer, IndentOptions, IndentStyle};
pub use refs::{
    ApplyResult, FixRecord, ReferenceFix, ReferenceFixer, ReferenceScanner, ScanOptions,
};
pub use rename::{CaseTransform, FileRenamer, RenameOptions, SpaceReplace, TimestampFormat};
pub use replace::{ContentReplacer, ReplaceOptions, ReplacePattern, ReplacePatternConfig};
pub use whitespace::{WhitespaceCleaner, WhitespaceOptions};

// Re-export Result type
pub type Result<T> = anyhow::Result<T>;
