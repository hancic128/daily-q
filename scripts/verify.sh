#!/bin/bash
# daily-q 验证脚本 —— 对应 docs/test-cases.md 的 TC-01 ~ TC-38
set -u
cd "$(dirname "$0")/.." || exit 1   # 定位到项目根
DQ=target/debug/dq
export HOME=/tmp/dq-test-home
DB=$HOME/.daily-q/daily-q.db
SQLITE=$(command -v sqlite3)
TODAY=$(date +%F)
PASS=0; FAIL=0

check() {
  local desc="$1" actual="$2" expect="$3"
  if printf '%s' "$actual" | grep -qF "$expect"; then
    echo "PASS  $desc"
    PASS=$((PASS+1))
  else
    echo "FAIL  $desc"
    echo "      实际: $(printf '%s' "$actual" | head -3 | tr '\n' ' ')"
    echo "      期望含: $expect"
    FAIL=$((FAIL+1))
  fi
}

# fake editor：dq answer 时直接把回答写入临时文件
cat > /tmp/dq-fake-editor.sh <<'EOF'
#!/bin/bash
cat > "$1" <<'E2'
# TCP 三次握手: medium
# 请描述TCP三次握手的过程

SYN, SYN-ACK, ACK 完成握手
E2
EOF
chmod +x /tmp/dq-fake-editor.sh
export EDITOR=/tmp/dq-fake-editor.sh

# 重置隔离环境 + 启动 mock
rm -rf "$HOME"; mkdir -p "$HOME"
python3 docs/mock_openai.py >/tmp/dq-mock.log 2>&1 &
MOCK_PID=$!
sleep 1

echo "== A. 配置 =="
OUT=$($DQ config)
check "TC-01 默认 model" "$OUT" "deepseek-chat"
check "TC-01 配置界面中文" "$OUT" "数据目录"

$DQ config api-key sk-test >/dev/null 2>&1
OUT=$($DQ config 2>&1)
check "TC-02 api-key 已保存" "$OUT" "***"

$DQ config model gpt-4o-mini >/dev/null 2>&1
grep -q '"model": "gpt-4o-mini"' "$HOME/.daily-q/config.json" && { echo "PASS  TC-03 model 落盘"; PASS=$((PASS+1)); } || { echo "FAIL  TC-03 model 落盘"; FAIL=$((FAIL+1)); }

$DQ config base-url http://127.0.0.1:8765 >/dev/null 2>&1
grep -q '127.0.0.1' "$HOME/.daily-q/config.json" && { echo "PASS  TC-04 base-url 落盘"; PASS=$((PASS+1)); } || { echo "FAIL  TC-04 base-url 落盘"; FAIL=$((FAIL+1)); }

$DQ config focus 网络,数据库 >/dev/null 2>&1
OUT=$($DQ config)
check "TC-05 focus 覆盖" "$OUT" "网络、数据库"
check "TC-07 部分更新保留 model" "$OUT" "gpt-4o-mini"

OUT=$($DQ config)


echo "== B. 画像 =="
OUT=$($DQ profile)
check "TC-09 画像未设置" "$OUT" "未设置"

OUT=$(printf '3\n1\n2\n3\n1,4\n' | $DQ profile setup)
check "TC-10 profile 保存" "$OUT" "画像已保存"
grep -q '3-5年 | 后端 | Java | 高级 | 分布式、性能优化' "$HOME/.daily-q/config.json" && { echo "PASS  TC-10 画像拼接正确"; PASS=$((PASS+1)); } || { echo "FAIL  TC-10 画像拼接"; FAIL=$((FAIL+1)); }

OUT=$($DQ profile)
check "TC-11 画像显示" "$OUT" "3-5年"

echo "== C. 规则 =="
$DQ rule add 只出场景题 >/dev/null 2>&1
$DQ rule add 不要算法题 >/dev/null 2>&1
OUT=$($DQ rule list)
check "TC-12/13/14 规则列表" "$OUT" "只出场景题"
check "TC-14 两条规则" "$OUT" "不要算法题"
OUT=$($DQ rule remove 1)
check "TC-15 删除规则" "$OUT" "规则已删除"
OUT=$($DQ rule remove 999 2>&1)
check "TC-16 删除不存在" "$OUT" "规则不存在"

echo "== D. 出题/答题 =="
OUT=$($DQ)
check "TC-17 出题显示" "$OUT" "请描述TCP三次握手的过程"
check "TC-17 questions.lang=zh" "$($SQLITE "$DB" "SELECT lang FROM questions WHERE date='$TODAY'")" "zh"

