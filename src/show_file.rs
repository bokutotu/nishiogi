//! File Content Display Module
//!
//! This module provides functionality to read the contents of a file specified by its path,
//! returning the content as a string. It uses a dedicated error enum, `FileReadError`,
//! to clearly represent possible failure cases in a manner that facilitates pattern matching.

use std::{error::Error, fmt, fs, path::Path};

/// Represents errors that can occur while reading a file.
///
/// This enum encapsulates various error conditions encountered when attempting
/// to read a file's content. Its variants are designed for straightforward pattern matching,
/// avoiding the use of arbitrary strings where possible.
#[derive(Debug)]
pub enum FileReadError {
    /// The specified file does not exist.
    NotFound,
    /// The specified path refers to a directory, not a file.
    IsDirectory,
    /// An underlying I/O error occurred.
    Io(std::io::Error),
}

impl fmt::Display for FileReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileReadError::NotFound => write!(f, "File not found"),
            FileReadError::IsDirectory => write!(f, "Path is a directory, not a file"),
            FileReadError::Io(err) => write!(f, "I/O error: {err}"),
        }
    }
}

impl Error for FileReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FileReadError::Io(err) => Some(err),
            _ => None,
        }
    }
}

/// Reads the content of the file at the specified path and returns it as a string.
///
/// # Parameters
///
/// - `path`: The path of the file to be read.
///
/// # Returns
///
/// On success, returns the file's content as a `String`.
/// On failure, returns a `FileReadError` indicating the error type:
///
/// - `FileReadError::NotFound` if the file does not exist.
/// - `FileReadError::IsDirectory` if the specified path is a directory.
/// - `FileReadError::Io` if an I/O error occurs while reading the file.
pub fn read_file_content(path: &Path) -> Result<String, FileReadError> {
    if !path.exists() {
        return Err(FileReadError::NotFound);
    }
    if path.is_dir() {
        return Err(FileReadError::IsDirectory);
    }
    fs::read_to_string(path).map_err(FileReadError::Io)
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_read_file_content() {
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let file_path = temp_dir.path().join("test.txt");
        let test_content = "Hello, world!";
        let mut file = File::create(&file_path).expect("Failed to create test file");
        file.write_all(test_content.as_bytes())
            .expect("Failed to write to test file");
        let content = read_file_content(&file_path).expect("Failed to read file content");
        assert_eq!(content, test_content);
    }

    #[test]
    fn test_read_nonexistent_file() {
        let nonexistent_path = Path::new("/path/to/nonexistent/file");
        let result = read_file_content(nonexistent_path);
        assert!(matches!(result, Err(FileReadError::NotFound)));
    }

    #[test]
    fn test_read_directory() {
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let result = read_file_content(temp_dir.path());
        assert!(matches!(result, Err(FileReadError::IsDirectory)));
    }
}
