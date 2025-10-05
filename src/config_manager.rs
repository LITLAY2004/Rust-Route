use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

use log::warn;

use crate::auth::AuthConfig;
use crate::ipv6::RipV6Config;
use crate::web::WebConfig;

const DEFAULT_HISTORY_LIMIT: usize = 20;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    pub router_id: String,
    pub interfaces: Vec<InterfaceConfig>,
    pub rip: RipConfig,
    pub ripv6: RipV6Config,
    pub web: WebConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
    pub metrics: MetricsConfig,
    pub backup: BackupConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    pub address: String,
    pub enabled: bool,
    pub cost: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipConfig {
    pub enabled: bool,
    pub port: u16,
    pub update_interval: u64,
    pub garbage_collection_timeout: u64,
    pub infinity_metric: u32,
    pub split_horizon: bool,
    pub poison_reverse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: Option<String>,
    pub max_file_size: u64,
    pub max_files: u32,
    pub console_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub collection_interval: u64,
    pub retention_days: u32,
    pub export_prometheus: bool,
    pub prometheus_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub enabled: bool,
    pub interval_hours: u64,
    pub max_backups: u32,
    pub backup_directory: String,
    pub include_routing_table: bool,
    pub compress: bool,
}

#[derive(Debug, Clone)]
struct ConfigSnapshot {
    version: u32,
    timestamp: DateTime<Utc>,
    config: RouterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigHistoryEntry {
    pub version: u32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiff {
    pub version: u32,
    pub timestamp: DateTime<Utc>,
    pub previous: String,
    pub current: String,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            router_id: "192.168.1.1".to_string(),
            interfaces: vec![InterfaceConfig {
                name: "eth0".to_string(),
                address: "192.168.1.1/24".to_string(),
                enabled: true,
                cost: 1,
            }],
            rip: RipConfig {
                enabled: true,
                port: 520,
                update_interval: 30,
                garbage_collection_timeout: 120,
                infinity_metric: 16,
                split_horizon: true,
                poison_reverse: false,
            },
            ripv6: RipV6Config::default(),
            web: WebConfig::default(),
            auth: AuthConfig::default(),
            logging: LoggingConfig {
                level: "info".to_string(),
                file_path: Some("/var/log/rust-route.log".to_string()),
                max_file_size: 10 * 1024 * 1024, // 10MB
                max_files: 5,
                console_output: true,
            },
            metrics: MetricsConfig {
                enabled: true,
                collection_interval: 60,
                retention_days: 30,
                export_prometheus: false,
                prometheus_port: 9090,
            },
            backup: BackupConfig {
                enabled: true,
                interval_hours: 24,
                max_backups: 7,
                backup_directory: "/var/backups/rust-route".to_string(),
                include_routing_table: true,
                compress: true,
            },
        }
    }
}

/// Configuration validation result
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.valid = false;
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    pub fn is_valid(&self) -> bool {
        self.valid && self.errors.is_empty()
    }
}

/// Configuration backup metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub size_bytes: u64,
    pub checksum: String,
    pub description: String,
    pub config_version: u32,
}

/// Configuration manager with hot-reload support
pub struct ConfigManager {
    config_path: PathBuf,
    current_config: Arc<RwLock<RouterConfig>>,
    config_version: Arc<RwLock<u32>>,
    change_sender: watch::Sender<RouterConfig>,
    history: Arc<RwLock<VecDeque<ConfigSnapshot>>>,
    history_limit: usize,
    _watcher: RecommendedWatcher,
}

impl ConfigManager {
    pub async fn new(
        config_path: impl AsRef<Path>,
    ) -> Result<(Self, watch::Receiver<RouterConfig>)> {
        let config_path = config_path.as_ref().to_path_buf();

        // Load initial configuration
        let config = Self::load_config(&config_path).await.unwrap_or_else(|_| {
            log::warn!("Failed to load config, using defaults");
            RouterConfig::default()
        });

        let current_config = Arc::new(RwLock::new(config.clone()));
        let config_version = Arc::new(RwLock::new(1));
        let history = Arc::new(RwLock::new(VecDeque::new()));
        {
            let mut history_guard = history.write().await;
            history_guard.push_back(ConfigSnapshot {
                version: 1,
                timestamp: Utc::now(),
                config: config.clone(),
            });
        }
        let (change_sender, change_receiver) = watch::channel(config.clone());

        // Setup file watcher for hot-reload
        let watcher = Self::setup_file_watcher(
            &config_path,
            current_config.clone(),
            config_version.clone(),
            change_sender.clone(),
            history.clone(),
            DEFAULT_HISTORY_LIMIT,
        )?;

        let manager = Self {
            config_path,
            current_config,
            config_version,
            change_sender,
            history,
            history_limit: DEFAULT_HISTORY_LIMIT,
            _watcher: watcher,
        };

        Ok((manager, change_receiver))
    }

