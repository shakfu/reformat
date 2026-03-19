//! Change tracking for refactoring operations
//!
//! This module provides types for recording changes made during refactoring
//! operations like file grouping, enabling subsequent reference fixing.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// A single change record from a refactoring operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Change {
    /// A directory was created
    DirectoryCreated {
        /// Path to the created directory (relative to base_dir)
        path: String,
    },
    /// A file was moved (and optionally renamed)
    FileMoved {
        /// Original path (relative to base_dir)
        from: String,
        /// New path (relative to base_dir)
        to: String,
    },
    /// A file was renamed in place
    FileRenamed {
        /// Original filename
        from: String,
        /// New filename
        to: String,
        /// Directory containing the file
        directory: String,
    },
}

/// Record of all changes from a refactoring operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    /// Name of the operation (e.g., "group", "rename_files")
    pub operation: String,
    /// ISO 8601 timestamp of when the operation occurred
    pub timestamp: String,
    /// Base directory where the operation was performed (absolute path)
    pub base_dir: String,
    /// Options used for the operation (operation-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
    /// List of changes made
    pub changes: Vec<Change>,
}

impl ChangeRecord {
    /// Creates a new change record
    pub fn new(operation: &str, base_dir: &Path) -> Self {
        let timestamp = chrono::Utc::now().to_rfc3339();
        ChangeRecord {
            operation: operation.to_string(),
            timestamp,
            base_dir: base_dir.to_string_lossy().to_string(),
            options: None,
            changes: Vec::new(),
        }
    }

    /// Sets the options for this operation
    pub fn with_options<T: Serialize>(mut self, options: &T) -> Self {
        self.options = serde_json::to_value(options).ok();
        self
    }

    /// Adds a directory creation change
    pub fn add_directory_created(&mut self, path: &str) {
        self.changes.push(Change::DirectoryCreated {
            path: path.to_string(),
        });
    }

    /// Adds a file move change
    pub fn add_file_moved(&mut self, from: &str, to: &str) {
        self.changes.push(Change::FileMoved {
            from: from.to_string(),
            to: to.to_string(),
        });
    }

    /// Adds a file rename change
    pub fn add_file_renamed(&mut self, from: &str, to: &str, directory: &str) {
        self.changes.push(Change::FileRenamed {
            from: from.to_string(),
            to: to.to_string(),
            directory: directory.to_string(),
        });
    }

    /// Returns true if there are no changes recorded
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Returns the number of changes
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Returns only the file move changes
    pub fn file_moves(&self) -> Vec<(&str, &str)> {
        self.changes
            .iter()
            .filter_map(|c| match c {
                Change::FileMoved { from, to } => Some((from.as_str(), to.as_str())),
                _ => None,
            })
            .collect()
    }

    /// Writes the change record to a JSON file
    pub fn write_to_file(&self, path: &Path) -> crate::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Reads a change record from a JSON file
    pub fn read_from_file(path: &Path) -> crate::Result<Self> {
        let json = fs::read_to_string(path)?;
        let record: ChangeRecord = serde_json::from_str(&json)?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_record_creation() {
        let record = ChangeRecord::new("group", Path::new("/tmp/test"));
        assert_eq!(record.operation, "group");
        assert!(record.changes.is_empty());
    }

    #[test]
    fn test_add_changes() {
        let mut record = ChangeRecord::new("group", Path::new("/tmp/test"));
        record.add_directory_created("wbs");
        record.add_file_moved("wbs_create.tmpl", "wbs/create.tmpl");

        assert_eq!(record.len(), 2);
        assert!(!record.is_empty());
    }

    #[test]
    fn test_file_moves() {
        let mut record = ChangeRecord::new("group", Path::new("/tmp/test"));
        record.add_directory_created("wbs");
        record.add_file_moved("wbs_create.tmpl", "wbs/create.tmpl");
        record.add_file_moved("wbs_delete.tmpl", "wbs/delete.tmpl");

        let moves = record.file_moves();
        assert_eq!(moves.len(), 2);
        assert_eq!(moves[0], ("wbs_create.tmpl", "wbs/create.tmpl"));
    }

    #[test]
    fn test_serialization() {
        let mut record = ChangeRecord::new("group", Path::new("/tmp/test"));
        record.add_directory_created("wbs");
        record.add_file_moved("wbs_create.tmpl", "wbs/create.tmpl");

        let json = serde_json::to_string_pretty(&record).unwrap();
        assert!(json.contains("\"operation\": \"group\""));
        assert!(json.contains("\"type\": \"directory_created\""));
        assert!(json.contains("\"type\": \"file_moved\""));
    }

    #[test]
    fn test_write_and_read() {
        let test_dir = std::env::temp_dir().join("reformat_changes_test");
        let _ = fs::create_dir_all(&test_dir);
        let file_path = test_dir.join("changes.json");

        let mut record = ChangeRecord::new("group", Path::new("/tmp/test"));
        record.add_file_moved("old.txt", "new/old.txt");
        record.write_to_file(&file_path).unwrap();

        let loaded = ChangeRecord::read_from_file(&file_path).unwrap();
        assert_eq!(loaded.operation, "group");
        assert_eq!(loaded.len(), 1);

        let _ = fs::remove_dir_all(&test_dir);
    }
}
