//! 业务逻辑（设计文档 4.4）
//!
//! 出题、答题、统计核心流程。全部使用中文。

use std::io::Write;

use anyhow::{Context, Result, bail};

use crate::config::Config;
use crate::db::Db;
use crate::display;
use crate::lang::TEXTS;
use crate::llm;
use crate::models::{Answer, QuizSession};
use crate::summary;
use crate::today_str;

/// 获取或创建今日题目（无题则调 LLM 生成）
pub async fn get_or_create_question(difficulty: Option<&str>) -> Result<QuizSession> {
    let config = Config::load()?;
    check_profile(&config)?;
    let db = Db::open(&config.db_path())?;
    let today = today_str();

    if let Some(q) = db.get_today_question(&today)? {
        if db.has_answered(q.id)? {
            // 已答：直接返回含答案
            let answer = db.get_answer_for_question(q.id)?;
            return Ok(QuizSession {
                question: q,
                answered: true,
                answer,
            });
        }
        if difficulty.is_some() {
            // 未答且指定了难度 → 删除重生成
            db.delete_question(q.id)?;
        } else {
            return Ok(QuizSession {
                question: q,
                answered: false,
                answer: None,
            });
        }
    }

    // 生成新题
    let weak = db.get_weak_topics()?;
    let rules = db.get_rules_text()?;
    let resp = llm::generate_question(&config, &weak, &rules, difficulty).await?;
    let question = db.insert_question(&resp, &today, "zh")?;
    Ok(QuizSession {
        question,
        answered: false,
        answer: None,
    })
}

/// 跳过今日题目（删除今日 question）
pub fn skip_today() -> Result<()> {
    let config = Config::load_raw()?;
    let db = Db::open(&config.db_path())?;
    if let Some(q) = db.get_today_question(&today_str())? {
        db.delete_question(q.id)?;
    }
    Ok(())
}

/// 答题：打开编辑器让用户写回答 → 调 LLM 评判 → 存库
pub async fn answer_interactive() -> Result<()> {
    let config = Config::load()?;
    let mut db = Db::open(&config.db_path())?;
    let today = today_str();
    let q = db
        .get_today_question(&today)?
        .context(TEXTS.no_question_today())?;
    if db.has_answered(q.id)? {
        bail!("{}", TEXTS.already_answered());
    }

    eprintln!(
        "{}",
        display::dim(&format!(
            "{} ({}: {})",
            TEXTS.answer_hint(),
            TEXTS.difficulty_label(),
            q.difficulty
        ))
    );
    let answer = edit_answer(&q)?;

    let judge = llm::judge_answer(&config, &q, &answer).await?;
    db.insert_answer(q.id, &answer, &judge)?;

    println!(
        "{} {}",
        display::label(TEXTS.score_label()),
        display::highlight(&judge.score.to_string())
    );
    println!(
        "{} {}",
        display::label(TEXTS.feedback_label()),
        judge.feedback
    );
    println!(
        "{} {}",
        display::label(TEXTS.reference_label()),
        q.reference_answer
    );

    // 等待后台总结落库（否则进程退出线程被杀死）
    summary::spawn_background_summary().join().ok();
    Ok(())
}

/// 答题：直接传字符串（未暴露到 CLI，内部使用）
#[allow(dead_code)]
pub async fn submit_answer(answer: &str) -> Result<Answer> {
    let config = Config::load()?;
    let mut db = Db::open(&config.db_path())?;
    let today = today_str();
    let q = db
        .get_today_question(&today)?
        .context(TEXTS.no_question_today())?;
    if db.has_answered(q.id)? {
        bail!("{}", TEXTS.already_answered());
    }
    let judge = llm::judge_answer(&config, &q, answer).await?;
    db.insert_answer(q.id, answer, &judge)?;
    let a = db
        .get_answer_for_question(q.id)?
        .context("answer missing after insert")?;
    summary::spawn_background_summary().join().ok();
    Ok(a)
}

