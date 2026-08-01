//! CLI 入口（设计文档 4.9）
//!
//! dq [--difficulty <easy|medium|hard>]  默认：获取/显示今日题目
//! dq answer / skip / stats / rule / profile / config

mod config;
mod db;
mod display;
mod lang;
mod llm;
mod models;
mod profile;
mod quiz;
mod summary;

use std::io::BufRead;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::db::Db;
use crate::lang::TEXTS;
use crate::models::today_str;

/// 每日一题 — 面向后端/数据方向程序员的面试知识练习
#[derive(Parser)]
#[command(name = "dq", version, about, long_about = None)]
struct Cli {
    /// 题目难度（easy/medium/hard）
    #[arg(long, value_parser = ["easy", "medium", "hard"])]
    difficulty: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 打开编辑器答题
    Answer,
    /// 跳过今日题目（删除）
    Skip,
    /// 查看掌握度统计
    Stats,
    /// 出题规则管理
    Rule {
        #[command(subcommand)]
        cmd: RuleCmd,
    },
    /// 画像管理
    Profile {
        #[command(subcommand)]
        cmd: Option<ProfileCmd>,
    },
    /// 配置管理
    Config {
        #[command(subcommand)]
        cmd: Option<ConfigCmd>,
    },
}

#[derive(Subcommand)]
enum RuleCmd {
    /// 添加出题规则
    Add { text: String },
    /// 列出所有规则
    List,
    /// 删除规则
    Remove { id: i64 },
}

#[derive(Subcommand)]
enum ProfileCmd {
    /// 交互式设置画像
    Setup,
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// 设置 API Key
    ApiKey { key: String },
    /// 设置模型（无参时交互式选择当前服务商常用模型）
    Model { name: Option<String> },
    /// 设置 API 地址（交互式选择常用服务商）
    BaseUrl { provider: Option<String> },
    /// 设置关注方向（逗号分隔）
    Focus { areas: String },
    /// 一键交互式配置 AI（服务商 + API Key，自动设置模型）
    Ai,
    /// 重置所有配置和数据
    Reset,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            let session = quiz::get_or_create_question(cli.difficulty.as_deref()).await?;
            quiz::show_question(&session);
        }
        Some(Commands::Answer) => {
            quiz::answer_interactive().await?;
        }
        Some(Commands::Skip) => {
            quiz::skip_today()?;
            println!("{}", display::dim(TEXTS.done()));
        }
        Some(Commands::Stats) => {
            quiz::show_stats()?;
        }
        Some(Commands::Rule { cmd }) => match cmd {
            RuleCmd::Add { text } => {
                let config = Config::load_raw()?;
                let db = Db::open(&config.db_path())?;
                db.add_rule(&text)?;
                println!("{}", display::good(TEXTS.rule_added()));
            }
            RuleCmd::List => {
                let config = Config::load_raw()?;
                let db = Db::open(&config.db_path())?;
                let rules = db.list_rules()?;
                if rules.is_empty() {
                    println!("{}", display::dim(TEXTS.no_rules()));
                } else {
                    println!("{}", display::title(TEXTS.rule_list_title()));
                    for (id, content) in rules {
                        println!("  {}  {}", display::highlight(&id.to_string()), content);
                    }
                }
            }
            RuleCmd::Remove { id } => {
                let config = Config::load_raw()?;
                let db = Db::open(&config.db_path())?;
                if db.delete_rule(id)? {
                    println!("{}", display::good(TEXTS.rule_deleted()));
                } else {
                    bail!("{}", TEXTS.rule_not_found());
                }
            }
        },
        Some(Commands::Profile { cmd }) => match cmd {
            None => {
                let config = Config::load_raw()?;
                println!("{}", display::title(TEXTS.profile_title()));
                match config.profile {
                    Some(p) if !p.trim().is_empty() => {
                        println!("{}", display::highlight(&p));
                    }
                    _ => println!("{}", display::dim(TEXTS.not_set())),
                }
            }
            Some(ProfileCmd::Setup) => {
                profile::run_profile_setup()?;
            }
        },
        Some(Commands::Config { cmd }) => match cmd {
            None => {
                let config = Config::load_raw()?;
                println!("{}", display::title(TEXTS.config_title()));
                println!(
                    "  {}  {}",
                    display::label(TEXTS.api_key_field()),
                    display::dim(&if config.api_key.is_empty() {
                        TEXTS.not_set().to_string()
                    } else {
                        "***".to_string()
                    })
                );
                println!(
                    "  {}  {}",
                    display::label(TEXTS.model_field()),
                    display::highlight(&config.model)
                );
                let base_url = if config.base_url.is_empty() {
                    TEXTS.not_set().to_string()
                } else {
                    config.base_url.clone()
                };
                println!(
                    "  {}  {}",
                    display::label(TEXTS.base_url_field()),
                    display::highlight(&base_url)
                );
                println!(
                    "  {}  {}",
                    display::label(TEXTS.focus_field()),
                    display::highlight(&config.focus_areas.join("、"))
                );
                println!(
                    "  {}  {}",
                    display::label(TEXTS.data_dir_field()),
                    display::dim(&config.data_dir.display().to_string())
                );
            }
            Some(ConfigCmd::ApiKey { key }) => {
                save_with_test(Some(key), None, None).await?;
                println!("{}", display::good(TEXTS.api_key_saved()));
            }
            Some(ConfigCmd::Model { name }) => match name {
                Some(n) => {
                    save_with_test(None, Some(n), None).await?;
                    println!("{}", display::good(TEXTS.model_saved()));
                }
                None => {
                    let model = select_model()?;
                    save_with_test(None, Some(model.clone()), None).await?;
                    println!(
                        "{} {}",
                        display::good(TEXTS.model_saved()),
                        display::dim(&model)
                    );
                }
            },
            Some(ConfigCmd::BaseUrl { provider }) => {
                let url = match provider {
                    Some(p) if p == "list" => {
                        print_providers();
                        return Ok(());
                    }
                    // 兼容直接传 URL（高级用法/测试）
                    Some(p) if p.starts_with("http") => p,
                    // 按服务商名设置
                    Some(p) => match config::provider_url(&p) {
                        Some((u, _)) => u.to_string(),
                        None => {
                            print_providers();
                            bail!("{}", TEXTS.provider_not_found());
                        }
                    },
                    // 交互式选择
                    None => select_provider()?.0,
                };
                save_with_test(None, None, Some(url.clone())).await?;
                println!(
                    "{} {}",
                    display::good(TEXTS.base_url_saved()),
                    display::dim(&url)
                );
            }
            Some(ConfigCmd::Focus { areas }) => {
                let list: Vec<String> = areas
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                Config::save(None, None, None, Some(list))?;
                println!("{}", display::good(TEXTS.focus_saved()));
            }
            Some(ConfigCmd::Ai) => {
                // 一键配置：选服务商 → 输 Key → 自动带出 base_url + 默认模型 → 测试 → 保存
                let (url, model) = select_provider()?;
                eprintln!("{}", display::label(TEXTS.api_key_prompt()));
                let mut line = String::new();
                std::io::stdin().lock().read_line(&mut line)?;
                let key = line.trim().to_string();
                if key.is_empty() {
                    bail!("{}", TEXTS.api_key_required());
                }
                let current = Config::load_raw()?;
                let temp = Config {
                    base_url: url.clone(),
                    model: model.clone(),
                    api_key: key.clone(),
                    ..current
                };
                eprintln!("{}", display::dim(TEXTS.testing_connection()));
                llm::test_connection(&temp)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}: {:#}", TEXTS.test_failed(), e))?;
                Config::save(Some(key), Some(model.clone()), Some(url.clone()), None)?;
                println!(
                    "{} {} {} {}",
                    display::good(TEXTS.ai_configured()),
                    display::dim(&url),
                    display::dim("|"),
                    display::dim(&model)
                );
            }
            Some(ConfigCmd::Reset) => {
                println!("{}", display::warn(TEXTS.reset_confirm()));
                let mut line = String::new();
                std::io::stdin().lock().read_line(&mut line)?;
                if matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
                    Config::reset()?;
                    println!("{}", display::good(TEXTS.reset_done()));
                } else {
                    println!("{}", display::dim(TEXTS.cancelled()));
                }
            }
        },
    }
    Ok(())
}

