1. 项目概述

daily-q 是一个「每日一题」CLI 工具，面向后端/数据方向程序员的面试知识练习。

核心流程：每天出 1 道面试题 → 用户答题 → AI 评分反馈 → 后台分析薄弱点 → 下次出题时倾向薄弱点。

技术栈：Rust + SQLite + Claude API（兼容 OpenAI 协议的任意 LLM）



2. 系统架构

┌──────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐
│  main.rs │  │  quiz.rs   │  │  summary   │  │  profile   │
│  CLI入口  │→ │ 出题+评判  │  │  .rs       │  │  .rs       │
└──────────┘  └─────┬─────┘  │  后台总结   │  │  画像引导  │
                      │        └─────┬─────┘  └───────────┘
                      │              │
              ┌───────┴──────┐       │
              │    llm.rs     │←──────┘
              │  Claude API   │
              └───────┬───────┘
                      │
              ┌───────┴──────┐
              │    db.rs      │
              │   SQLite      │
              └───────┬──────┘
                      │
              ┌───────┴──────┐
              │  models.rs   │
              │  数据结构     │
              └──────────────┘

模块职责







文件



职责



行数





main.rs



CLI 入口，clap 命令路由



~419





config.rs



配置读写（JSON 文件）



~251





db.rs



SQLite 增删改查



~385





llm.rs



LLM API 调用（出题/评判/总结）



~264





quiz.rs



出题+评判+统计业务逻辑



~247





summary.rs



后台线程触发 LLM 总结



~49





profile.rs



交互式画像引导设置



~139





models.rs



数据结构定义



~109





lang.rs



中文文案（宏生成）



~187





display.rs



终端颜色输出



~38

数据流向

                    用户执行 dq
                        │
                        ▼
              quiz::get_or_create_question()
                        │
                  ┌─────┴──────┐
                  │  今天已有题?  │
                  └─────┬──────┘
                  YES    │    NO
                 直接返回  │
                        ▼
              ┌──────────────────┐
              │ llm::generate_  │ ← 传: 薄弱知识点、规则、画像
              │ question()       │
              └──────┬─────────┘
                     │
                     ▼
              db.insert_question()  → 存入 SQLite
                     │
                     ▼
              显示题目给用户
                     │
         ┌───────────┴───────────┐
         │ 用户执行 dq answer     │
         │ 打开编辑器，写回答      │
         └───────────┬───────────┘
                     │
                     ▼
              llm::judge_answer()  → 评分 0-100 + 反馈 + 标签
                     │
                     ▼
              db.insert_answer()    → 存入 + 更新 mastery 表
                     │
                     ▼
              summary::spawn_background_summary()
                     │ (后台线程)
                     ▼
              llm::summarize_history()  → 分析薄弱点、关联、建议
                     │
                     ▼
              db.save_summary()   → 存入 summary_cache 表



3. 数据结构

3.1 核心模型 (models.rs)

// 题目
struct Question {
    id: i64,
    date: String,          // "2024-08-01"
    topic: String,         // "TCP 三次握手"
    difficulty: String,    // "easy" / "medium" / "hard"
    content: String,      // 题目正文
    reference_answer: String, // 参考答案
    lang: String,              // 题目语言 "zh" / "en"
}
​
// 答题记录（注意：不含 topic_tags，tags 存在 answers 表的 JSON 字段中）
struct Answer {
    id: i64,
    question_id: i64,
    content: String,      // 用户的回答
    score: i32,          // 0-100
    feedback: String,    // AI 的详细反馈
    answered_at: String,
}
​
// 知识点掌握度
struct TopicMastery {
    id: i64,
    topic: String,
    total: i32,          // 总答题次数
    correct: i32,         // 答对次数（score >= 60 算正确）
    last_practiced: Option<String>,
​
    // 计算方法：mastery_level()
    // - total == 0 → "none"
    // - correct/total >= 0.8 → "strong"
    // - correct/total >= 0.5 → "medium"
    // - 其他 → "weak"
}
​
// 一次做题会话（题目 + 是否已答 + 可选的回答记录）
struct QuizSession {
    question: Question,
    answered: bool,
    answer: Option<Answer>,
}
​
// 辅助函数
fn today_str() -> String  // 返回 "2024-08-01"

