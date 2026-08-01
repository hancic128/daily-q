# daily-q 使用文档

## 目录

1. [安装](#安装)
2. [快速开始](#快速开始)
3. [命令详解](#命令详解)
4. [配置说明](#配置说明)
5. [面试画像](#面试画像)
6. [出题规则](#出题规则)
7. [数据存储与备份](#数据存储与备份)
8. [常见问题](#常见问题)

---

## 安装

前置要求：Rust 工具链（≥1.88）、可访问 OpenAI 兼容 API 的网络。

```bash
git clone https://github.com/Angryshark128/daily-q && cd daily-q
cargo build --release
```

二进制产物为 `target/release/dq`，建议加入 PATH：

```bash
# macOS / Linux
ln -s "$PWD/target/release/dq" /usr/local/bin/dq
```

## 快速开始

```bash
# 1. 一键配置 AI（选择服务商 → 输入 Key，自动设置地址与模型）
dq config ai

# 2. 设置面试画像（强烈建议，直接影响出题针对性）
dq profile setup

# 3. 日常使用
dq            # 获取今日题目（首次会自动生成）
dq answer     # 打开编辑器作答，获得 AI 评分与反馈
dq stats      # 查看掌握度与学习建议
```

## 命令详解

### `dq` —— 获取/生成今日题目

```
dq                          # 有题直接展示；无题则生成
dq --difficulty hard        # 今日未答时按指定难度重新生成
```

设计原则：**今日已有题时零 LLM 调用**，秒开展示；仅在今日无题或指定难度时才调 LLM。

### `dq answer` —— 作答

- 打开 `$EDITOR`（默认 `vim`）编辑临时文件，文件头写有题目信息作提示
- 保存退出后自动提交，AI 给出：`得分`、`反馈`、`参考答案`
- 作答后自动触发后台 LLM 总结（薄弱点 / 知识关联 / 建议），写入 `summary_cache`

回答为空或只写了 `#` 注释行会报错。

### `dq skip` —— 跳过今日

删除今日题目（含关联的答题记录），明天会重新生成。

### `dq stats` —— 掌握度统计

```
[图标] 知识点  correct/total (正确率%) [等级]
```

- 图标/等级：`+` 掌握（≥80%）、`~` 一般（≥50%）、`!` 薄弱（<50%）、`-` 未练
- 按正确率升序排列，最薄弱在最前
- 有总结时一并展示最新学习总结

### `dq rule` —— 出题规则

```bash
dq rule add 只出场景题        # 添加规则
dq rule list                 # 列出规则
dq rule remove 2             # 删除 id=2 的规则
```

规则作为 system prompt 的一部分传入 LLM，用于硬性约束出题方向。

### `dq profile` —— 画像

```bash
dq profile          # 查看当前画像
dq profile setup    # 交互式设置（年限/岗位/技术栈/级别/重点方向）
```

画像格式：`3-5年 | 后端 | Go | 高级 | 分布式`，嵌入出题与评判 prompt。

### `dq config` —— 配置

```bash
dq config                           # 查看配置
dq config ai                        # 一键配置 AI：选服务商 → 输 Key → 自动设置地址与模型
dq config model                     # 交互式选择当前服务商常用模型（或直接指定名字）
dq config focus 网络,数据库,分布式   # 关注方向（覆盖式）
dq config reset                     # 重置所有配置和数据（交互确认）
```

**推荐 `dq config ai` 一键配置**：选择服务商后自动带出 `base_url` 与默认模型，只需粘贴 API Key，测试通过即保存，无需分别配置地址/模型/密钥。

**连通性校验**：配置 `api-key` / `model` / `base-url` 时，若三者齐备会自动发送最小请求测试 LLM 连通性——**测试通过才保存**，失败会报错且不落盘，避免配错模型/地址/密钥。三者未齐时直接保存（等配齐后再次配置即触发测试）。

## 配置说明

配置文件 `~/.daily-q/config.json`：

```json
{
  "api_key": "sk-xxx",
  "model": "deepseek-chat",
  "base_url": "https://api.deepseek.com/v1",
  "focus_areas": ["计算机网络", "操作系统", "数据库", "数据结构与算法", "分布式系统"],
  "profile": "3-5年 | 后端 | Go | 高级 | 分布式",
}
```

任意字段缺失时使用默认值（`config.json` 不存在视为空配置）。默认**不预置** `api_key`、`base_url`、`profile`，需要用户配置。

### API 服务商预设

`base_url` 无需手动输入，**`dq config ai` 一键选择**（也可 `dq config base-url` 单独选择，`list` 查看全部）：

| 名字 | base_url | 常用模型示例 |
|------|----------|-------------|
| `deepseek` | `https://api.deepseek.com/v1` | `deepseek-chat` / `deepseek-v4-flash` |
| `siliconflow` | `https://api.siliconflow.cn/v1` | Qwen2.5 / DeepSeek-V3 等 |
| `openai` | `https://api.openai.com/v1` | `gpt-4o-mini` 等 |
| `moonshot` | `https://api.moonshot.cn/v1` | `moonshot-v1-8k` 等 |
| `zhipu` | `https://open.bigmodel.cn/api/paas/v4` | `glm-4-flash` 等 |
| `qwen` | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `qwen-turbo` 等 |
| `ollama` | `http://localhost:11434/v1` | 本地模型 |

切换服务商后运行 `dq config model` **交互式选择**当前服务商的常用模型（deepseek 等均有预设），也可直接指定模型名。

### 接入其他模型

只要兼容 OpenAI 的 `/v1/chat/completions` 即可，直接指定名字或 URL：

```bash
dq config base-url siliconflow     # 预设服务商
dq config model Qwen/Qwen2.5-7B-Instruct
dq config api-key sk-xxx
```

## 面试画像

画像通过 `dq profile setup` 交互式设置，包含 5 个维度：

| 维度 | 示例选项 |
|------|---------|
| 工作年限 | `<1年 / 1-2年 / 3-5年 / 5-8年 / 8年+` |
| 岗位方向 | 可多选（逗号分隔），如"后端、数据"；选项：后端 / 前端 / 数据 / 算法 / 架构师 / AI / 测试 / 运维 / 安全 / 移动 / 产品 |
| 技术栈 | 可多选；按所选岗位取并集动态过滤，如"后端、数据"→ Go/Java/Python/Spark/Flink 等 |
| 目标级别 | 初级 / 中级 / 高级 / 专家 |
| 重点方向 | 分布式 / 高并发 / 微服务 / 性能优化 / 架构设计 / 数据库 / 缓存 / 消息队列 |

画像影响：出题时的知识范围与深度、评判时结合经验水平评估。

## 出题规则

规则是用户自定义的硬约束，会随每次出题传给 LLM。示例：

```bash
dq rule add 只能出场景题，不要纯概念背诵题
dq rule add 不要算法题
dq rule add 每题附加一个追问
```

## 数据存储与备份

所有数据在 `~/.daily-q/`：

| 文件 | 内容 |
|------|------|
| `config.json` | 配置 |
| `daily-q.db` | SQLite 数据库 |

5 张表：`questions`（每日一题）、`answers`（答题记录）、`topic_mastery`（掌握度）、`summary_cache`（学习总结）、`rules`（出题规则）。

备份 = 直接复制这两个文件。删除 `~/.daily-q` 即完全重置；也可用命令 `dq config reset`（交互确认后清空全部配置与数据）。

## 常见问题

**Q：提示"未配置 API 密钥"？**
运行 `dq config ai` 一键配置。

**Q：提示"请先设置画像"？**
运行 `dq profile setup`。

**Q：`dq answer` 提示"今天还没有题目"？**
先运行 `dq` 生成今日题目。

**Q：提示"今天已答过"？**
每天一题一答。查看结果用 `dq stats`，重做需 `dq skip` 后重新出题。

**Q：`dq` 一直报"LLM 请求失败"？**
检查 `base_url` 与 `model` 是否匹配、网络是否可达、API Key 是否有效。DeepSeek 偶发瞬时故障，重试即可。

**Q：如何接入其他模型？**
见[接入其他模型](#接入其他模型)。
