use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Application configuration with support for multiple sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Number of days to retain log data
    pub log_retention_days: u32,

    /// Maximum log database size in GB
    pub log_max_size_gb: f64,

    /// Number of days to retain process metrics
    pub process_retention_days: u32,

    /// Maximum process metrics database size in GB
    pub process_max_size_gb: f64,

    /// Cleanup interval in minutes (clamped to 5-15 range)
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_minutes: u32,

    /// Path to the config file
    #[serde(skip)]
    pub config_file: PathBuf,
}

fn default_cleanup_interval() -> u32 {
    10
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            log_retention_days: 30,
            log_max_size_gb: 1.0,
            process_retention_days: 7,
            process_max_size_gb: 0.5,
            cleanup_interval_minutes: 10,
            config_file: Self::default_config_path(),
        }
    }
}

impl Settings {
    /// Get the default config file path
    pub fn default_config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".livedata").join("config.toml")
    }

    /// Load settings from file, environment, and CLI args
    /// Priority: CLI args > Environment variables > Config file > Defaults
    pub fn load() -> Result<Self> {
        let mut settings = Settings::default();

        // Try to load from config file
        if settings.config_file.exists() {
            settings = Self::load_from_file(&settings.config_file)?;
        } else {
            // Create default config file
            Self::create_default_config(&settings.config_file)?;
        }

        // Override with environment variables
        settings.apply_env_vars();

        Ok(settings)
    }

    /// Load settings with CLI overrides
    pub fn load_with_cli_args(
        log_retention_days: Option<u32>,
        log_max_size_gb: Option<f64>,
        process_retention_days: Option<u32>,
        process_max_size_gb: Option<f64>,
        cleanup_interval_minutes: Option<u32>,
    ) -> Result<Self> {
        let mut settings = Self::load()?;

        // Apply CLI overrides (highest priority)
        if let Some(days) = log_retention_days {
            settings.log_retention_days = days;
        }
        if let Some(size) = log_max_size_gb {
            settings.log_max_size_gb = size;
        }
        if let Some(days) = process_retention_days {
            settings.process_retention_days = days;
        }
        if let Some(size) = process_max_size_gb {
            settings.process_max_size_gb = size;
        }
        if let Some(interval) = cleanup_interval_minutes {
            settings.cleanup_interval_minutes = Self::clamp_cleanup_interval(interval);
        }

        Ok(settings)
    }

    /// Clamp cleanup interval to 5-15 minute range
    fn clamp_cleanup_interval(interval: u32) -> u32 {
        interval.clamp(5, 15)
    }

    /// Load settings from a TOML file
    fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = fs::read_to_string(path.as_ref()).context("Failed to read config file")?;

        let mut settings: Settings =
            toml::from_str(&contents).context("Failed to parse config file")?;

        settings.config_file = path.as_ref().to_path_buf();

        Ok(settings)
    }

    /// Apply environment variable overrides
    fn apply_env_vars(&mut self) {
        if let Ok(val) = std::env::var("LIVEDATA_LOG_RETENTION_DAYS")
            && let Ok(days) = val.parse()
        {
            self.log_retention_days = days;
        }

        if let Ok(val) = std::env::var("LIVEDATA_LOG_MAX_SIZE_GB")
            && let Ok(size) = val.parse()
        {
            self.log_max_size_gb = size;
        }

        if let Ok(val) = std::env::var("LIVEDATA_PROCESS_RETENTION_DAYS")
            && let Ok(days) = val.parse()
        {
            self.process_retention_days = days;
        }

        if let Ok(val) = std::env::var("LIVEDATA_PROCESS_MAX_SIZE_GB")
            && let Ok(size) = val.parse()
        {
            self.process_max_size_gb = size;
        }

        if let Ok(val) = std::env::var("LIVEDATA_RETENTION_CLEANUP_INTERVAL")
            && let Ok(interval) = val.parse()
        {
            self.cleanup_interval_minutes = Self::clamp_cleanup_interval(interval);
        }
    }

    /// Create a default config file
    fn create_default_config<P: AsRef<Path>>(path: P) -> Result<()> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let default_settings = Settings::default();
        let toml_content = toml::to_string_pretty(&default_settings)
            .context("Failed to serialize default config")?;

        fs::write(path, toml_content).context("Failed to write default config file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.log_retention_days, 30);
        assert_eq!(settings.log_max_size_gb, 1.0);
        assert_eq!(settings.process_retention_days, 7);
        assert_eq!(settings.process_max_size_gb, 0.5);
    }

    #[test]
    fn test_create_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // Create default config
        Settings::create_default_config(&config_path).unwrap();
        assert!(config_path.exists());

        // Load it back
        let settings = Settings::load_from_file(&config_path).unwrap();
        assert_eq!(settings.log_retention_days, 30);
        assert_eq!(settings.log_max_size_gb, 1.0);
    }

    #[test]
    fn test_cli_overrides() {
        let settings =
            Settings::load_with_cli_args(Some(60), Some(2.0), Some(14), Some(1.0), Some(8))
                .unwrap();

        assert_eq!(settings.log_retention_days, 60);
        assert_eq!(settings.log_max_size_gb, 2.0);
        assert_eq!(settings.process_retention_days, 14);
        assert_eq!(settings.process_max_size_gb, 1.0);
        assert_eq!(settings.cleanup_interval_minutes, 8);
    }

    #[test]
    fn test_cleanup_interval_clamping() {
        // Test below minimum
        let settings = Settings::load_with_cli_args(None, None, None, None, Some(3)).unwrap();
        assert_eq!(settings.cleanup_interval_minutes, 5);

        // Test above maximum
        let settings = Settings::load_with_cli_args(None, None, None, None, Some(20)).unwrap();
        assert_eq!(settings.cleanup_interval_minutes, 15);

        // Test within range
        let settings = Settings::load_with_cli_args(None, None, None, None, Some(10)).unwrap();
        assert_eq!(settings.cleanup_interval_minutes, 10);
    }
}
