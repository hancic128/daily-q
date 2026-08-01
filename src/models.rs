//! 核心数据结构（设计文档 3.1 / 3.2）

use serde::{Deserialize, Serialize};

/// 题目（questions 表一行）
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Question {
    pub id: i64,
    pub date: String, // "2024-08-01"
    pub topic: String,
    pub difficulty: String, // "easy" / "medium" / "hard"
    pub content: String,
    pub reference_answer: String,
    pub lang: String, // 题目语言 "zh" / "en"
}

/// 答题记录（注意：不含 topic_tags，tags 存在 answers 表的 JSON 字段中）
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Answer {
    pub id: i64,
    pub question_id: i64,
    pub content: String,
    pub score: i32, // 0-100
    pub feedback: String,
    pub answered_at: String, // RFC3339
}

/// 知识点掌握度（topic_mastery 表一行）
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TopicMastery {
    pub id: i64,
    pub topic: String,
    pub total: i32,
    pub correct: i32,
    pub last_practiced: Option<String>,
}

impl TopicMastery {
    /// 掌握度等级
    /// - total == 0 → "none"
    /// - correct/total >= 0.8 → "strong"
    /// - correct/total >= 0.5 → "medium"
    /// - 其他 → "weak"
    pub fn mastery_level(&self) -> &'static str {
        if self.total == 0 {
            "none"
        } else {
            let rate = self.correct as f64 / self.total as f64;
            if rate >= 0.8 {
                "strong"
            } else if rate >= 0.5 {
                "medium"
            } else {
                "weak"
            }
        }
    }
}

/// 一次做题会话（题目 + 是否已答 + 可选的回答记录）
#[derive(Debug, Clone)]
pub struct QuizSession {
    pub question: Question,
    pub answered: bool,
    pub answer: Option<Answer>,
}

// ---------------- LLM 交互模型 ----------------

/// LLM 出题返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmQuizResponse {
    pub topic: String,
    pub difficulty: String,
    pub question: String,
    pub reference_answer: String,
}

/// LLM 评判返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmJudgeResponse {
    pub score: i32, // 0-100
    pub feedback: String,
    pub topic_tags: Vec<String>, // ["TCP", "三次握手", "传输层"]
}

/// LLM 总结返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSummaryResponse {
    pub weak_topics: Vec<String>,
    pub related_weaknesses: Vec<TopicRelation>,
    pub suggestions: Vec<String>,
}

/// 薄弱点关联
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicRelation {
    pub topic_a: String,
    pub topic_b: String,
    pub reason: String,
}

/// 返回今天日期字符串 "2024-08-01"
pub fn today_str() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
