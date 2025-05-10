use nil_syntax::{SourceFile, AstNode, TextRange, SyntaxError};
use nil_ide::{
    base::{SourceDatabase, SourceDatabaseExt, FileId, Change, FileSource},
    def::DefDatabase, // For name resolution, etc.
    diagnostics::{self, Diagnostic, DiagnosticKind as NilDiagnosticKind}, // Renamed to avoid clash
    RootDatabase, // The central query database from nil-ide
    config::Config as NilConfig, // nil's config
};
use std::{sync::Arc, collections::HashMap, path::{Path, PathBuf}};
use crate::Result; // Your project's Result
use tracing::{debug, info, warn};

/// Severity levels for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NgSeverity { 
    Error, 
    Warning, 
    Info, 
    Hint 
}

/// Represents a diagnostic from nil-syntax or nil-ide
#[derive(Debug)]
pub struct NgDiagnostic { 
    /// The FileId for the nil database
    pub file_id_for_db: FileId,
    
    /// The path to the file
    pub file_path: PathBuf,
    
    /// The range in the file where the diagnostic applies
    pub range: TextRange,
    
    /// The diagnostic message
    pub message: String,
    
    /// The severity of the diagnostic
    pub severity: NgSeverity,
}

/// Central hub for Nix code analysis using nil-syntax and nil-ide
pub struct NixAnalysisContext {
    /// The nil-ide RootDatabase
    db: RootDatabase,
    
    /// Map from file paths to nil's FileId
    file_map: HashMap<PathBuf, FileId>,
    
    /// Counter for generating new FileIds
    next_file_id: u32,
}

impl NixAnalysisContext {
    /// Create a new NixAnalysisContext
    pub fn new() -> Self {
        Self {
            db: RootDatabase::default(),
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
        self.db.set_file_source(file_id, FileSource::Local(path.to_path_buf()));
        file_id
    }

    /// Parse a file with nil-syntax
    pub fn parse_file_with_syntax(&mut self, path: &Path, content: Arc<String>) -> (FileId, Arc<SourceFile>, Vec<SyntaxError>) {
        let file_id = self.get_or_assign_file_id(path);
        let mut change = Change::new();
        change.change_file(file_id, Some(content));
        self.db.apply_change(change);
        let source_file = self.db.parse(file_id);
        let errors = source_file.errors().to_vec(); // Clone errors
        (file_id, source_file, errors)
    }

    /// Get semantic diagnostics for a file
    pub fn get_semantic_diagnostics(&self, file_id: FileId, nil_config: &NilConfig) -> Vec<Diagnostic> {
        // This query will trigger parsing, name resolution, HIR lowering, etc., as needed by nil's DB.
        diagnostics::diagnostics(&self.db, file_id, nil_config)
    }

    /// Get the content of a file from the database
    pub fn get_file_content(&self, file_id: FileId) -> Option<Arc<String>> {
        self.db.file_text(file_id)
    }

    /// Convert a nil-syntax error to an NgDiagnostic
    pub fn convert_nil_syntax_error_to_ng(&self, e: &SyntaxError, file_id_for_db: FileId, file_path: &Path) -> NgDiagnostic {
        NgDiagnostic {
            file_id_for_db,
            file_path: file_path.to_path_buf(),
            range: e.range(),
            message: e.message().to_string(),
            severity: NgSeverity::Error,
        }
    }

    /// Convert a nil-ide diagnostic to an NgDiagnostic
    pub fn convert_nil_diagnostic_to_ng(&self, d: &Diagnostic, file_id_for_db: FileId, file_path: &Path) -> NgDiagnostic {
        NgDiagnostic {
            file_id_for_db,
            file_path: file_path.to_path_buf(),
            range: d.range,
            message: d.message.clone(),
            severity: match d.severity {
                nil_ide::Severity::Error => NgSeverity::Error,
                nil_ide::Severity::Warning => NgSeverity::Warning,
                nil_ide::Severity::Info => NgSeverity::Info,
                nil_ide::Severity::Hint => NgSeverity::Hint,
            }
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