    async fn load_config(path: &Path) -> Result<RouterConfig> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read config file")?;

        let config: RouterConfig =
            serde_json::from_str(&content).context("Failed to parse config JSON")?;

        Ok(config)
    }

    fn setup_file_watcher(
        config_path: &Path,
        current_config: Arc<RwLock<RouterConfig>>,
        config_version: Arc<RwLock<u32>>,
        change_sender: watch::Sender<RouterConfig>,
        history: Arc<RwLock<VecDeque<ConfigSnapshot>>>,
        history_limit: usize,
    ) -> Result<RecommendedWatcher> {
        let config_path = config_path.to_path_buf();

        let watch_path = config_path.clone();
        let runtime = tokio::runtime::Handle::current();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            let config_path = config_path.clone();
            let current_config = current_config.clone();
            let config_version = config_version.clone();
            let change_sender = change_sender.clone();
            let runtime = runtime.clone();
            let history = history.clone();

            runtime.spawn(async move {
                match res {
                    Ok(event) => {
                        if matches!(event.kind, EventKind::Modify(_)) {
                            log::info!("🔄 Configuration file changed, reloading...");

                            match Self::load_config(&config_path).await {
                                Ok(new_config) => {
                                    // Validate new configuration
                                    let validation = Self::validate_config(&new_config);
                                    if !validation.is_valid() {
                                        log::error!("❌ Invalid configuration detected:");
                                        for error in &validation.errors {
                                            log::error!("  - {}", error);
                                        }
                                        return;
                                    }

                                    // Show warnings if any
                                    for warning in &validation.warnings {
                                        log::warn!("⚠️  {}", warning);
                                    }

                                    // Update configuration
                                    {
                                        let mut config = current_config.write().await;
                                        *config = new_config.clone();
                                    }

                                    // Increment version
                                    {
                                        let mut version = config_version.write().await;
                                        *version += 1;
                                    }

                                    // Notify subscribers
                                    if let Err(e) = change_sender.send(new_config.clone()) {
                                        log::error!("Failed to notify config change: {}", e);
                                    } else {
                                        log::info!("✅ Configuration reloaded successfully");
                                        let current_version = *config_version.read().await;
                                        Self::record_snapshot(
                                            &history,
                                            history_limit,
                                            current_version,
                                            new_config,
                                        )
                                        .await;
                                    }
                                }
                                Err(e) => {
                                    log::error!("❌ Failed to reload configuration: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("File watcher error: {}", e);
                    }
                }
            });
        })?;

        watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;
        Ok(watcher)
    }

    pub async fn get_config(&self) -> RouterConfig {
        self.current_config.read().await.clone()
    }

    pub async fn update_config(&self, new_config: RouterConfig) -> Result<()> {
        // Validate configuration
        let validation = Self::validate_config(&new_config);
        if !validation.is_valid() {
            return Err(anyhow::anyhow!(
                "Configuration validation failed: {:?}",
                validation.errors
            ));
        }

        // Save to file
        let json =
            serde_json::to_string_pretty(&new_config).context("Failed to serialize config")?;

        tokio::fs::write(&self.config_path, json)
            .await
            .context("Failed to write config file")?;

        // Update in-memory config
        {
            let mut config = self.current_config.write().await;
            *config = new_config.clone();
        }

        // Increment version
        {
            let mut version = self.config_version.write().await;
            *version += 1;
        }

        // Notify subscribers
        if let Err(e) = self.change_sender.send(new_config.clone()) {
            warn!("Config change notification dropped: {}", e);
        }

        log::info!("✅ Configuration updated successfully");

        let current_version = *self.config_version.read().await;
        Self::record_snapshot(
            &self.history,
            self.history_limit,
            current_version,
            new_config,
        )
        .await;
        Ok(())
    }

    pub async fn list_history(&self) -> Vec<ConfigHistoryEntry> {
        let history = self.history.read().await;
        history
            .iter()
            .rev()
            .map(|snapshot| ConfigHistoryEntry {
                version: snapshot.version,
                timestamp: snapshot.timestamp,
            })
            .collect()
    }

    pub async fn diff(&self, version: u32) -> Result<ConfigDiff> {
        let snapshot = {
            let history = self.history.read().await;
            history
                .iter()
                .find(|entry| entry.version == version)
                .cloned()
        }
        .ok_or_else(|| anyhow::anyhow!("Snapshot not found"))?;

        let current = self.get_config().await;

        Ok(ConfigDiff {
            version: snapshot.version,
            timestamp: snapshot.timestamp,
            previous: serde_json::to_string_pretty(&snapshot.config)
                .context("Failed to serialize snapshot config")?,
            current: serde_json::to_string_pretty(&current)
                .context("Failed to serialize current config")?,
        })
    }

    pub async fn rollback_to(&self, version: u32) -> Result<()> {
        let snapshot = {
            let history = self.history.read().await;
            history
                .iter()
                .find(|entry| entry.version == version)
                .cloned()
        }
        .ok_or_else(|| anyhow::anyhow!("Snapshot not found"))?;

        self.update_config(snapshot.config).await
    }

    async fn record_snapshot(
        history: &Arc<RwLock<VecDeque<ConfigSnapshot>>>,
        limit: usize,
        version: u32,
        config: RouterConfig,
    ) {
        let mut guard = history.write().await;
        guard.retain(|snapshot| snapshot.version != version);
        guard.push_back(ConfigSnapshot {
            version,
            timestamp: Utc::now(),
            config,
        });
        while guard.len() > limit {
            guard.pop_front();
        }
    }

    pub fn validate_config(config: &RouterConfig) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Validate router ID
        if config.router_id.is_empty() {
            result.add_error("Router ID cannot be empty".to_string());
        } else if config.router_id.parse::<std::net::IpAddr>().is_err() {
            result.add_warning("Router ID should be a valid IP address".to_string());
        }

        // Validate interfaces
        if config.interfaces.is_empty() {
            result.add_error("At least one interface must be configured".to_string());
        }

        for interface in &config.interfaces {
            if interface.name.is_empty() {
                result.add_error("Interface name cannot be empty".to_string());
            }

            if interface.address.parse::<ipnet::IpNet>().is_err() {
                result.add_error(format!("Invalid interface address: {}", interface.address));
            }

            if interface.cost == 0 {
                result.add_warning(format!(
                    "Interface {} has cost 0, which may cause issues",
                    interface.name
                ));
            }
        }

        // Validate RIP configuration
        if config.rip.enabled {
            if config.rip.port == 0 {
                result.add_error("RIP port cannot be 0".to_string());
            }

            if config.rip.update_interval == 0 {
                result.add_error("RIP update interval cannot be 0".to_string());
            }

            if config.rip.infinity_metric == 0 {
                result.add_error("RIP infinity metric cannot be 0".to_string());
            }

            if config.rip.infinity_metric > 16 {
                result.add_warning("RIP infinity metric > 16 is non-standard".to_string());
            }
        }

        // Validate web configuration
        if config.web.enabled {
            if config.web.port == 0 {
                result.add_error("Web interface port cannot be 0".to_string());
            }

            if config.web.bind_address.is_empty() {
                result.add_error("Web interface bind address cannot be empty".to_string());
            }
        }

        // Validate authentication
        if config.auth.enabled {
            if config.auth.jwt_secret.len() < 32 {
                result.add_warning("JWT secret should be at least 32 characters long".to_string());
            }

            if config.auth.token_expiry_hours == 0 {
                result.add_error("Token expiry cannot be 0".to_string());
            }
        }

        if config.web.auth_enabled != config.auth.enabled {
            result.add_warning(
                "web.auth_enabled and auth.enabled differ; authentication only activates when both are true"
                    .to_string(),
            );
        }

        // Validate logging
        if config.logging.level.is_empty() {
            result.add_error("Log level cannot be empty".to_string());
        } else {
            let valid_levels = ["error", "warn", "info", "debug", "trace"];
            if !valid_levels.contains(&config.logging.level.as_str()) {
                result.add_error(format!("Invalid log level: {}", config.logging.level));
            }
        }

        // Validate backup configuration
        if config.backup.enabled {
            if config.backup.backup_directory.is_empty() {
                result.add_error("Backup directory cannot be empty".to_string());
            }

            if config.backup.max_backups == 0 {
                result.add_warning(
                    "Max backups is 0, backups will be deleted immediately".to_string(),
                );
            }
        }

        result
    }

    pub async fn create_backup(&self, description: String) -> Result<PathBuf> {
        let config = self.get_config().await;

        if !config.backup.enabled {
            return Err(anyhow::anyhow!("Backup is disabled"));
        }

        let backup_dir = Path::new(&config.backup.backup_directory);
        tokio::fs::create_dir_all(backup_dir)
            .await
            .context("Failed to create backup directory")?;

        let timestamp = Utc::now();
        let backup_filename = format!(
            "rust-route-backup-{}.json",
            timestamp.format("%Y%m%d-%H%M%S")
        );
        let backup_path = backup_dir.join(&backup_filename);

        // Create backup content
        let backup_content = serde_json::to_string_pretty(&config)?;

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        backup_content.hash(&mut hasher);
        let checksum_hex = format!("{:x}", hasher.finish());

        // Write backup file
        if config.backup.compress {
            // In a real implementation, we'd compress the content
            tokio::fs::write(&backup_path, backup_content.as_bytes()).await?;
        } else {
            tokio::fs::write(&backup_path, backup_content.as_bytes()).await?;
        }

        // Create metadata
        let metadata = BackupMetadata {
            timestamp,
            version: env!("CARGO_PKG_VERSION").to_string(),
            size_bytes: backup_content.len() as u64,
            checksum: checksum_hex,
            description,
            config_version: *self.config_version.read().await,
        };

        let metadata_path = backup_dir.join(format!("{}.meta", backup_filename));
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        tokio::fs::write(metadata_path, metadata_json).await?;

        // Cleanup old backups
        self.cleanup_old_backups(&config.backup).await?;

        log::info!("✅ Backup created: {}", backup_path.display());
        Ok(backup_path)
    }

    pub async fn restore_backup(&self, backup_path: impl AsRef<Path>) -> Result<()> {
        let backup_path = backup_path.as_ref();

        if !backup_path.exists() {
            return Err(anyhow::anyhow!("Backup file does not exist"));
        }

        let content = tokio::fs::read_to_string(backup_path)
            .await
            .context("Failed to read backup file")?;

        let config: RouterConfig =
            serde_json::from_str(&content).context("Failed to parse backup configuration")?;

        // Validate the restored configuration
        let validation = Self::validate_config(&config);
        if !validation.is_valid() {
            return Err(anyhow::anyhow!(
                "Backup contains invalid configuration: {:?}",
                validation.errors
            ));
        }

        // Apply the configuration
        self.update_config(config).await?;

        log::info!(
            "✅ Configuration restored from backup: {}",
            backup_path.display()
        );
        Ok(())
    }

    pub async fn list_backups(&self) -> Result<Vec<(PathBuf, BackupMetadata)>> {
        let config = self.get_config().await;
        let backup_dir = Path::new(&config.backup.backup_directory);

        if !backup_dir.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();
        let mut dir = tokio::fs::read_dir(backup_dir).await?;

        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "meta" {
                    let metadata_content = tokio::fs::read_to_string(&path).await?;
                    if let Ok(metadata) = serde_json::from_str::<BackupMetadata>(&metadata_content)
                    {
                        let backup_path = path.with_extension("json");
                        if backup_path.exists() {
                            backups.push((backup_path, metadata));
                        }
                    }
                }
            }
        }

        // Sort by timestamp, newest first
        backups.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));

        Ok(backups)
    }

    async fn cleanup_old_backups(&self, backup_config: &BackupConfig) -> Result<()> {
        let backups = self.list_backups().await?;

        if backups.len() > backup_config.max_backups as usize {
            let to_delete = &backups[backup_config.max_backups as usize..];

            for (backup_path, _) in to_delete {
                tokio::fs::remove_file(backup_path).await?;
                let meta_path = backup_path.with_extension("meta");
                if meta_path.exists() {
                    tokio::fs::remove_file(meta_path).await?;
                }
                log::info!("🗑️  Deleted old backup: {}", backup_path.display());
            }
        }

        Ok(())
    }

    pub async fn get_config_version(&self) -> u32 {
        *self.config_version.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_validation() {
        let config = RouterConfig::default();
        let result = ConfigManager::validate_config(&config);
        assert!(result.is_valid());
    }

    #[test]
    fn test_invalid_config_validation() {
        let mut config = RouterConfig::default();
        config.router_id = "".to_string();
        config.interfaces.clear();

        let result = ConfigManager::validate_config(&config);
        assert!(!result.is_valid());
        assert!(result.errors.len() >= 2);
    }

    #[tokio::test]
    async fn test_config_backup_restore() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let backup_dir = temp_dir.path().join("backups");

        let mut config = RouterConfig::default();
        config.backup.backup_directory = backup_dir.to_string_lossy().to_string();

        // Save initial config
        let config_json = serde_json::to_string_pretty(&config).unwrap();
        tokio::fs::write(&config_path, config_json).await.unwrap();

        let (manager, _) = ConfigManager::new(&config_path).await.unwrap();

        // Create backup
        let backup_path = manager
            .create_backup("Test backup".to_string())
            .await
            .unwrap();
        assert!(backup_path.exists());

        // Modify config
        let mut new_config = config.clone();
        new_config.router_id = "192.168.2.1".to_string();
        manager.update_config(new_config).await.unwrap();

        // Restore backup
        manager.restore_backup(&backup_path).await.unwrap();

        let restored_config = manager.get_config().await;
        assert_eq!(restored_config.router_id, config.router_id);
    }
}
