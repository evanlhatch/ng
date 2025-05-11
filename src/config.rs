use serde::Deserialize;
use toml;
use std::path::PathBuf; // Required for load_from_path

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct NgConfig {
    #[serde(default)]
    pub pre_flight: PreFlightConfig,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct PreFlightConfig {
    pub checks: Option<Vec<String>>,
    pub strict_lint: Option<bool>,
    pub strict_format: Option<bool>,
    #[serde(default)]
    pub format: FormatConfig,
    #[serde(default)]
    pub external_linters: ExternalLintersConfig, // ADDED
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct FormatConfig {
    pub tool: Option<String>,
}

// ADDED NEW STRUCT
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct ExternalLintersConfig {
    pub enable: Option<Vec<String>>, // e.g., ["statix", "deadnix"]
    pub statix_path: Option<String>,
    pub deadnix_path: Option<String>,
    pub statix_args: Option<Vec<String>>,
    pub deadnix_args: Option<Vec<String>>,
}

impl NgConfig {
    pub fn from_str(toml_content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_content)
    }

    pub fn load_from_path(path: &PathBuf) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => match Self::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("[WARN] Failed to parse config file at {}: {}. Using default configuration.", path.display(), e);
                    Self::default()
                }
            },
            Err(_) => {
                // eprintln!("[DEBUG] Config file at {} not found. Using default configuration.", path.display());
                Self::default()
            }
        }
    }

    pub fn load() -> Self { 
        // TODO: Implement search for ng.toml in current dir then parent dirs.
        // For now, simple load from current directory "ng.toml"
        Self::load_from_path(&PathBuf::from("ng.toml"))
    }
}
