//! 配置加载器
//!
//! 从 `.aether/config.toml` 加载配置，支持默认值生成和配置验证。

use super::types::AetherConfig;
use crate::utils::{AetherError, Result};
use std::path::{Path, PathBuf};

/// 配置文件名
const CONFIG_DIR: &str = ".aether";
const CONFIG_FILE: &str = "config.toml";

/// 配置管理器
pub struct ConfigLoader {
    repo_path: PathBuf,
}

impl ConfigLoader {
    /// 创建配置加载器
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    /// 获取配置文件路径
    pub fn config_path(&self) -> PathBuf {
        self.repo_path.join(CONFIG_DIR).join(CONFIG_FILE)
    }

    /// 获取配置目录路径
    pub fn config_dir(&self) -> PathBuf {
        self.repo_path.join(CONFIG_DIR)
    }

    /// 加载配置，如果配置文件不存在则返回默认配置
    pub fn load(&self) -> Result<AetherConfig> {
        let path = self.config_path();
        if path.exists() {
            self.load_from_file(&path)
        } else {
            Ok(AetherConfig::default())
        }
    }

    /// 从文件加载配置
    fn load_from_file(&self, path: &Path) -> Result<AetherConfig> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            AetherError::Config(format!("无法读取配置文件 {}: {}", path.display(), e))
        })?;

        let config: AetherConfig = toml::from_str(&content).map_err(|e| {
            AetherError::Config(format!("配置文件解析失败: {}", e))
        })?;

        // 展开环境变量（如 ${AETHER_OPENAI_KEY}）
        let config = self.expand_env_vars(config);

        Ok(config)
    }

    /// 展开配置中的环境变量引用
    fn expand_env_vars(&self, mut config: AetherConfig) -> AetherConfig {
        if config.llm.api_key.starts_with("${") && config.llm.api_key.ends_with('}') {
            let var_name = &config.llm.api_key[2..config.llm.api_key.len() - 1];
            if let Ok(val) = std::env::var(var_name) {
                config.llm.api_key = val;
            }
        }
        config
    }

    /// 保存配置到文件
    pub fn save(&self, config: &AetherConfig) -> Result<()> {
        let dir = self.config_dir();
        std::fs::create_dir_all(&dir).map_err(|e| {
            AetherError::Config(format!("无法创建配置目录 {}: {}", dir.display(), e))
        })?;

        let content = toml::to_string_pretty(config).map_err(|e| {
            AetherError::Config(format!("配置序列化失败: {}", e))
        })?;

        let path = self.config_path();
        std::fs::write(&path, content).map_err(|e| {
            AetherError::Config(format!("无法写入配置文件 {}: {}", path.display(), e))
        })?;

        Ok(())
    }

    /// 初始化配置文件（生成默认配置）
    pub fn init(&self) -> Result<AetherConfig> {
        let path = self.config_path();
        if path.exists() {
            return Err(AetherError::Config(format!(
                "配置文件已存在: {}",
                path.display()
            )));
        }

        let config = AetherConfig::default();
        self.save(&config)?;
        Ok(config)
    }

    /// 验证配置的合法性
    pub fn validate(&self, config: &AetherConfig) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // 验证门控配置
        if config.gate.enabled {
            if config.gate.thresholds.max_files_changed == 0 {
                warnings.push("gate.thresholds.max_files_changed 为 0，这意味着任何文件变更都会触发门控".into());
            }
            if config.gate.thresholds.max_lines_added == 0 {
                warnings.push("gate.thresholds.max_lines_added 为 0，这意味着任何新增行都会触发门控".into());
            }
        }

        // 验证 LLM 配置
        if !config.llm.api_key.is_empty() && !config.llm.api_key.starts_with("sk-") && !config.llm.api_key.starts_with("ant-") {
            warnings.push("llm.api_key 格式可能不正确（期望 sk- 或 ant- 前缀）".into());
        }

        if config.llm.temperature < 0.0 || config.llm.temperature > 2.0 {
            return Err(AetherError::Config(format!(
                "llm.temperature 必须在 0.0~2.0 之间，当前值: {}",
                config.llm.temperature
            )));
        }

        // 验证验证配置
        if config.verify.enabled {
            if !config.verify.basic.compile_check
                && !config.verify.basic.lint_check
                && !config.verify.testing.run_unit_tests
            {
                warnings.push("验证已启用但所有检查都被禁用，建议至少启用一项检查".into());
            }
        }

        Ok(warnings)
    }

    /// 更新单个配置项（简单的 key=value 设置）
    pub fn set_value(&self, key: &str, value: &str) -> Result<()> {
        let mut config = self.load()?;

        match key {
            "gate.enabled" => config.gate.enabled = value.parse().map_err(|_| AetherError::Config(format!("无法解析布尔值: {}", value)))?,
            "llm.provider" => config.llm.provider = value.to_string(),
            "llm.model" => config.llm.model = value.to_string(),
            "llm.api_key" => config.llm.api_key = value.to_string(),
            "llm.temperature" => config.llm.temperature = value.parse().map_err(|_| AetherError::Config(format!("无法解析浮点数: {}", value)))?,
            "verify.enabled" => config.verify.enabled = value.parse().map_err(|_| AetherError::Config(format!("无法解析布尔值: {}", value)))?,
            "gate.thresholds.max_files_changed" => config.gate.thresholds.max_files_changed = value.parse().map_err(|_| AetherError::Config(format!("无法解析整数: {}", value)))?,
            "gate.thresholds.max_lines_added" => config.gate.thresholds.max_lines_added = value.parse().map_err(|_| AetherError::Config(format!("无法解析整数: {}", value)))?,
            "gate.thresholds.max_lines_deleted" => config.gate.thresholds.max_lines_deleted = value.parse().map_err(|_| AetherError::Config(format!("无法解析整数: {}", value)))?,
            "gate.thresholds.max_commits_per_hour" => config.gate.thresholds.max_commits_per_hour = value.parse().map_err(|_| AetherError::Config(format!("无法解析整数: {}", value)))?,
            _ => return Err(AetherError::Config(format!("未知配置项: {}。支持的配置项: gate.enabled, llm.provider, llm.model, llm.api_key, llm.temperature, verify.enabled, gate.thresholds.max_files_changed, gate.thresholds.max_lines_added, gate.thresholds.max_lines_deleted, gate.thresholds.max_commits_per_hour", key))),
        }

        self.save(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, ConfigLoader) {
        let dir = tempfile::tempdir().unwrap();
        let loader = ConfigLoader::new(dir.path());
        (dir, loader)
    }

    #[test]
    fn test_default_config() {
        let (_dir, loader) = setup_test_env();
        let config = loader.load().unwrap();
        assert!(config.gate.enabled);
        assert!(config.verify.enabled);
        assert_eq!(config.llm.provider, "openai");
    }

    #[test]
    fn test_init_config() {
        let (_dir, loader) = setup_test_env();
        let config = loader.init().unwrap();
        assert!(config.gate.enabled);

        // 再次初始化应该失败
        assert!(loader.init().is_err());
    }

    #[test]
    fn test_save_and_load() {
        let (_dir, loader) = setup_test_env();
        let mut config = AetherConfig::default();
        config.gate.enabled = false;
        loader.save(&config).unwrap();

        let loaded = loader.load().unwrap();
        assert!(!loaded.gate.enabled);
    }

    #[test]
    fn test_validate() {
        let (_dir, loader) = setup_test_env();
        let mut config = AetherConfig::default();
        let warnings = loader.validate(&config).unwrap();
        assert!(warnings.is_empty());

        // 无效 temperature
        config.llm.temperature = 5.0;
        assert!(loader.validate(&config).is_err());
    }

    #[test]
    fn test_set_value() {
        let (_dir, loader) = setup_test_env();
        loader.init().unwrap();

        loader.set_value("gate.enabled", "false").unwrap();
        let config = loader.load().unwrap();
        assert!(!config.gate.enabled);

        loader.set_value("llm.model", "gpt-3.5-turbo").unwrap();
        let config = loader.load().unwrap();
        assert_eq!(config.llm.model, "gpt-3.5-turbo");
    }
}