3.2 LLM 交互模型

// LLM 出题返回
struct LlmQuizResponse {
    topic: String,
    difficulty: String,
    question: String,
    reference_answer: String,
}
​
// LLM 评判返回
struct LlmJudgeResponse {
    score: i32,           // 0-100
    feedback: String,
    topic_tags: Vec<String>, // ["TCP", "三次握手", "传输层"]
}
​
// LLM 总结返回
struct LlmSummaryResponse {
    weak_topics: Vec<String>,
    related_weaknesses: Vec<TopicRelation>,
    suggestions: Vec<String>,
}
​
// 薄弱点关联
struct TopicRelation {
    topic_a: String,
    topic_b: String,
    reason: String,
}

3.3 数据库表结构

-- 5 张表，全部在 db.rs 的 SCHEMA 常量中定义
​
CREATE TABLE questions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL UNIQUE,       -- 每天只有一条
    topic TEXT NOT NULL,
    difficulty TEXT NOT NULL,
    content TEXT NOT NULL,
    reference_answer TEXT NOT NULL,
    lang TEXT NOT NULL DEFAULT 'zh'   -- 题目语言
);
​
CREATE TABLE answers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question_id INTEGER NOT NULL REFERENCES questions(id),
    content TEXT NOT NULL,
    score INTEGER NOT NULL,
    feedback TEXT NOT NULL,
    topic_tags TEXT NOT NULL DEFAULT '[]',  -- JSON 数组字符串
    answered_at TEXT NOT NULL                -- RFC3339 时间戳
);
​
CREATE TABLE topic_mastery (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    total INTEGER NOT NULL DEFAULT 0,
    correct INTEGER NOT NULL DEFAULT 0,
    last_practiced TEXT
);
​
CREATE TABLE summary_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    weak_topics TEXT NOT NULL DEFAULT '[]',    -- JSON
    relations TEXT NOT NULL DEFAULT '[]',        -- JSON
    suggestions TEXT NOT NULL DEFAULT '[]'       -- JSON
);
​
CREATE TABLE rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);

3.4 数据存储位置

所有数据存储在 ~/.daily-q/ 目录下：





db 文件：~/.daily-q/daily-q.db（SQLite）



配置文件：~/.daily-q/config.json



4. 模块详细设计

4.1 config.rs —— 配置管理

职责：读写 ~/.daily-q/config.json，管理 API Key、模型、URL、关注方向、画像。

struct Config {
    api_key: String,           // 从 config.json 读，为空则报错退出
    model: String,              // 默认 "deepseek-chat"
    base_url: String,          // 默认空，需用户配置（如 "https://api.deepseek.com"）
    focus_areas: Vec<String>,  // 默认 ["计算机网络","操作系统","数据库","数据结构与算法","分布式系统"]
    profile: Option<String>,   // 画像字符串，如 "3-5年 | 后端 | Go | 高级 | 分布式"
    lang: Lang,               // zh 或 en，默认 zh
    data_dir: PathBuf,
}

对外接口：





Config::load() -> Result<Config> — 读配置，API key 为空时直接 bail!



Config::save(api_key?, model?, base_url?, focus?) -> Result<()> — 部分更新



Config::set_profile(profile: &str) -> Result<()> — 单独写画像






Config::ensure_data_dir() -> Result<()> — 确保 ~/.daily-q/ 存在



Config::db_path() -> PathBuf — 返回 db 文件路径






Config::reset() -> Result<()> — 重置：删除 ~/.daily-q/ 全部内容（交互确认）

内部实现：用 serde_json::Value 做部分更新 —— 先读整个 JSON，修改对应字段，再写回。避免覆盖未提供的字段。


服务商预设：base_url 不手动输入，内置 7 个 OpenAI 兼容服务商预设（PROVIDERS 三元组：名字 / base_url / 默认模型）——deepseek、硅基流动、openai、moonshot、zhipu、qwen、ollama。`dq config ai` 一键配置：选服务商 → 输 API Key → 自动带出 base_url 与默认模型 → 测试连通 → 一次保存（不再拆三步）。

