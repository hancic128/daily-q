//! 后台总结（设计文档 4.5）
//!
//! 用户答完题后，用 std::thread::spawn 启动后台线程做 LLM 总结，不阻塞终端。
//! 线程内创建独立的 tokio current_thread runtime（reqwest 需要 tokio context）。
//! 失败只打印错误，不 panic 不阻塞主线程。

use std::thread;

use anyhow::Result;

use crate::config::Config;
use crate::db::Db;
use crate::lang::TEXTS;
use crate::llm;

/// 从 quiz.rs 调用：启动后台总结线程，返回 JoinHandle 供主线程等待落库
///
/// 说明：CLI 进程在 main 返回后即退出，若直接 detach，后台线程会被进程退出杀死，
/// 总结永不落库。故返回 handle，由调用方在答案展示后 join（等待总结完成）。
pub fn spawn_background_summary() -> std::thread::JoinHandle<()> {
    thread::spawn(|| {
        if let Err(e) = run_summary() {
            eprintln!("[summary] error: {e:#}");
        }
    })
}

/// 创建 tokio runtime，block_on 异步任务
fn run_summary() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(do_summary())
}

/// 查历史 → 调 LLM 总结 → 存库
async fn do_summary() -> Result<()> {
    let config = Config::load()?;
    let mut db = Db::open(&config.db_path())?;
    let history = db.get_all_answers()?;
    if history.is_empty() {
        return Ok(());
    }
    let s = llm::summarize_history(&config, &history).await?;
    let n = s.weak_topics.len();
    db.save_summary(&s)?;
    eprintln!("{}", TEXTS.summary_done(n));
    Ok(())
}
