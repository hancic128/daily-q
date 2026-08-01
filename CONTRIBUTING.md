# 贡献指南

感谢你考虑为 daily-q 贡献代码！请先阅读 [README](README.md) 与 [设计文档](docs/design.md) 了解项目。

## 开发环境

- Rust 工具链 ≥ 1.88
- macOS / Linux（Windows 未经测试，欢迎补全）

```bash
git clone https://github.com/Angryshark128/daily-q && cd daily-q
cargo build          # 构建 debug 版本（产物 target/debug/dq）
cargo run -- --help  # 查看命令
```

## 代码约定

- 遵循 [docs/design.md](docs/design.md) 的模块划分：config / db / llm / quiz / summary / profile / models / lang / display / main
- **所有用户可见文案必须走 `lang.rs` 的 `texts!` 宏**（中英文各一份），禁止在业务代码硬编码文案
- 错误统一用 `anyhow::Result` + `Context`；后台线程（summary）失败只打 stderr，不 panic
- 核心数据结构定义在 `models.rs`，不散落模块内部
- 提交前保证 `cargo build` 无警告、`cargo clippy` 干净

## 测试

```bash
# 单元/冒烟测试：mock LLM 验证 55+ 条断言（无需真实 API）
bash scripts/verify.sh

# 手动端到端（需 API Key）
dq config ai         # 配置服务商 + Key
dq profile setup     # 设置画像
dq && dq answer      # 出题 + 答题
```

新增功能时请在 [docs/test-cases.md](docs/test-cases.md) 补充对应用例，并同步 `scripts/verify.sh`。

## 提交 PR

1. Fork 仓库并创建特性分支：`git checkout -b feat/xxx`
2. 提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)：
   - `feat:` 新功能 / `fix:` 修复 / `docs:` 文档 / `refactor:` 重构 / `test:` 测试
3. 运行 `cargo fmt`、`cargo clippy`、`bash scripts/verify.sh` 全部通过
4. 发起 PR，描述改动内容与验证方式

## 翻译

新增文案时需同时提供中英文。如需调整现有文案，请保持 `lang.rs` 的 `texts!` 宏结构。
