# daily-q 测试用例

> 状态图例：`[ ]` 待验证 / `[x]` 通过 / `[!]` 失败
>
> LLM 相关用例通过本地 mock server（`docs/mock_openai.py`，模拟 `/v1/chat/completions`）验证，
> 不依赖真实 API。mock 按 system prompt 关键词返回固定 JSON。
>
> 测试环境隔离：`HOME=/tmp/dq-test-home`，不污染真实 `~/.daily-q`。
> 所有用例按编号线性执行（状态前后依赖）。

## 验证记录

### mock 验证（2026-08-01）
- 全部 38 条用例通过（`bash /tmp/dq-verify.sh` → `PASS=51 FAIL=0`，断言数含复合断言）。
- 环境：rustc 1.88.0 / macOS，mock server 本地端口 8765。

### 真实 LLM 验证（2026-08-01，DeepSeek deepseek-chat）
- 中文出题 ✅：返回单题，贴合画像（分布式锁，对应"分布式/性能优化"）
- 中文评判 ✅：68 分 + 详细反馈 + 完整参考答案
- 后台总结 ✅：识别 5 个薄弱项 + 5 条知识关联 + 6 条建议，落库 `summary_cache`
- 掌握度分级 ✅：judge tags → 强项；summary weak_topics → 弱项（正确标记）
- 已答时 `--lang en` ✅：保留原语言题目与答案，英文提示"original language"
- 英文出题 ✅：`--lang en` 生成全英文题目
- 过程中发现并修复 2 个 mock 掩盖的真实 bug：
  1. **三个 LLM prompt 缺"返回 JSON 格式"约束** → LLM 自由发挥返回题目数组，反序列化失败；已在 system prompt 补充明确 JSON schema
  2. **英文模式下 system 指令被中文 user 上下文盖过** → 题目仍为中文；已在 user prompt 增加"目标语言"项 + system 强化英文强制
- 注：期间偶发 DeepSeek API 瞬时故障（TLS eof / 空响应），重试即恢复，非代码问题。

## A. 配置类

| # | 命令 | 前置 | 预期结果 | 状态 |
|---|------|------|----------|------|
| TC-01 | `dq config` | 无配置 | 显示默认配置：model=deepseek-chat、base_url=未设置、focus 默认 5 项、API Key 未设置，界面中文 | [x] |
| TC-02 | `dq config api-key sk-test` | TC-01 | 保存成功提示"API 密钥已保存" | [x] |
| TC-03 | `dq config model gpt-4o-mini` | TC-02 | "模型已设置"，config.json 中 model 变更 | [x] |
| TC-04 | `dq config base-url http://127.0.0.1:8765/v1`（直填 URL 兼容路径，触发连通测试） | TC-03 | "API 地址已设置" | [x] |
| TC-05 | `dq config focus 网络,数据库` | TC-04 | "关注方向已设置"，focus 覆盖为两项 | [x] |
| TC-07 | 部分更新不覆盖 | TC-03 之后 | 只改 focus 后 model 仍保留之前的值（TC-05 验证） | [x] |

## B. 画像类

| # | 命令 | 前置 | 预期结果 | 状态 |
|---|------|------|----------|------|
| TC-09 | `dq profile` | 未设置 | 显示"未设置" | [x] |
| TC-10 | `dq profile setup`（输入 `3,1,2,3,1,4`） | TC-05 | 拼接保存 `3-5年 \| 后端 \| Java \| 高级 \| 分布式、性能优化` | [x] |
| TC-11 | `dq profile` | TC-10 | 显示已保存画像 | [x] |
| TC-44 | `dq profile setup` 岗位多选（岗位输入 `1,2` = 后端、前端） | 画像含"后端、前端"，技术栈取两岗位并集（手动验证） | [x] |

## C. 规则类

| # | 命令 | 前置 | 预期结果 | 状态 |
|---|------|------|----------|------|
| TC-12 | `dq rule add 只出场景题` | - | "规则已添加" | [x] |
| TC-13 | `dq rule add 不要算法题` | TC-12 | 添加第二条 | [x] |
| TC-14 | `dq rule list` | TC-13 | 列出 2 条，带 id | [x] |
| TC-15 | `dq rule remove 1` | TC-14 | 删除 id=1，"规则已删除" | [x] |
| TC-16 | `dq rule remove 999` | TC-15 | 报错"规则不存在" | [x] |

