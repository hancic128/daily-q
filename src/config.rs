//! 配置管理（设计文档 4.1）
//!
//! 配置存储在 ~/.daily-q/config.json。
//! `save`/`set_profile` 均为关联函数，用 serde_json::Value 做部分更新，
//! 只修改指定字段，不覆盖未提供的字段。

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lang::{Lang, TEXTS};

fn default_data_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".daily-q")
}

/// 常用 OpenAI 兼容服务商预设：(名字, base_url, 默认模型)
/// base_url 后缀 /v1、/v4 由 llm.rs 自动处理，避免重复
pub const PROVIDERS: &[(&str, &str, &str)] = &[
    (
        "deepseek",
        "https://api.deepseek.com/v1",
        "deepseek-v4-flash",
    ),
    (
        "siliconflow",
        "https://api.siliconflow.cn/v1",
        "Qwen/Qwen2.5-72B-Instruct",
    ),
    ("openai", "https://api.openai.com/v1", "gpt-4o-mini"),
    ("moonshot", "https://api.moonshot.cn/v1", "moonshot-v1-8k"),
    (
        "zhipu",
        "https://open.bigmodel.cn/api/paas/v4",
        "glm-4-flash",
    ),
    (
        "qwen",
        "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "qwen-turbo",
    ),
    ("ollama", "http://localhost:11434/v1", "qwen2.5"),
];

/// 按服务商名查 (base_url, 默认模型)
pub fn provider_url(name: &str) -> Option<(&'static str, &'static str)> {
    PROVIDERS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, url, model)| (*url, *model))
}

/// 各服务商常用模型预设（`dq config model` 交互式选择用）
pub const MODEL_PRESETS: &[(&str, &[&str])] = &[
    (
        "deepseek",
        &[
            "deepseek-chat",
            "deepseek-v4-flash",
            "deepseek-v4-pro",
            "deepseek-reasoner",
        ],
    ),
    (
        "siliconflow",
        &[
            "Qwen/Qwen2.5-72B-Instruct",
            "deepseek-ai/DeepSeek-V3",
            "deepseek-ai/DeepSeek-V2.5",
            "Qwen/Qwen2.5-7B-Instruct",
            "THUDM/glm-4-9b-chat",
        ],
    ),
    ("openai", &["gpt-4o-mini", "gpt-4o", "gpt-4-turbo"]),
    (
        "moonshot",
        &["moonshot-v1-8k", "moonshot-v1-32k", "moonshot-v1-128k"],
    ),
    ("zhipu", &["glm-4-flash", "glm-4-plus", "glm-4-air"]),
    (
        "qwen",
        &["qwen-turbo", "qwen-plus", "qwen-max", "qwen-turbo-latest"],
    ),
    ("ollama", &["llama3", "qwen2.5", "mistral", "phi3"]),
];

/// 按 base_url 域名匹配服务商名（容忍 /v1 等后缀差异）
pub fn provider_name_for_url(base_url: &str) -> Option<&'static str> {
    let host = host_of(base_url)?;
    PROVIDERS
        .iter()
        .find(|(_, url, _)| host_of(url) == Some(host))
        .map(|(name, _, _)| *name)
}

fn host_of(url: &str) -> Option<&str> {
    url.strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .map(|rest| rest.split('/').next().unwrap_or(""))
}

/// 全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub api_key: String,
    pub model: String,            // 默认 "deepseek-chat"
    pub base_url: String,         // 默认空，需用户配置（如 "https://api.deepseek.com"）
    pub focus_areas: Vec<String>, // 关注方向
    pub profile: Option<String>,  // 画像字符串
    pub lang: Lang,               // zh 或 en，默认 zh
    #[serde(skip)]
    pub data_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "deepseek-chat".into(),
            base_url: String::new(),
            focus_areas: vec![
                "计算机网络".into(),
                "操作系统".into(),
                "数据库".into(),
                "数据结构与算法".into(),
                "分布式系统".into(),
            ],
            profile: None,
            lang: Lang::Zh,
            data_dir: default_data_dir(),
        }
    }
}

impl Config {
    /// 读配置；API 地址 / API key 未配置时直接 bail（设计文档 6）
    pub fn load() -> Result<Config> {
        let mut cfg = Self::load_raw()?;
        if cfg.base_url.is_empty() {
            bail!("{}", TEXTS.base_url_not_found());
        }
        if cfg.api_key.is_empty() {
            bail!("{}", TEXTS.config_not_found());
        }
        cfg.data_dir = default_data_dir();
        Ok(cfg)
    }

    /// 只读配置文件，不检查 api_key；设置全局语言（任何命令启动即跟随配置语言）
    pub fn load_raw() -> Result<Config> {
        let path = config_path_static();
        let cfg = if !path.exists() {
            Config::default()
        } else {
            let text = fs::read_to_string(&path).context("read config.json")?;
            let mut cfg: Config = serde_json::from_str(&text).context("parse config.json")?;
            cfg.data_dir = default_data_dir();
            cfg
        };
        Ok(cfg)
    }

    /// 确保 ~/.daily-q/ 存在
    #[allow(dead_code)]
    pub fn ensure_data_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.data_dir).context("create data dir")?;
        Ok(())
    }

    /// 返回 db 文件路径
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("daily-q.db")
    }

    /// 返回配置路径
    #[allow(dead_code)]
    pub fn config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }

    /// 部分更新：只写提供的字段，其余保持不变
    pub fn save(
        api_key: Option<String>,
        model: Option<String>,
        base_url: Option<String>,
        focus: Option<Vec<String>>,
    ) -> Result<()> {
        let mut obj = read_obj()?;
        if let Some(k) = api_key {
            obj.insert("api_key".into(), Value::String(k));
        }
        if let Some(m) = model {
            obj.insert("model".into(), Value::String(m));
        }
        if let Some(u) = base_url {
            obj.insert("base_url".into(), Value::String(u));
        }
        if let Some(f) = focus {
            obj.insert("focus_areas".into(), serde_json::to_value(f)?);
        }
        write_obj(&obj)
    }

    /// 单独写画像
    pub fn set_profile(profile: &str) -> Result<()> {
        let mut obj = read_obj()?;
        obj.insert("profile".into(), Value::String(profile.to_string()));
        write_obj(&obj)
    }

    /// 重置：删除 ~/.daily-q/ 下全部内容（配置 + 数据），恢复到初始状态
    pub fn reset() -> Result<()> {
        let dir = default_data_dir();
        if dir.exists() {
            std::fs::remove_dir_all(&dir).context("reset data dir")?;
        }
        Ok(())
    }
}

fn config_path_static() -> PathBuf {
    default_data_dir().join("config.json")
}

fn ensure_dir() -> Result<()> {
    fs::create_dir_all(default_data_dir()).context("create data dir")?;
    Ok(())
}

/// 读取现有 JSON 对象；文件不存在或损坏时返回空对象
fn read_obj() -> Result<serde_json::Map<String, Value>> {
    ensure_dir()?;
    let path = config_path_static();
    if !path.exists() {
        return Ok(serde_json::Map::new());
    }
    let text = fs::read_to_string(&path).context("read config.json")?;
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Ok(serde_json::Map::new()),
    }
}

fn write_obj(obj: &serde_json::Map<String, Value>) -> Result<()> {
    ensure_dir()?;
    let path = config_path_static();
    let pretty = serde_json::to_string_pretty(&Value::Object(obj.clone()))?;
    fs::write(&path, pretty).context("write config.json")?;
    Ok(())
}
