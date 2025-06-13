use std::path::Path;

#[derive(Debug, Clone)]
pub enum SupportedLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    Cpp,
    C,
    Go,
    Ruby,
    PHP,
    CSharp,
    Swift,
    Kotlin,
    Scala,
    Clojure,
    Haskell,
    OCaml,
    Elm,
    Dart,
    Markdown,
    Text,
    Json,
    Yaml,
    Toml,
    Xml,
    Html,
    Css,
    Unknown,
}

pub struct LanguageDetector;

impl LanguageDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn detect_from_path(&self, path: &Path) -> SupportedLanguage {
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                match ext_str.to_lowercase().as_str() {
                    "rs" => SupportedLanguage::Rust,
                    "py" => SupportedLanguage::Python,
                    "js" => SupportedLanguage::JavaScript,
                    "ts" => SupportedLanguage::TypeScript,
                    "java" => SupportedLanguage::Java,
                    "cpp" | "cc" | "cxx" => SupportedLanguage::Cpp,
                    "c" => SupportedLanguage::C,
                    "go" => SupportedLanguage::Go,
                    "rb" => SupportedLanguage::Ruby,
                    "php" => SupportedLanguage::PHP,
                    "cs" => SupportedLanguage::CSharp,
                    "swift" => SupportedLanguage::Swift,
                    "kt" => SupportedLanguage::Kotlin,
                    "scala" => SupportedLanguage::Scala,
                    "clj" => SupportedLanguage::Clojure,
                    "hs" => SupportedLanguage::Haskell,
                    "ml" => SupportedLanguage::OCaml,
                    "elm" => SupportedLanguage::Elm,
                    "dart" => SupportedLanguage::Dart,
                    "md" => SupportedLanguage::Markdown,
                    "txt" => SupportedLanguage::Text,
                    "json" => SupportedLanguage::Json,
                    "yaml" | "yml" => SupportedLanguage::Yaml,
                    "toml" => SupportedLanguage::Toml,
                    "xml" => SupportedLanguage::Xml,
                    "html" | "htm" => SupportedLanguage::Html,
                    "css" => SupportedLanguage::Css,
                    _ => SupportedLanguage::Unknown,
                }
            } else {
                SupportedLanguage::Unknown
            }
        } else {
            SupportedLanguage::Unknown
        }
    }
} 