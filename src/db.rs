//! 数据层（设计文档 4.2）
//!
//! 封装全部 SQLite 操作，5 张表：questions / answers / topic_mastery / summary_cache / rules。
//! 关键逻辑：insert_answer 双路更新 topic_mastery（judge.topic_tags）；
//! save_summary 也按 weak_topics 更新（标记为答错）。

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::models::{
    Answer, LlmJudgeResponse, LlmQuizResponse, LlmSummaryResponse, Question, TopicMastery,
};

/// 5 张表的建表语句
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS questions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL UNIQUE,
    topic TEXT NOT NULL,
    difficulty TEXT NOT NULL,
    content TEXT NOT NULL,
    reference_answer TEXT NOT NULL,
    lang TEXT NOT NULL DEFAULT 'zh'
);

CREATE TABLE IF NOT EXISTS answers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question_id INTEGER NOT NULL REFERENCES questions(id),
    content TEXT NOT NULL,
    score INTEGER NOT NULL,
    feedback TEXT NOT NULL,
    topic_tags TEXT NOT NULL DEFAULT '[]',
    answered_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS topic_mastery (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    total INTEGER NOT NULL DEFAULT 0,
    correct INTEGER NOT NULL DEFAULT 0,
    last_practiced TEXT
);

CREATE TABLE IF NOT EXISTS summary_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    weak_topics TEXT NOT NULL DEFAULT '[]',
    relations TEXT NOT NULL DEFAULT '[]',
    suggestions TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);
"#;

/// SQLite 连接封装
pub struct Db {
    conn: Connection,
}

fn row_to_question(row: &rusqlite::Row) -> rusqlite::Result<Question> {
    Ok(Question {
        id: row.get(0)?,
        date: row.get(1)?,
        topic: row.get(2)?,
        difficulty: row.get(3)?,
        content: row.get(4)?,
        reference_answer: row.get(5)?,
        lang: row.get(6)?,
    })
}

fn row_to_answer(row: &rusqlite::Row) -> rusqlite::Result<Answer> {
    Ok(Answer {
        id: row.get(0)?,
        question_id: row.get(1)?,
        content: row.get(2)?,
        score: row.get(3)?,
        feedback: row.get(4)?,
        answered_at: row.get(5)?,
    })
}

impl Db {
    /// 打开连接，执行建表语句
    pub fn open(path: &std::path::Path) -> Result<Db> {
        let conn = Connection::open(path).context("open sqlite")?;
        conn.execute_batch(SCHEMA).context("init schema")?;
        Ok(Db { conn })
    }

