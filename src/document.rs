use std::fs;
use std::path::{Path, PathBuf};

use url::Url;

pub const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "mkd"];

#[derive(Debug, Clone)]
pub struct Document {
    pub path: PathBuf,
    pub content: String,
    pub bytes: usize,
    pub lines: usize,
    pub image_base_uri: Option<String>,
}

impl Document {
    pub fn title(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Markdown".to_owned())
    }

    pub fn display_path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

pub fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            MARKDOWN_EXTENSIONS
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
}

pub fn load_document(path: impl AsRef<Path>) -> Result<Document, String> {
    let supplied_path = path.as_ref();
    if !is_markdown_path(supplied_path) {
        return Err(format!(
            "Not a Markdown file: {}",
            supplied_path.to_string_lossy()
        ));
    }

    let metadata = fs::metadata(supplied_path)
        .map_err(|error| format!("Cannot access {}: {error}", supplied_path.to_string_lossy()))?;
    if !metadata.is_file() {
        return Err(format!(
            "Not a regular file: {}",
            supplied_path.to_string_lossy()
        ));
    }
    if metadata.len() > MAX_DOCUMENT_BYTES {
        return Err(format!(
            "Markdown file is larger than the 64 MiB safety limit: {}",
            supplied_path.to_string_lossy()
        ));
    }

    let mut content = fs::read_to_string(supplied_path).map_err(|error| {
        format!(
            "Cannot read {} as UTF-8 text: {error}",
            supplied_path.to_string_lossy()
        )
    })?;
    if content.starts_with('\u{feff}') {
        content.remove(0);
    }

    let path = supplied_path
        .canonicalize()
        .unwrap_or_else(|_| supplied_path.to_path_buf());
    let image_base_uri = path
        .parent()
        .and_then(|parent| Url::from_directory_path(parent).ok())
        .map(String::from);

    Ok(Document {
        bytes: content.len(),
        lines: content.lines().count(),
        path,
        content,
        image_base_uri,
    })
}

pub fn human_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_path(extension: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "md-viewer-test-{}-{nonce}.{extension}",
            std::process::id()
        ))
    }

    #[test]
    fn recognizes_markdown_extensions_case_insensitively() {
        assert!(is_markdown_path(Path::new("README.md")));
        assert!(is_markdown_path(Path::new("README.MARKDOWN")));
        assert!(!is_markdown_path(Path::new("notes.txt")));
        assert!(!is_markdown_path(Path::new("README")));
    }

    #[test]
    fn loads_utf8_document_and_strips_bom() {
        let path = temporary_path("md");
        fs::write(&path, "\u{feff}# Überschrift\n\nText\n").unwrap();
        let document = load_document(&path).unwrap();
        assert_eq!(document.content, "# Überschrift\n\nText\n");
        assert_eq!(document.lines, 3);
        assert!(
            document
                .image_base_uri
                .as_deref()
                .unwrap()
                .starts_with("file:")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_non_markdown_and_invalid_utf8() {
        let text_path = temporary_path("txt");
        fs::write(&text_path, "text").unwrap();
        assert!(load_document(&text_path).is_err());
        fs::remove_file(text_path).unwrap();

        let binary_path = temporary_path("md");
        fs::write(&binary_path, [0xff, 0xfe, 0xfd]).unwrap();
        assert!(load_document(&binary_path).is_err());
        fs::remove_file(binary_path).unwrap();
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(human_size(12), "12 B");
        assert_eq!(human_size(1536), "1.5 KiB");
        assert_eq!(human_size(2 * 1024 * 1024), "2.0 MiB");
    }
}
