//! 全局配置：`~/.cicbyte/apps/byte-git/config/config.yaml`。
//!
//! serde_yaml 版 schema，首次读取自动写入默认值。

use crate::util::AppPaths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AISettings {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    /// 提交信息语言（默认 zh）。
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub max_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubSettings {
    #[serde(default)]
    pub token: String,
}

/// 外部 OpenCodeReview（代码审查）集成配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OCRSettings {
    /// ocr 可执行文件路径；空 = 从 PATH 自动探测。
    #[serde(default)]
    pub exe_path: String,
    /// 审查并发（0 = OCR 默认 8）。
    #[serde(default)]
    pub concurrency: i64,
    /// agent 工具调用上限（0 = OCR 默认，最小 10）。
    #[serde(default)]
    pub max_tools: i64,
    /// "agent"（JSON）| "human"（进度行）。
    #[serde(default)]
    pub audience: String,
    /// 输出语言覆盖；空 = OCR 默认。
    #[serde(default)]
    pub language: String,
}

/// 外部 gitleaks（密钥扫描）集成配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitleaksSettings {
    #[serde(default)]
    pub exe_path: String,
}

/// 外部 `reference` CLI 集成配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReferenceSettings {
    #[serde(default)]
    pub exe_path: String,
}

/// 外部依赖功能开关（桌面端菜单可见性）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeatureSettings {
    #[serde(default)]
    pub github: bool,
    #[serde(default)]
    pub reference: bool,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitignoreSettings {
    /// "local" | "github" | "ai"
    #[serde(default)]
    pub default_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RewriteSettings {
    /// 改写前是否自动整库克隆备份。
    #[serde(default)]
    pub auto_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TerminalSettings {
    /// 终端 shell 路径；空 = 自动探测。
    #[serde(default)]
    pub shell_path: String,
    /// 终端字号（0 = 默认 13）。
    #[serde(default)]
    pub font_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub ai: AISettings,
    #[serde(default)]
    pub github: GitHubSettings,
    #[serde(default)]
    pub ocr: OCRSettings,
    #[serde(default)]
    pub gitleaks: GitleaksSettings,
    #[serde(default)]
    pub reference: ReferenceSettings,
    #[serde(default)]
    pub features: FeatureSettings,
    #[serde(default)]
    pub gitignore: GitignoreSettings,
    #[serde(default)]
    pub rewrite: RewriteSettings,
    #[serde(default)]
    pub terminal: TerminalSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ai: AISettings {
                model: "gemma4:e4b".into(),
                base_url: "http://localhost:11434/v1".into(),
                lang: "zh".into(),
                max_tokens: 4000,
                api_key: String::new(),
            },
            ocr: OCRSettings {
                audience: "agent".into(),
                ..Default::default()
            },
            terminal: TerminalSettings {
                font_size: 13,
                ..Default::default()
            },
            gitignore: GitignoreSettings {
                default_source: "local".into(),
            },
            github: Default::default(),
            gitleaks: Default::default(),
            reference: Default::default(),
            features: Default::default(),
            rewrite: Default::default(),
        }
    }
}

impl Settings {
    /// 读取配置；文件不存在时写入默认配置再返回。
    pub fn load() -> Result<Self> {
        let paths = AppPaths::new()?;
        let file = paths.settings_file();
        let data = match std::fs::read(&file) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let s = Settings::default();
                s.save()?;
                return Ok(s);
            }
            Err(e) => return Err(e).with_context(|| format!("读取配置失败: {}", file.display())),
        };
        // 默认值打底，文件字段覆盖（解析失败回退默认值）。
        let merged: Settings =
            serde_yaml::from_str(String::from_utf8_lossy(&data).as_ref())
                .unwrap_or_else(|_| Settings::default());
        Ok(merged)
    }

    pub fn save(&self) -> Result<()> {
        let paths = AppPaths::new()?;
        AppPaths::ensure_dir(&paths.config_dir())?;
        let data = serde_yaml::to_string(self).context("序列化配置失败")?;
        std::fs::write(paths.settings_file(), data).context("写入配置失败")?;
        Ok(())
    }

    /// 点路径读值（`config get` 用）。如 `ai.model`、`github.token`。
    pub fn get(&self, key: &str) -> Option<String> {
        let json = serde_json::to_value(self).ok()?;
        let mut cur = &json;
        for part in key.split('.') {
            cur = cur.get(part)?;
        }
        match cur {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Null => None,
            v => Some(v.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_roundtrip() {
        let s = Settings::default();
        let yaml = serde_yaml::to_string(&s).unwrap();
        let back: Settings = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.ai.model, "gemma4:e4b");
        assert_eq!(back.terminal.font_size, 13);
    }
}