/// 配置校验：base_url/model/api_key 三要素齐则先测试 LLM 连通，通过才保存
async fn save_with_test(
    api_key: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
) -> Result<()> {
    let current = Config::load_raw()?;
    // 用待写入的值构造临时配置（不落盘）进行测试
    let temp = Config {
        api_key: api_key.clone().unwrap_or_else(|| current.api_key.clone()),
        model: model.clone().unwrap_or_else(|| current.model.clone()),
        base_url: base_url.clone().unwrap_or_else(|| current.base_url.clone()),
        focus_areas: current.focus_areas.clone(),
        profile: current.profile.clone(),
        lang: current.lang,
        data_dir: current.data_dir.clone(),
    };
    let ready = !temp.base_url.is_empty() && !temp.model.is_empty() && !temp.api_key.is_empty();
    if ready {
        eprintln!("{}", display::dim(TEXTS.testing_connection()));
        llm::test_connection(&temp)
            .await
            .map_err(|e| anyhow::anyhow!("{}: {:#}", TEXTS.test_failed(), e))?;
    }
    Config::save(api_key, model, base_url, None)?;
    Ok(())
}

/// 交互式选择当前服务商的常用模型
fn select_model() -> Result<String> {
    let config = Config::load_raw()?;
    if config.base_url.is_empty() {
        bail!("{}", TEXTS.model_need_provider());
    }
    let provider = config::provider_name_for_url(&config.base_url);
    let models: &[&str] = match provider {
        Some(name) => config::MODEL_PRESETS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, m)| *m)
            .unwrap_or(&[]),
        None => &[],
    };
    if models.is_empty() {
        bail!("{}", TEXTS.model_not_found());
    }
    println!("{}", display::title(TEXTS.model_list_title()));
    for (i, m) in models.iter().enumerate() {
        println!("  {}. {}", i + 1, display::highlight(m));
    }
    println!("{}", display::dim(TEXTS.enter_number()));
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let idx: usize = line
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("{}", TEXTS.invalid_input()))?;
    if (1..=models.len()).contains(&idx) {
        Ok(models[idx - 1].to_string())
    } else {
        bail!("{}", TEXTS.invalid_input())
    }
}

/// 交互式选择 API 服务商，返回 (base_url, 默认模型)
fn select_provider() -> Result<(String, String)> {
    print_providers();
    println!("{}", display::dim(TEXTS.enter_number()));
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let idx: usize = line
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("{}", TEXTS.provider_not_found()))?;
    if (1..=config::PROVIDERS.len()).contains(&idx) {
        let (_, url, model) = config::PROVIDERS[idx - 1];
        Ok((url.to_string(), model.to_string()))
    } else {
        bail!("{}", TEXTS.provider_not_found())
    }
}

/// 打印服务商预设列表（含默认模型）
fn print_providers() {
    println!("{}", display::title(TEXTS.provider_list_title()));
    for (i, (name, url, model)) in config::PROVIDERS.iter().enumerate() {
        println!(
            "  {}. {}  {}（默认模型：{}）",
            i + 1,
            display::highlight(name),
            display::dim(url),
            display::dim(model)
        );
    }
}
