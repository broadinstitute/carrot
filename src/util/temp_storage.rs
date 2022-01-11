//! Contains functionality for interacting with temporary file storage

use log::error;
use std::io::Write;
use tempfile::NamedTempFile;

/// Creates a temporary file with `contents` and returns it
///
/// Creates a NamedTempFile and writes `contents` to it.  Returns the file if successful.  Returns
/// an error if creating or writing to the file fails
pub fn get_temp_file(contents: &[u8]) -> Result<NamedTempFile, std::io::Error> {
    match NamedTempFile::new() {
        Ok(mut file) => {
            if let Err(e) = file.write_all(contents) {
                error!(
                    "Encountered error while attempting to write to temporary file: {}",
                    e
                );
                Err(e)
            } else {
                Ok(file)
            }
        }
        Err(e) => {
            error!(
                "Encountered error while attempting to create temporary file: {}",
                e
            );
            Err(e)
        }
    }
}
