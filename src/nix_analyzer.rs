use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use ide::{
    AnalysisHost,
    Change,
    // SourceDatabase, // Unused
    // DefDatabase,    // Unused
    Diagnostic,
    FileId,
    FileSet,
    Severity as IdeSeverity, // DiagnosticKind as NilDiagnosticKind was unused
    SourceRoot,
    VfsPath, // Added FileSet, SourceRoot, VfsPath
};
use syntax::ast::SourceFile; // AstNode was unused
use syntax::Error as SyntaxErrorFull;
// use crate::Result;
// use tracing::{debug, info, warn};

/// Severity levels for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NgSeverity {
    Error,
    Warning,
    // Info and Hint are not in ide::Severity
}

/// Represents a diagnostic from various tools
#[derive(Debug)]
pub struct NgDiagnostic {
    pub tool_name: Option<String>, // Name of the tool that generated the diagnostic
    pub file_path: PathBuf,        // The path to the file
    pub line: Option<u32>,         // Start line number (1-based)
    pub column: Option<u32>,       // Start column number (1-based)
    // Consider adding end_line, end_column if needed in future
    pub message: String,
    pub severity: NgSeverity,
}

/// Central hub for Nix code analysis
pub struct NixAnalysisContext {
    db: AnalysisHost,
    file_map: HashMap<PathBuf, FileId>,
    next_file_id: u32,
}

impl NixAnalysisContext {
    pub fn new() -> Self {
        Self {
            db: AnalysisHost::default(), // Use AnalysisHost
            file_map: HashMap::new(),
            next_file_id: 0,
        }
    }

    /// Get or assign a FileId for a path
    fn get_or_assign_file_id(&mut self, path: &Path) -> FileId {
        if let Some(id) = self.file_map.get(path) {
            return *id;
        }
        let file_id = FileId(self.next_file_id);
        self.next_file_id += 1;
        self.file_map.insert(path.to_path_buf(), file_id);
        // Initially set file source to be the path itself for nil's VFS
        // self.db.set_file_source(file_id, FileSource::Local(path.to_path_buf())); // FileSource unresolved, commented out
        file_id
    }

    /// Parse a file with nil-syntax
    pub fn parse_file_with_syntax(
        &mut self,
        path: &Path,
        content: Arc<String>,
    ) -> (FileId, Arc<SourceFile>, Vec<SyntaxErrorFull>) {
        let file_id = self.get_or_assign_file_id(path);

        let mut change = Change::default();
        let content_for_db: Arc<str> = Arc::from(content.as_str());
        change.change_file(file_id, content_for_db.clone()); // Pass Arc<str> directly

        // Basic SourceRoot setup for the current file
        let mut file_set = FileSet::default();
        // Use the actual path for VfsPath, assuming it's absolute or resolvable
        // VfsPath::new needs something that can be AsRef<Path>
        // Let's try to make it a canonical/absolute path for robustness if possible,
        // otherwise, use it as is. For simplicity now, use as is.
        file_set.insert(file_id, VfsPath::from(path.to_path_buf()));
        change.set_roots(vec![SourceRoot::new_local(file_set, Some(file_id))]);

        self.db.apply_change(change);

        // Perform syntax parsing using the `syntax` crate's parser
        let parse_result: syntax::Parse = syntax::parse_file(content.as_str());
        let source_file_ast: SourceFile = parse_result.root(); // SourceFile from syntax::ast
        let errors: Vec<SyntaxErrorFull> = parse_result.errors().to_vec(); // SyntaxErrorFull is alias for syntax::Error

        (file_id, Arc::new(source_file_ast), errors)
    }

    /// Get semantic diagnostics for a file
    // Removed nil_config parameter. Configuration is implicit in AnalysisHost setup or via specific methods.
    pub fn get_semantic_diagnostics(
        &self,
        file_id: FileId,
    ) -> Result<Vec<Diagnostic>, ide::Cancelled> {
        self.db.snapshot().diagnostics(file_id)
    }

    /// Get the content of a file from the database
    // pub fn get_file_content(&self, file_id: FileId) -> Option<Arc<String>> {
    //     // AnalysisHost does not directly expose file_text.
    //     // To get text known to the DB, you might query self.db.snapshot().file_text(file_id) if Analysis/RootDatabase exposes it.
    //     None
    // }

    /// Convert a nil-syntax error to an NgDiagnostic
    pub fn convert_nil_syntax_error_to_ng(
        &self,
        e: &SyntaxErrorFull,
        _file_id_for_db: FileId,
        file_path: &Path,
    ) -> NgDiagnostic {
        // TODO: Restore line/column information by fixing file_line_index access
        NgDiagnostic {
            tool_name: Some("nil-syntax".to_string()),
            file_path: file_path.to_path_buf(),
            line: None,   // Temporarily None
            column: None, // Temporarily None
            message: e.kind.to_string(),
            severity: NgSeverity::Error,
        }
    }

    /// Convert a nil-ide diagnostic to an NgDiagnostic
    pub fn convert_nil_diagnostic_to_ng(
        &self,
        d: &Diagnostic,
        _file_id_for_db: FileId,
        file_path: &Path,
    ) -> NgDiagnostic {
        // TODO: Restore line/column information by fixing file_line_index access
        NgDiagnostic {
            tool_name: Some("nil-ide".to_string()),
            file_path: file_path.to_path_buf(),
            line: None,   // Temporarily None
            column: None, // Temporarily None
            message: d.message().clone(),
            severity: match d.severity() {
                IdeSeverity::Error => NgSeverity::Error,
                IdeSeverity::Warning => NgSeverity::Warning,
                IdeSeverity::IncompleteSyntax => NgSeverity::Error,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_analysis_context() {
        let analyzer = NixAnalysisContext::new();
        assert_eq!(analyzer.file_map.len(), 0);
    }

    #[test]
    fn test_get_or_assign_file_id() {
        let mut analyzer = NixAnalysisContext::new();
        let path = Path::new("/tmp/test.nix");
        let file_id = analyzer.get_or_assign_file_id(path);
        assert_eq!(file_id, FileId(0));

        // Getting the same path should return the same id
        let file_id2 = analyzer.get_or_assign_file_id(path);
        assert_eq!(file_id, file_id2);

        // Getting a different path should return a different id
        let path2 = Path::new("/tmp/test2.nix");
        let file_id3 = analyzer.get_or_assign_file_id(path2);
        assert_eq!(file_id3, FileId(1));
    }
}
