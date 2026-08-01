//! LLM 调用层（设计文档 4.3）
//!
//! 封装 OpenAI 兼容的 /v1/chat/completions 端点，提供三个核心函数：
//! 出题、评判、总结。全部使用中文。

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::config::Config;
use crate::models::{
    Answer, LlmJudgeResponse, LlmQuizResponse, LlmSummaryResponse, Question, TopicMastery,
};

/// LLM HTTP 客户端
pub struct LlmClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl LlmClient {
    fn new(config: &Config) -> LlmClient {
        LlmClient {
            client: reqwest::Client::new(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
        }
    }

    /// 纯文本对话，返回原始响应
    async fn chat(&self, system: &str, user: &str) -> Result<String> {
        // base_url 已含 /v1 或 /v4 时直接拼 /chat/completions，避免重复
        let url = if self.base_url.ends_with("/v1") || self.base_url.ends_with("/v4") {
            format!("{}/chat/completions", self.base_url)
        } else {
            format!("{}/v1/chat/completions", self.base_url)
        };
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ]
        });
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("LLM 请求失败")?;
        let status = resp.status();
        let text = resp.text().await.context("读取 LLM 响应失败")?;
        if !status.is_success() {
            bail!("LLM error ({status}): {text}");
        }
        let v: Value = serde_json::from_str(&text).context("解析 LLM 响应失败")?;
        v.pointer("/choices/0/message/content")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            .context("LLM 响应缺少 choices[0].message.content")
    }

    /// 调用 chat，提取 JSON 并反序列化
    async fn chat_json<T: DeserializeOwned>(&self, system: &str, user: &str) -> Result<T> {
        let raw = self.chat(system, user).await?;
        let json = extract_json(&raw);
        // 先解析为 Value，避免 LLM 返回重复 key 导致反序列化失败
        let v: Value = serde_json::from_str(json).context("从 LLM 响应提取 JSON 失败")?;
        serde_json::from_value(v).context("反序列化 LLM JSON 失败")
    }
}

/// 从 LLM 响应中提取 JSON 片段
fn extract_json(raw: &str) -> &str {
    // 1. ```json 代码块
    if let Some(start) = raw.find("```json") {
        let rest = &raw[start + 7..];
        if let Some(end) = rest.find("```") {
            return rest[..end].trim();
        }
    }
    // 2. 普通 ``` 代码块
    if let Some(start) = raw.find("```") {
        let rest = &raw[start + 3..];
        if let Some(end) = rest.find("```") {
            return rest[..end].trim();
        }
    }
    // 3. 从第一个 { 或 [ 提取到括号闭合（容忍前后文本）
    if let Some(start) = raw.find(['{', '[']) {
        if let Some(end) = matching_close(raw, start) {
            return raw[start..=end].trim();
        }
    }
    // 4. 原样 trim
    raw.trim()
}

/// 从 start（{ 或 [）找到匹配的闭合位置，忽略字符串内的括号
fn matching_close(raw: &str, start: usize) -> Option<usize> {
    let bytes = raw.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------- 出题 ----------------

/// LLM 出题
pub async fn generate_question(
    config: &Config,
    weak_topics: &[TopicMastery],
    rules: &[String],
    difficulty_hint: Option<&str>,
) -> Result<LlmQuizResponse> {
    let client = LlmClient::new(config);
    // 全部使用中文出题
    let system = "你是一名资深后端面试官，专门考察程序员的后端和数据方向知识。请用中文出题。只返回JSON，不要其他内容。JSON格式必须是：{\"topic\": \"知识点名称\", \"difficulty\": \"easy/medium/hard\", \"question\": \"题目内容\", \"reference_answer\": \"参考答案\"}";

    let mut parts: Vec<String> = Vec::new();
    if let Some(p) = &config.profile {
        parts.push(format!("候选人画像：{p}"));
    }
    parts.push(format!("知识范围：{}", config.focus_areas.join("、")));
    // 薄弱知识点（level = weak）
    let weak: Vec<&TopicMastery> = weak_topics
        .iter()
        .filter(|t| t.mastery_level() == "weak")
        .collect();
    if !weak.is_empty() {
        let fmt = weak
            .iter()
            .map(|t| {
                let rate = (t.correct as f64 / t.total as f64 * 100.0).round() as i32;
                format!("{}（正确率{}%）", t.topic, rate)
            })
            .collect::<Vec<_>>()
            .join("、");
        parts.push(format!("薄弱知识点：{fmt}"));
    }
    if !rules.is_empty() {
        parts.push(format!("出题规则：{}", rules.join("；")));
    }
    if let Some(d) = difficulty_hint {
        parts.push(format!("题目难度：{d}"));
    }
    let user = parts.join("\n");

    client.chat_json::<LlmQuizResponse>(system, &user).await
}

// ---------------- 评判 ----------------

/// LLM 评判
pub async fn judge_answer(
    config: &Config,
    question: &Question,
    answer: &str,
) -> Result<LlmJudgeResponse> {
    let client = LlmClient::new(config);
    // 评判始终中文（语言切换仅作用于题目）
    let system = "你是一名资深面试官，请评判候选人的回答。用中文反馈。只返回JSON，不要其他内容。JSON格式必须是：{\"score\": 0到100的整数, \"feedback\": \"详细反馈\", \"topic_tags\": [\"知识点标签1\", \"知识点标签2\"]}。topic_tags 必须使用中文知识点名称（Raft、Cassandra 等专有名词除外）";
    let mut user = format!(
        "题目（知识点：{}，难度：{}）：\n{}\n\n参考答案：\n{}\n\n候选人回答：\n{}",
        question.topic, question.difficulty, question.content, question.reference_answer, answer
    );
    if let Some(p) = &config.profile {
        user.push_str(&format!("\n\n候选人画像：{p}（请结合其经验水平评估）"));
    }

    client.chat_json::<LlmJudgeResponse>(system, &user).await
}

// ---------------- 总结 ----------------

/// LLM 总结
pub async fn summarize_history(
    config: &Config,
    history: &[(Question, Answer)],
) -> Result<LlmSummaryResponse> {
    let client = LlmClient::new(config);
    // 总结始终中文（语言切换仅作用于题目）
    let system = "你是一名学习分析专家，分析用户的答题历史，总结薄弱点和知识关联。用中文。只返回JSON，不要其他内容。JSON格式必须是：{\"weak_topics\": [\"薄弱知识点\"], \"related_weaknesses\": [{\"topic_a\": \"知识点A\", \"topic_b\": \"知识点B\", \"reason\": \"原因\"}], \"suggestions\": [\"建议\"]}。weak_topics 使用中文知识点名称（专有名词除外）";

    let mut lines = Vec::new();
    for (q, a) in history {
        lines.push(format!(
            "[{}] {} | 得分: {} | 反馈: {}",
            q.topic, q.content, a.score, a.feedback
        ));
    }
    let user = lines.join("\n");

    client.chat_json::<LlmSummaryResponse>(system, &user).await
}

// ---------------- 连通性测试 ----------------

/// 连通性测试：发送最小请求验证 base_url / model / api_key 可用
pub async fn test_connection(config: &Config) -> Result<String> {
    let client = LlmClient::new(config);
    client
        .chat(
            "You are a connectivity test. Reply with exactly: ok",
            "test",
        )
        .await
}