    /// 按日期查题
    pub fn get_today_question(&self, date: &str) -> Result<Option<Question>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, date, topic, difficulty, content, reference_answer, lang FROM questions WHERE date = ?1")
            .context("prepare get_today_question")?;
        let mut rows = stmt
            .query_map([date], row_to_question)
            .context("query get_today_question")?;
        rows.next().transpose().context("map row")
    }

    /// 插入题目（date、lang），返回带 id 的 Question
    pub fn insert_question(&self, q: &LlmQuizResponse, date: &str, lang: &str) -> Result<Question> {
        self.conn
            .execute(
                "INSERT INTO questions (date, topic, difficulty, content, reference_answer, lang)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    date,
                    q.topic,
                    q.difficulty,
                    q.question,
                    q.reference_answer,
                    lang
                ],
            )
            .context("insert question")?;
        let id = self.conn.last_insert_rowid();
        Ok(Question {
            id,
            date: date.to_string(),
            topic: q.topic.clone(),
            difficulty: q.difficulty.clone(),
            content: q.question.clone(),
            reference_answer: q.reference_answer.clone(),
            lang: lang.to_string(),
        })
    }

    /// 删除题目，级联删除关联的 answers
    pub fn delete_question(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM answers WHERE question_id = ?1", [id])
            .context("delete answers")?;
        self.conn
            .execute("DELETE FROM questions WHERE id = ?1", [id])
            .context("delete question")?;
        Ok(())
    }

    /// 某题是否已答
    pub fn has_answered(&self, question_id: i64) -> Result<bool> {
        let n: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM answers WHERE question_id = ?1",
                [question_id],
                |r| r.get(0),
            )
            .context("query has_answered")?;
        Ok(n > 0)
    }

    /// 查某题的答案
    pub fn get_answer_for_question(&self, qid: i64) -> Result<Option<Answer>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, question_id, content, score, feedback, answered_at
                 FROM answers WHERE question_id = ?1 ORDER BY id DESC LIMIT 1",
            )
            .context("prepare get_answer")?;
        let mut rows = stmt.query_map([qid], row_to_answer).context("query")?;
        rows.next().transpose().context("map row")
    }

    /// 插入答案 + 更新 topic_mastery（score >= 60 视为正确）
    pub fn insert_answer(
        &mut self,
        qid: i64,
        content: &str,
        judge: &LlmJudgeResponse,
    ) -> Result<()> {
        let tx = self.conn.transaction().context("begin tx")?;
        tx.execute(
            "INSERT INTO answers (question_id, content, score, feedback, topic_tags, answered_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                qid,
                content,
                judge.score,
                judge.feedback,
                serde_json::to_string(&judge.topic_tags).unwrap_or_else(|_| "[]".to_string()),
                chrono::Utc::now().to_rfc3339(),
            ],
        )
        .context("insert answer")?;
        for tag in &judge.topic_tags {
            upsert_topic_mastery(&tx, tag, judge.score >= 60).context("upsert mastery")?;
        }
        tx.commit().context("commit tx")?;
        Ok(())
    }

    /// 按正确率升序排列（最薄弱在前），未练过的排最后
    pub fn get_weak_topics(&self) -> Result<Vec<TopicMastery>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, topic, total, correct, last_practiced FROM topic_mastery
                 ORDER BY CASE WHEN total = 0 THEN 1 ELSE 0 END,
                          CAST(correct AS REAL) / total ASC",
            )
            .context("prepare get_weak_topics")?;
        let rows = stmt
            .query_map([], |r| {
                Ok(TopicMastery {
                    id: r.get(0)?,
                    topic: r.get(1)?,
                    total: r.get(2)?,
                    correct: r.get(3)?,
                    last_practiced: r.get(4)?,
                })
            })
            .context("query")?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.context("map row")?);
        }
        Ok(out)
    }

    /// JOIN 查全部答题历史，用于总结
    pub fn get_all_answers(&self) -> Result<Vec<(Question, Answer)>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT q.id, q.date, q.topic, q.difficulty, q.content, q.reference_answer, q.lang,
                        a.id, a.question_id, a.content, a.score, a.feedback, a.answered_at
                 FROM answers a JOIN questions q ON q.id = a.question_id
                 ORDER BY a.answered_at ASC",
            )
            .context("prepare get_all_answers")?;
        let rows = stmt
            .query_map([], |r| {
                let q = Question {
                    id: r.get(0)?,
                    date: r.get(1)?,
                    topic: r.get(2)?,
                    difficulty: r.get(3)?,
                    content: r.get(4)?,
                    reference_answer: r.get(5)?,
                    lang: r.get(6)?,
                };
                let a = Answer {
                    id: r.get(7)?,
                    question_id: r.get(8)?,
                    content: r.get(9)?,
                    score: r.get(10)?,
                    feedback: r.get(11)?,
                    answered_at: r.get(12)?,
                };
                Ok((q, a))
            })
            .context("query")?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.context("map row")?);
        }
        Ok(out)
    }

    /// 存总结 + 把薄弱 topic 写入 topic_mastery（标记为答错）
    pub fn save_summary(&mut self, s: &LlmSummaryResponse) -> Result<()> {
        let tx = self.conn.transaction().context("begin tx")?;
        tx.execute(
            "INSERT INTO summary_cache (created_at, weak_topics, relations, suggestions)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                chrono::Utc::now().to_rfc3339(),
                serde_json::to_string(&s.weak_topics).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&s.related_weaknesses).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&s.suggestions).unwrap_or_else(|_| "[]".to_string()),
            ],
        )
        .context("insert summary")?;
        for topic in &s.weak_topics {
            upsert_topic_mastery(&tx, topic, false).context("upsert mastery")?;
        }
        tx.commit().context("commit tx")?;
        Ok(())
    }

    /// 取最新的总结
    pub fn get_latest_summary(&self) -> Result<Option<LlmSummaryResponse>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT weak_topics, relations, suggestions FROM summary_cache
                 ORDER BY id DESC LIMIT 1",
            )
            .context("prepare get_latest_summary")?;
        let mut rows = stmt
            .query_map([], |r| {
                let weak: String = r.get(0)?;
                let rel: String = r.get(1)?;
                let sug: String = r.get(2)?;
                Ok((weak, rel, sug))
            })
            .context("query")?;
        match rows.next().transpose().context("map row")? {
            None => Ok(None),
            Some((weak, rel, sug)) => {
                let weak_topics: Vec<String> = serde_json::from_str(&weak).unwrap_or_default();
                let related_weaknesses = serde_json::from_str(&rel).unwrap_or_default();
                let suggestions: Vec<String> = serde_json::from_str(&sug).unwrap_or_default();
                Ok(Some(LlmSummaryResponse {
                    weak_topics,
                    related_weaknesses,
                    suggestions,
                }))
            }
        }
    }

    /// 返回新规则的 id
    pub fn add_rule(&self, content: &str) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO rules (content, created_at) VALUES (?1, ?2)",
                rusqlite::params![content, chrono::Utc::now().to_rfc3339()],
            )
            .context("insert rule")?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 返回所有规则
    pub fn list_rules(&self) -> Result<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, content FROM rules ORDER BY id")
            .context("prepare list_rules")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .context("query")?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.context("map row")?);
        }
        Ok(out)
    }

    /// 删除规则，返回是否删到了
    pub fn delete_rule(&self, id: i64) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM rules WHERE id = ?1", [id])
            .context("delete rule")?;
        Ok(n > 0)
    }

    /// 返回规则内容列表（不含 id）
    pub fn get_rules_text(&self) -> Result<Vec<String>> {
        Ok(self
            .list_rules()?
            .into_iter()
            .map(|(_, content)| content)
            .collect())
    }
}

/// 如果 topic 已存在则更新计数，否则插入新行
fn upsert_topic_mastery(conn: &Connection, topic: &str, correct: bool) -> rusqlite::Result<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM topic_mastery WHERE topic = ?1",
        [topic],
        |r| r.get(0),
    )?;
    if exists > 0 {
        conn.execute(
            "UPDATE topic_mastery
             SET total = total + 1, correct = correct + ?1, last_practiced = ?2
             WHERE topic = ?3",
            rusqlite::params![correct as i32, chrono::Utc::now().to_rfc3339(), topic],
        )?;
    } else {
        conn.execute(
            "INSERT INTO topic_mastery (topic, total, correct, last_practiced)
             VALUES (?1, 1, ?2, ?3)",
            rusqlite::params![topic, correct as i32, chrono::Utc::now().to_rfc3339()],
        )?;
    }
    Ok(())
}