N1=$($SQLITE "$DB" "SELECT COUNT(*) FROM questions WHERE date='$TODAY'")
$DQ >/dev/null 2>&1
N2=$($SQLITE "$DB" "SELECT COUNT(*) FROM questions WHERE date='$TODAY'")
check "TC-18 已有题不重复生成 ($N1->$N2)" "$N2" "$N1"

$DQ --difficulty hard >/dev/null 2>&1
N3=$($SQLITE "$DB" "SELECT COUNT(*) FROM questions WHERE date='$TODAY'")
check "TC-19 重生成今日仍一条" "$N3" "1"

OUT=$($DQ answer 2>&1)
check "TC-20 评分 75" "$OUT" "75"
check "TC-20 反馈显示" "$OUT" "整体正确"
check "TC-20 answer 落库" "$($SQLITE "$DB" "SELECT score FROM answers")" "75"
sleep 2   # 等待后台 summary 完成
check "TC-20 mastery 更新(含总结标记)" "$($SQLITE "$DB" "SELECT total||'/'||correct FROM topic_mastery WHERE topic='TCP'")" "2/1"

OUT=$($DQ answer 2>&1)
check "TC-21 已答报错" "$OUT" "今天已答过"

OUT=$($DQ stats 2>&1)
check "TC-22 stats 掌握度" "$OUT" "TCP"
check "TC-22 等级文案" "$OUT" "一般"

$DQ skip >/dev/null 2>&1
OUT=$($DQ answer 2>&1)
check "TC-23 无题报错" "$OUT" "今天还没有题目"

OUT=$($DQ skip 2>&1)
check "TC-24 无题 skip" "$OUT" "完成"

echo "== F. 错误处理 =="
python3 -c "import json,os; p=os.path.expanduser('~/.daily-q/config.json'); d=json.load(open(p)); d['api_key']=''; json.dump(d,open(p,'w'),ensure_ascii=False)"
OUT=$($DQ 2>&1)
check "TC-33 无 key 报错" "$OUT" "未配置 API 密钥"
python3 -c "import json,os; p=os.path.expanduser('~/.daily-q/config.json'); d=json.load(open(p)); d['api_key']='sk-test'; json.dump(d,open(p,'w'),ensure_ascii=False)"

python3 -c "import json,os; p=os.path.expanduser('~/.daily-q/config.json'); d=json.load(open(p)); d.pop('profile',None); json.dump(d,open(p,'w'),ensure_ascii=False)"
OUT=$($DQ 2>&1)
check "TC-34 无画像报错" "$OUT" "请先设置画像"
printf '3\n1\n2\n3\n1,4\n' | $DQ profile setup >/dev/null 2>&1

rm -f "$DB"
OUT=$($DQ stats 2>&1)
check "TC-35 暂无记录" "$OUT" "暂无答题记录"

$DQ >/dev/null 2>&1    # 出题（mock 正常）
curl -s http://127.0.0.1:8765/set-fail/1 >/dev/null
OUT=$($DQ answer 2>&1)
check "TC-36 summary 失败不影响答题" "$OUT" "75"
curl -s http://127.0.0.1:8765/set-fail/0 >/dev/null
sleep 1

OUT=$($DQ --difficulty invalid 2>&1)
check "TC-37 非法难度被拒" "$OUT" "invalid"

OUT=$($DQ rule add 2>&1)
check "TC-38 rule add 缺参报错" "$OUT" "error"

# TC-40/41 服务商预设 + 配置测试校验
OUT=$($DQ config base-url list 2>&1)
check "TC-40 服务商列表" "$OUT" "siliconflow"
OUT=$($DQ config model gpt-4o-mini 2>&1)
check "TC-41 配置测试通过后保存" "$OUT" "模型已设置"

# TC-43 重置（y 清空 / n 保留）
echo "n" | $DQ config reset >/dev/null 2>&1
if [ -e "$HOME/.daily-q/config.json" ]; then echo "PASS  TC-43 reset 取消保留"; PASS=$((PASS+1)); else echo "FAIL  TC-43 reset 取消保留"; FAIL=$((FAIL+1)); fi
echo "y" | $DQ config reset >/dev/null 2>&1
if [ ! -e "$HOME/.daily-q/config.json" ]; then echo "PASS  TC-43 reset 确认清空"; PASS=$((PASS+1)); else echo "FAIL  TC-43 reset 确认清空"; FAIL=$((FAIL+1)); fi

kill $MOCK_PID 2>/dev/null
echo "======================================"
echo "RESULT: PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ] && echo "ALL GREEN" || echo "HAS FAILURES"
