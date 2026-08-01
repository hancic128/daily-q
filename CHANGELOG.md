# Changelog

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/) 与 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [0.1.0] - 2026-08-01

首个可用版本。

### 新增

- **每日一题 CLI**：基于面试画像 + 薄弱知识点 + 自定义规则，由 LLM 动态出题
- **编辑器作答**：`dq answer` 复用 `$EDITOR` 写长答案，AI 评分（0-100）+ 详细反馈 + 参考答案
- **掌握度画像**：`dq stats` 按知识点统计正确率与等级（强/中/弱）
- **后台学习总结**：答题后自动分析薄弱点、知识关联与学习建议（`summary_cache` 落库）
- **全中文体验**：题目、提示、评分、总结全部使用中文，专注面试练习
- **一键 AI 配置**：`dq config ai` 交互式选择服务商（deepseek / 硅基流动 / OpenAI / Kimi / 智谱 / 通义 / Ollama）+ 输入 Key，自动设置地址与模型，配置前自动测试连通性
- **出题规则**：`dq rule add/list/remove` 自定义出题约束
- **交互式画像**：`dq profile setup`，岗位方向与技术栈支持多选（11 种岗位）
- **SQLite 单文件存储**：数据在 `~/.daily-q/`，5 张表，零运维
- **配置重置**：`dq config reset` 交互确认后清空全部配置与数据

### 修复

- LLM 返回 JSON 提取鲁棒性：支持代码块包裹、括号匹配、容忍前后文本
- `max_tokens` 提升至 4096，避免长输出截断破坏 JSON
- 后台总结线程在进程退出前等待落库，避免数据丢失

### 已知限制

- 依赖 LLM 可用性与响应质量；小参数模型（如 Qwen2.5-7B）JSON 生成能力不足，建议使用 deepseek-v4-flash / Qwen2.5-72B 及以上