## D. 出题/答题类（需 LLM，mock）

| # | 命令 | 前置 | 预期结果 | 状态 |
|---|------|------|----------|------|
| TC-17 | `dq`（今日无题） | TC-11+TC-16 | 调 mock 生成题目并显示；questions 表新增当日记录（lang=zh） | [x] |
| TC-18 | `dq`（今日已有题） | TC-17 | 直接显示，不重复生成（questions 表今日仍 1 条） | [x] |
| TC-19 | `dq --difficulty hard` | TC-18 | 未答 → 删除旧题重生成（今日仍 1 条） | [x] |
| TC-20 | `dq answer`（fake editor 写回答） | TC-19 | 评分 75+反馈+参考答案；answers 表新增；topic_mastery 更新 | [x] |
| TC-21 | `dq answer`（今日已答） | TC-20 | 报错"今天已答过" | [x] |
| TC-22 | `dq stats`（有答题记录） | TC-20 | 掌握度列表按正确率升序，图标/等级正确 | [x] |
| TC-23 | `dq skip` 后 `dq answer` | TC-21 | skip 删今日题 → answer 报错"今天还没有题目" | [x] |
| TC-24 | `dq skip`（今日无题） | TC-23 | 无操作，输出"完成" | [x] |

## E. 说明

> 已移除语言切换功能：全部使用中文（题目、UI、标签、评估均中文）。早期 `--lang en` 相关验证记录保留为历史存档。

## F. 错误处理类

| # | 场景 | 预期结果 | 状态 |
|---|------|----------|------|
| TC-33 | 无 api-key 运行 `dq` | 报错"未配置 API 密钥，请运行 dq config ai" | [x] |
| TC-34 | 无画像运行 `dq` | 报错"请先设置画像" | [x] |
| TC-35 | `dq stats`（无记录） | 显示"暂无答题记录"，不 panic | [x] |
| TC-36 | 后台总结失败（mock server 停止） | 答题成功，stderr 打印 summary 错误，主流程不受影响 | [x] |
| TC-37 | `dq --difficulty invalid` | clap 校验拒绝非法值 | [x] |
| TC-38 | `dq rule add`（缺参数） | clap 报缺参 | [x] |
| TC-39 | 无 base-url（key 已有）运行 `dq` | 报错"未配置 AI 服务商，请运行 dq config ai" | [x] |
| TC-40 | `dq config base-url list` | 显示 7 个服务商预设（含 siliconflow） | [x] |
| TC-41 | `dq config model <name>`（三要素齐） | 先测试连通 → 通过后"模型已设置" | [x] |
| TC-42 | 配置错误模型（真实 LLM，如 gpt-4o-mini 配 deepseek） | 测试失败报错"连接测试失败，配置未保存"，config 不落盘（真实环境已验证） | [x] |
| TC-43 | `dq config reset`（输入 y / n） | y → 清空 ~/.daily-q/ 全部配置与数据；n → 取消保留 | [x] |

## G. 执行方法

一键运行（项目根目录，需先 `cargo build`）：

```bash
bash scripts/verify.sh
```

手动步骤：

```bash
# 1) 启动 mock server（后台）
python3 docs/mock_openai.py &

# 2) 隔离 HOME
export HOME=/tmp/dq-test-home
DQ=target/debug/dq
DB=$HOME/.daily-q/daily-q.db

# 3) fake editor（dq answer 用，直接写入回答）
cat > /tmp/dq-fake-editor.sh <<'EOF'
#!/bin/bash
cat > "$1" <<'E2'
# TCP 三次握手: medium
# 请描述TCP三次握手的过程

SYN, SYN-ACK, ACK 完成握手
E2
EOF
chmod +x /tmp/dq-fake-editor.sh

# 4) 示例断言
$DQ config | grep -q "deepseek-chat" && echo PASS
sqlite3 "$DB" "SELECT topic,lang FROM questions"
```