配置测试校验：配置 api-key / model / base-url 时，若三者齐备先调用 LLM 连通性测试（最小请求），测试通过才保存，失败报错不落盘（main.rs 的 save_with_test）。



4.2 db.rs —— 数据层

职责：封装所有 SQLite 操作，5 张表的 CRUD。

每个方法的入参出参：







方法



入参



出参



说明





Db::open()



-



Result<Db>



打开连接，执行建表语句





get_today_question(date)



&str



Option<Question>



按日期查题





insert_question(q, date)



&LlmQuizResponse, &str, &str



Result<Question>



插入题目（date、lang），返回带 id 的 Question





delete_question(id)



i64



Result<()>



级联删除关联的 answers





has_answered(question_id)



i64



Result<bool>



某题是否已答





get_answer_for_question(qid)



i64



Option<Answer>



查某题的答案





insert_answer(qid, content, judge)



i64, &str, &LlmJudgeResponse



Result<()>



插入答案 + 更新 topic_mastery





upsert_topic_mastery(topic, correct)



&str, bool



Result<()>



如果 topic 已存在则更新计数，否则插入新行





get_weak_topics()



-



Vec<TopicMastery>



按正确率升序排列（最薄弱在前）





get_all_answers()



-



Vec<(Question, Answer)>



JOIN 查全部答题历史，用于总结





save_summary(s)



&LlmSummaryResponse



Result<()>



存总结 + 把薄弱 topic 写入 topic_mastery





get_latest_summary()



-



Option<LlmSummaryResponse>



取最新的总结





add_rule(content)



&str



Result<i64>



返回新规则的 id





list_rules()



-



Vec<(i64, String)>



返回所有规则





delete_rule(id)



i64



Result<bool>



返回是否删到了





get_rules_text()



-



Vec<String>



返回规则的内容列表（不含 id）

关键逻辑：





insert_answer 内部分两步：先插入 answers 行，再对 judge.topic_tags 中的每个 tag 调用 upsert_topic_mastery。score >= 60 视为正确。



upsert_topic_mastery 先检查 topic 是否存在，存在则 UPDATE（total+1，correct 视情况 +1），否则 INSERT 新行。



save_summary 插入后也遍历 weak_topics 更新 topic_mastery（标记为答错）。



4.3 llm.rs —— LLM 调用

职责：封装 LLM API 调用，提供三个核心函数：出题、评判、总结。使用 OpenAI 兼容的 /v1/chat/completions 端点。

内部结构：

LlmClient {
    client: reqwest::Client,   // HTTP 客户端
    api_key: String,
    model: String,             // 如 "deepseek-chat"
    base_url: String,         // 如 "https://api.deepseek.com"
}
​
// 基础方法
chat(system, user) -> Result<String>       // 纯文本对话，返回原始响应
chat_json<T>(system, user) -> Result<T>   // 调用 chat，然后从响应中提取 JSON 并反序列化
​
// 工具函数
extract_json(raw: &str) -> &str           // 从 LLM 响应中提取 JSON（处理 ```json``` 包裹）

extract_json 的逻辑：





先找 ```json 代码块，取里面的内容



没有则找 ``` 普通代码块