/// 展示掌握度统计
pub fn show_stats() -> Result<()> {
    let config = Config::load_raw()?;
    let db = Db::open(&config.db_path())?;
    println!("{}", display::title(TEXTS.stats_title()));

    let topics = db.get_weak_topics()?;
    if topics.is_empty() {
        println!("{}", display::dim(TEXTS.no_records()));
    } else {
        for t in topics {
            let (icon, level) = match t.mastery_level() {
                "strong" => ("+", TEXTS.level_strong()),
                "medium" => ("~", TEXTS.level_medium()),
                "weak" => ("!", TEXTS.level_weak()),
                _ => ("-", TEXTS.level_none()),
            };
            let pct = if t.total == 0 {
                0
            } else {
                (t.correct as f64 / t.total as f64 * 100.0).round() as i32
            };
            let line = format!(
                "[{}] {}  {}/{} ({:>3}%) [{}]",
                icon, t.topic, t.correct, t.total, pct, level
            );
            match t.mastery_level() {
                "strong" => println!("{}", display::good(&line)),
                "medium" => println!("{}", display::warn(&line)),
                "weak" => println!("{}", display::bad(&line)),
                _ => println!("{}", display::dim(&line)),
            }
        }
    }

    if let Some(s) = db.get_latest_summary()? {
        println!();
        println!("{}", display::title(TEXTS.summary_title()));
        if !s.weak_topics.is_empty() {
            println!(
                "{} {}",
                display::label(TEXTS.weak_topics_label()),
                s.weak_topics.join("、")
            );
        }
        if !s.related_weaknesses.is_empty() {
            println!("{}", display::label(TEXTS.relations_label()));
            for r in &s.related_weaknesses {
                println!("  {} ↔ {}: {}", r.topic_a, r.topic_b, r.reason);
            }
        }
        if !s.suggestions.is_empty() {
            println!("{}", display::label(TEXTS.suggestions_label()));
            for sug in &s.suggestions {
                println!("  - {sug}");
            }
        }
    }
    Ok(())
}

/// 渲染一道题（带答案/不带答案）
pub fn show_question(session: &QuizSession) {
    let q = &session.question;
    println!(
        "{} {} {}",
        display::title(TEXTS.today_question()),
        display::highlight(&q.topic),
        display::dim(&q.difficulty)
    );
    println!("{}", q.content);
    if session.answered
        && let Some(a) = &session.answer
    {
        println!();
        println!(
            "{} {}",
            display::label(TEXTS.score_label()),
            display::highlight(&a.score.to_string())
        );
        println!("{} {}", display::label(TEXTS.feedback_label()), a.feedback);
    }
}

/// 检查画像是否已设置（设计文档 6：check_ready）
fn check_profile(config: &Config) -> Result<()> {
    match config.profile.as_deref() {
        Some(p) if !p.trim().is_empty() => Ok(()),
        _ => bail!("{}", TEXTS.profile_required()),
    }
}

/// 打开 $EDITOR（默认 vim）编辑答案
fn edit_answer(q: &crate::models::Question) -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let mut tmp = tempfile::NamedTempFile::new().context("创建临时文件失败")?;
    let header = format!("# {}: {}\n# {}\n\n", q.topic, q.difficulty, q.content);
    tmp.write_all(header.as_bytes()).context("写入模板失败")?;
    tmp.flush().context("flush 临时文件失败")?;

    let status = std::process::Command::new(&editor)
        .arg(tmp.path())
        .status()
        .context("启动编辑器失败")?;
    if !status.success() {
        bail!("{}", TEXTS.editor_failed());
    }

    let content = std::fs::read_to_string(tmp.path()).context("读取回答文件失败")?;
    let answer: String = content
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if answer.is_empty() {
        bail!("{}", TEXTS.empty_answer());
    }
    Ok(answer)
}
