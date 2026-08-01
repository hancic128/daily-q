//! 中文文案（design 4.6）
//!
//! 全部使用中文，不支持语言切换。`texts!` 宏统一生成文案方法，返回固定中文。

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// 语言（兼容 config.json 旧字段，默认 zh；无切换功能）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Zh,
    En,
}

impl FromStr for Lang {
    type Err = ();
    /// "en" → En，其他一律 Zh
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "en" => Lang::En,
            _ => Lang::Zh,
        })
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lang::Zh => write!(f, "zh"),
            Lang::En => write!(f, "en"),
        }
    }
}

/// 批量生成文案：`texts! { 字段名: "中文", "English"; ... }`
///
/// 语言切换仅作用于题目正文，UI 提示 / 标签 / 评估始终中文，故统一返回中文。
macro_rules! texts {
    ($($name:ident: $zh:literal, $en:literal;)+) => {
        /// 全局文案容器（所有用户可见文案的统一入口）
        #[derive(Debug, Clone, Copy)]
        pub struct Texts;

        impl Texts {
            $(
                /// 返回中文文案（UI 不随语言切换）
                #[allow(dead_code)]
                pub fn $name(&self) -> &'static str {
                    $zh
                }
            )+
        }

        /// 全局文案实例
        pub const TEXTS: Texts = Texts;
    };
}

texts! {
    // ---- 配置 ----
    config_not_found: "未配置 API 密钥，请运行：dq config ai", "API key not set, run: dq config ai";
    base_url_not_found: "未配置 AI 服务商，请运行：dq config ai", "AI provider not set, run: dq config ai";
    api_key_saved: "API 密钥已保存", "API key saved";
    ai_configured: "AI 配置完成", "AI configured";
    api_key_prompt: "请输入 API Key：", "Enter API Key:";
    api_key_required: "API Key 不能为空", "API Key cannot be empty";
    model_saved: "模型已设置", "Model set";
    base_url_saved: "API 地址已设置", "API base URL set";
    provider_list_title: "常用 API 服务商", "Common API providers";
    provider_not_found: "未知的服务商，运行 dq config base-url list 查看可选列表", "Unknown provider, run dq config base-url list to see available providers";
    testing_connection: "正在测试 LLM 连接（请稍候）…", "Testing LLM connection...";
    test_failed: "连接测试失败，配置未保存", "Connection test failed, config not saved";
    reset_confirm: "确定要重置所有配置和数据吗？（将删除 ~/.daily-q/ 全部内容）[y/N]", "Reset all config and data? (will delete everything under ~/.daily-q) [y/N]";
    model_list_title: "常用模型（当前服务商）", "Common models (current provider)";
    model_need_provider: "请先运行 dq config base-url 选择 API 服务商", "Run dq config base-url first to select an API provider";
    model_not_found: "当前服务商暂无模型预设，请直接指定：dq config model <模型名>", "No model presets for this provider, specify directly: dq config model <name>";
    reset_done: "已重置，恢复到初始状态", "Reset complete, back to initial state";
    cancelled: "已取消", "Cancelled";
    focus_saved: "关注方向已设置", "Focus areas set";
    profile_saved: "画像已保存", "Profile saved";
    config_title: "当前配置", "Current config";
    profile_title: "当前画像", "Current profile";
    not_set: "未设置", "Not set";
    api_key_field: "API Key", "API Key";
    model_field: "模型", "Model";
    base_url_field: "API 地址", "API Base URL";
    focus_field: "关注方向", "Focus areas";
    data_dir_field: "数据目录", "Data directory";
    profile_field: "画像", "Profile";

    // ---- 画像引导 ----
    profile_setup_title: "面试画像设置", "Profile setup";
    profile_setup_desc: "以下设置将影响出题与评判", "These settings affect question generation and judging";
    q_experience: "工作年限", "Years of experience";
    q_role: "岗位方向（可多选，用逗号分隔）", "Job roles (multi-select, comma separated)";
    q_tech: "技术栈（可多选，用逗号分隔）", "Tech stack (multi-select, comma separated)";
    q_level: "目标级别", "Target level";
    q_focus: "重点方向（可多选，用逗号分隔）", "Focus areas (multi-select, comma separated)";
    other: "其他", "Other";
    enter_number: "请输入选项编号", "Enter option number";
    invalid_input: "无效输入，请重试", "Invalid input, try again";
    confirm_profile: "确认以上画像？", "Confirm this profile?";

    // ---- 答题流程 ----
    no_question_today: "今天还没有题目，请先运行 dq", "No question today, run dq first";
    already_answered: "今天已答过，去 dq stats 查看", "Already answered today, check dq stats";
    answer_hint: "请在编辑器中作答，保存并退出", "Write your answer in the editor, save and exit";
    answer_saved: "答案已提交", "Answer submitted";
    empty_answer: "回答为空，请勿只写注释", "Answer is empty, don't write only comments";
    editor_failed: "编辑器异常退出", "Editor exited abnormally";
    score_label: "得分", "Score";
    feedback_label: "反馈", "Feedback";
    reference_label: "参考答案", "Reference answer";
    topic_label: "知识点", "Topic";
    difficulty_label: "难度", "Difficulty";
    lang_mismatch_hint: "（题目为原语言展示，未重新生成）", "(question shown in its original language)";

    // ---- 统计展示 ----
    stats_title: "掌握度统计", "Mastery stats";
    no_records: "暂无答题记录", "No records yet";
    summary_title: "最新总结", "Latest summary";
    weak_topics_label: "薄弱知识点", "Weak topics";
    relations_label: "知识关联", "Related weaknesses";
    suggestions_label: "建议", "Suggestions";
    level_strong: "掌握", "strong";
    level_medium: "一般", "medium";
    level_weak: "薄弱", "weak";
    level_none: "未练", "none";

    // ---- 规则管理 ----
    rule_added: "规则已添加", "Rule added";
    rule_deleted: "规则已删除", "Rule deleted";
    rule_list_title: "出题规则", "Question rules";
    no_rules: "暂无规则", "No rules";
    rule_not_found: "规则不存在", "Rule not found";

    // ---- 通用 ----
    error: "错误", "Error";
    done: "完成", "Done";
    today_question: "今日题目", "Today's question";
    question_regenerated: "今日题目已按当前语言重新生成", "Today's question regenerated in current language";
    profile_required: "请先设置画像：dq profile setup", "Set up your profile first: dq profile setup";
}

impl Texts {
    /// 后台总结完成提示（带数量）
    pub fn summary_done(&self, count: usize) -> String {
        format!("[summary] 完成 — {count} 个薄弱项已更新")
    }
}