都没有则从第一个 { 或 [ 提取到括号闭合（忽略字符串内括号，容忍前后文本）



最终兜底返回原始文本 trim

chat 方法细节：





请求体：{model, max_tokens: 4096, messages: [{role, content}]}



请求头：Authorization: Bearer {api_key}，content-type: application/json



状态码非 200 时返回完整错误文本



从 choices[0].message.content 取响应



base_url 拼接：若 base_url 已含 /v1 或 /v4 后缀则直接拼 /chat/completions，否则拼 /v1/chat/completions（服务商预设统一处理）

chat_json 方法细节：





先调 chat 拿原始文本



调 extract_json 提取 JSON 片段



用 serde_json::from_value 反序列化（先解析为 Value，处理可能的重复 key 问题）



4.3.1 出题 —— generate_question()

入参：

pub async fn generate_question(
config: &Config,           // 含 api_key, model, base_url, profile, focus_areas
    weak_topics: &[TopicMastery],  // 薄弱知识点列表
    rules: &[String],          // 用户自定义的出题规则
    difficulty_hint: Option<&str>,  // 用户指定的难度（easy/medium/hard），None 则随机
) -> Result<LlmQuizResponse>

System Prompt（中文为例）：



你是一名资深后端面试官，专门考察程序员的后端和数据方向知识。请用中文出题。只返回JSON，不要其他内容。JSON格式必须是：{"topic": "知识点名称", "difficulty": "easy/medium/hard", "question": "题目内容", "reference_answer": "参考答案"}

User Prompt 构成（拼接以下信息）：





候选人画像（来自 config.profile，如为空则省略）



知识范围（config.focus_areas，用顿号连接）



薄弱知识点提示（取 weak_topics 中 level="weak" 的项，写成"TCP（正确率30%）"格式）



出题规则（用户定义，如"只能出场景题"）



题目难度（有则写，无则省略）




返回格式要求：

{
  "topic": "知识点名称",
  "difficulty": "easy/medium/hard",
  "question": "题目内容",
  "reference_answer": "参考答案"
}



4.3.2 评判 —— judge_answer()

入参：

pub async fn judge_answer(
    config: &Config,
    question: &Question,     // 题目信息
    answer: &str,           // 用户回答
) -> Result<LlmJudgeResponse>

System Prompt（中文为例）：



你是一名资深面试官，请评判候选人的回答。用中文反馈。只返回JSON，不要其他内容。JSON格式必须是：{"score": 0到100的整数, "feedback": "详细反馈", "topic_tags": ["知识点标签"]}。topic_tags 使用中文知识点名称

User Prompt 构成：





题目正文 + 知识点 + 参考答案



候选人回答



候选人画像（如有，补充"结合其经验水平评估"）

返回格式：

{
  "score": 0-100的整数,
  "feedback": "详细反馈，指出不足和改进方向",
  "topic_tags": ["TCP", "三次握手", "传输层"]
}

评分标准：0-100 分。topic_tags 用于更新 topic_mastery 表。



4.3.3 总结 —— summarize_history()

入参：

pub async fn summarize_history(
    config: &Config,
    history: &[(Question, Answer)],  // 全部历史答题记录
) -> Result<LlmSummaryResponse>

System Prompt（中文为例）：



你是一名学习分析专家，分析用户的答题历史，总结薄弱点和知识关联。用中文。只返回JSON，不要其他内容。JSON格式必须是：{"weak_topics": ["薄弱知识点"], "related_weaknesses": [{"topic_a": "A", "topic_b": "B", "reason": "原因"}], "suggestions": ["建议"]}

User Prompt：拼接所有历史答题记录的摘要（每题：[知识点] 题目 | 得分: XX | 反馈: XXX）。

返回格式：

{
  "weak_topics": ["知识点1", "知识点2"],
  "related_weaknesses": [
    {"topic_a": "TCP", "topic_b": "UDP", "reason": "TCP和UDP都答错，说明传输层整体薄弱"}
  ],
  "suggestions": ["建议1", "建议2"]
}



4.4 quiz.rs —— 业务逻辑

职责：出题、答题、统计的核心业务流程。

对外接口：

// 获取或创建今日题目（无题则调 LLM 生成）
pub async fn get_or_create_question(difficulty: Option<&str>) -> Result<QuizSession>

// 跳过今日题目（删除今日 question）
pub fn skip_today() -> Result<()>

// 答题：打开编辑器让用户写回答 → 调 LLM 评判 → 存库
pub async fn answer_interactive() -> Result<()>

// 答题：直接传字符串（未暴露到 CLI，内部使用）
pub async fn submit_answer(answer: &str) -> Result<Answer>

// 展示掌握度统计
pub fn show_stats() -> Result<()>

// 渲染一道题（带答案/不带答案）
pub fn show_question(session: &QuizSession)

get_or_create_question 逻辑：





加载 Config，检查 profile 是否存在（没有则报错退出）



查今天是否已有题






没题：取薄弱知识点 + 规则列表 → 调 llm::generate_question → db.insert_question（题目为中文，questions.lang 默认 zh）



answer_interactive 逻辑：





检查今天是否有题（没有报错），是否已答（已答报错）



打开 $EDITOR（默认 vim），创建临时文件，文件头写 # 知识点: 难度\n# 题目内容



用户写完后，读回文件内容，跳过 # 开头的注释行



调 llm::judge_answer 评分



db.insert_answer 存答案



打印：得分、反馈、参考答案



最后调 summary::spawn_background_summary() 触发后台总结

show_stats 逻辑：





查 get_weak_topics()（按正确率升序）



逐行打印，每行格式：[图标] 知识点名  correct/total (百分比) [等级]



图标：strong→+(绿)，medium→~(黄)，weak→!(红)



如果有最新总结，也打印出来（薄弱点、关联、建议）

answer_interactive 的编辑器交互细节：

# 临时文件内容（模板）:
# TCP 三次握手: medium
# 请描述TCP三次握手的过程

<用户在此输入回答>

读回后过滤逻辑：lines().filter(|l| !l.starts_with('#'))，保留非注释行。回答为空则报错。

国内 LLM 兼容性：show_question 和 answer_interactive 虽然调 LLM，但 main.rs 的 None 分支（dq 无参数）只调 show_question 不调 LLM。dq answer 才调。



4.5 summary.rs —— 后台总结

职责：在用户答完题后，启动一个后台线程去做 LLM 总结，不阻塞终端。

// 公共接口：从 quiz.rs 调用
pub fn spawn_background_summary()

// 内部实现
fn run_summary() -> Result<()>  // 创建 tokio runtime，block_on 异步任务
async fn do_summary() -> Result<()>  // 查历史 → 调 LLM 总结 → 存库

关键细节：





使用 std::thread::spawn（不是 tokio::spawn），因为要在同步上下文中启动



线程内创建独立的 tokio::runtime::Builder::new_current_thread()，因为 reqwest 的异步需要 tokio context



失败只打印错误，不 panic；spawn 返回 JoinHandle，quiz 在答案展示后 join 等待总结落库（CLI 进程在 main 返回后即退出，detach 线程会被杀死导致总结丢失，故必须 join）



总结结果存到 summary_cache 表，最新的覆盖旧的



成功时用 stderr 输出简短提示（"[summary] 完成 — 3 个薄弱项已更新"）



4.6 lang.rs —— 中文文案

实现方式：用 Rust macro texts! 批量生成一个 Texts 结构体，每个字段是一个 fn(Lang) -> &'static str 的方法。

enum Lang { Zh, En }

// 宏用法示例：
texts! {
    config_not_found: "未配置API密钥", "API key not set";
    api_key_saved: "API密钥已保存", "API key saved";
    // ... 约 45 个字段
}

Lang 判断逻辑：





Lang::from_str("en") → En，其他一律 Zh



默认 Zh



全部文案固定中文，不支持语言切换

覆盖范围：配置提示、画像引导、答题流程、统计展示、规则管理、总结提示 —— 所有面向用户的文案。



4.7 display.rs —— 终端样式

职责：用 colored crate 给文本上色，提供语义化函数。

pub fn title(text: &str) -> String    // 青色，用于标题行
pub fn label(text: &str) -> String    // 蓝色，用于字段名（"得分:"）
pub fn highlight(text: &str) -> String // 黄色，用于高亮值
pub fn dim(text: &str) -> String      // 暗色，用于提示
pub fn good(text: &str) -> String     // 绿色，用于正面信息
pub fn warn(text: &str) -> String     // 紫色，用于警告
pub fn bad(text: &str) -> String      // 红色，用于错误



4.8 profile.rs —— 画像设置

职责：交互式引导用户填写面试画像（工作年限、岗位方向、技术栈、目标级别、重点方向）。

流程（profile::run_profile_setup()）：





打印标题和说明



逐题问，每题用 prompt_select() 函数显示选项列表（带编号），用户输入数字或选择"其他"手动输入



多选题支持逗号分隔（如 "1,3"）



技术栈选项根据所选岗位（可多选）取并集动态过滤（get_tech_options()）：从 TECH_STACKS 常量中匹配各岗位技术栈并去重合并



最后一题确认，拼接成字符串，岗位与技术栈多选用顿号连接，如："8年+ | 后端、架构师 | Java、分布式 | 专家 | 分布式、高并发"



写入 Config::set_profile

选项数据：硬编码在代码中（中文），共 11 种岗位 × 各自技术栈（含架构师、AI）。岗位方向与技术栈均支持多选（逗号分隔）。

交互循环：prompt_select() 内部是一个 loop，直到用户给出有效输入。无效输入显示错误提示，重新提问。



4.9 main.rs —— CLI 入口

命令结构（clap derive）：

dq [--difficulty <easy|medium|hard>]  # 默认：获取/显示今日题目
dq answer                               # 打开编辑器答题
dq skip                                # 跳过今日题目（删除）
dq stats                               # 查看掌握度统计
dq rule add <规则文本>                  # 添加出题规则
dq rule list                          # 列出所有规则
dq rule remove <id>                   # 删除规则
dq profile                            # 查看当前画像
dq profile setup                      # 交互式设置画像
dq config                             # 查看当前配置
dq config ai                              # 一键配置 AI：选服务商 → 输 Key → 自动设置地址与模型
dq config api-key <key>               # 设置 API Key
dq config model [name]               # 设置模型（无参交互式选择当前服务商常用模型）
dq config base-url [name|list]       # 单独设置 API 地址（高级，一般用 dq config ai）
dq config focus <逗号分隔>           # 设置关注方向

dq config reset                      # 重置所有配置和数据（交互确认）

命令分发逻辑（main() 函数）：





匹配 cli.command，分派到对应函数



None（无子命令）且有 --difficulty：删旧题重新生成









每个分支都处理 Result，错误用 anyhow::bail! 返回，Clap 自动打印错误信息



5. 关键设计决策





无本地题库，全由 LLM 动态出题：根据用户薄弱点和画像定制，灵活但依赖 LLM 可用性



SQLite 单文件：零运维，数据在 ~/.daily-q/ 下，查询方便



后台线程做总结：用 std::thread::spawn + 独立 tokio runtime，答题后不阻塞终端



topic_mastery 的更新时机：评判时根据 judge.topic_tags 更新，总结时根据 weak_topics 也更新（标记为答错）



Answer 表存 topic_tags 为 JSON 字符串：方便后续扩展，但 Answer struct 不包含此字段



用户画像：交互式引导设置，字符串格式存储，嵌入 LLM prompt 中影响出题和评判



出题规则：允许用户自定义约束（如"只出场景题"、"不要算法题"），作为 system prompt 的一部分传给 LLM



编辑器答题：用 $EDITOR 环境变量，默认 vim，临时文件头写题目信息作为提示



配置文件部分更新：Config::save 只传要改的字段，其余保持不变——用 serde_json::Value 实现



全中文体验：所有用户可见文案与 LLM 内容（题目/反馈/总结）均为中文，专注面试练习



文案统一：lang.rs 的 texts! 宏统一管理所有用户可见文案，固定返回中文



服务商预设：base_url 不手动输入，内置常用 OpenAI 兼容服务商预设（deepseek / siliconflow / openai / moonshot / zhipu / qwen / ollama），`dq config base-url` 交互式选择或按名指定；llm.rs 自动处理 /v1、/v4 后缀避免重复



配置测试校验：修改 api-key / model / base-url 时，若三要素齐备先调用 LLM 连通性测试，通过才保存，失败报错不落盘



多选画像：岗位方向与技术栈支持多选（逗号分隔），技术栈取所选岗位并集，画像字符串更精确表达候选人能力



JSON 提取鲁棒性：extract_json 支持 ```json 代码块、括号匹配提取，容忍 LLM 返回带前后文本；max_tokens 4096 避免长输出截断破坏 JSON



6. 错误处理策略





配置缺失：API key 未设置时 Config::load() 直接 bail!，提示运行 dq config ai 一键配置



配置缺失（API 地址）：base_url 未设置时 Config::load() 直接 bail!，提示运行 dq config ai 一键配置>



画像缺失：check_ready() 检查 profile 为空时 bail，提示用户先设置



今日无题：答 answer 时如果今天没题，bail 提示先运行 dq



已答过：bail 提示去 dq stats 查看



LLM 调用失败：用 anyhow::Context 包装错误，返回可读的错误信息



后台总结失败：只打 stderr 日志，不影响主流程



编辑器异常：编辑器退出码非 0 或文件读失败，直接 bail





