//! 画像设置（设计文档 4.8）
//!
//! 交互式引导用户填写面试画像（工作年限、岗位方向、技术栈、目标级别、重点方向），
//! 拼接成字符串写入 config.json，如 "3-5年 | 后端 | Go | 高级 | 分布式"。
//! 选项硬编码，中英文文案走 lang.rs。

use std::io::BufRead;

use anyhow::Result;

use crate::config::Config;
use crate::display;
use crate::lang::TEXTS;

const YEARS: &[&str] = &["<1年", "1-2年", "3-5年", "5-8年", "8年+"];
const ROLES: &[&str] = &[
    "后端",
    "前端",
    "数据",
    "算法",
    "架构师",
    "AI",
    "测试",
    "运维",
    "安全",
    "移动",
    "产品",
];
const LEVELS: &[&str] = &["初级", "中级", "高级", "专家"];
const FOCUS: &[&str] = &[
    "分布式",
    "高并发",
    "微服务",
    "性能优化",
    "架构设计",
    "数据库",
    "缓存",
    "消息队列",
];
const OTHER_STACKS: &[&str] = &["Go", "Java", "Python", "C++", "Rust", "JavaScript"];

/// 11 种岗位 × 各自技术栈
const TECH_STACKS: &[(&str, &[&str])] = &[
    ("后端", &["Go", "Java", "Python", "C++", "Rust", "Node.js"]),
    (
        "前端",
        &["React", "Vue", "Angular", "TypeScript", "Node.js"],
    ),
    (
        "数据",
        &["SQL", "Python", "Spark", "Flink", "Hive", "Doris"],
    ),
    (
        "算法",
        &["Python", "C++", "机器学习", "深度学习", "NLP", "CV"],
    ),
    (
        "架构师",
        &["系统架构", "分布式", "微服务", "高并发", "Go", "Java"],
    ),
    (
        "AI",
        &[
            "Python",
            "PyTorch",
            "TensorFlow",
            "机器学习",
            "深度学习",
            "RAG",
            "LLM",
        ],
    ),
    (
        "测试",
        &["Python", "Java", "自动化测试", "性能测试", "接口测试"],
    ),
    (
        "运维",
        &["Linux", "Docker", "Kubernetes", "CI/CD", "云原生", "监控"],
    ),
    ("安全", &["渗透测试", "Web安全", "二进制安全", "安全审计"]),
    (
        "移动",
        &["Android", "iOS", "Flutter", "React Native", "Kotlin"],
    ),
    ("产品", &["需求分析", "原型设计", "数据分析", "项目管理"]),
];

/// 交互式画像引导（profile::run_profile_setup）
pub fn run_profile_setup() -> Result<()> {
    println!("{}", display::title(TEXTS.profile_setup_title()));
    println!("{}", display::dim(TEXTS.profile_setup_desc()));
    println!();

    let years = prompt_select(TEXTS.q_experience(), YEARS)?;
    let roles = prompt_select_multi(TEXTS.q_role(), ROLES)?;
    let stacks = get_tech_options(&roles);
    let tech = prompt_select_multi(TEXTS.q_tech(), &stacks)?.join("、");
    let level = prompt_select(TEXTS.q_level(), LEVELS)?;
    let focus = prompt_select_multi(TEXTS.q_focus(), FOCUS)?.join("、");

    let profile = format!(
        "{years} | {} | {tech} | {level} | {focus}",
        roles.join("、")
    );
    println!();
    println!("{}", display::label(TEXTS.confirm_profile()));
    println!("{}", display::highlight(&profile));
    Config::set_profile(&profile)?;
    println!("{}", display::good(TEXTS.profile_saved()));
    Ok(())
}

/// 按岗位（可多个）取技术栈并集；未匹配则用通用技术栈
fn get_tech_options(roles: &[String]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for role in roles {
        for (r, stacks) in TECH_STACKS {
            if *r == role {
                for s in *stacks {
                    if !out.contains(s) {
                        out.push(s);
                    }
                }
            }
        }
    }
    if out.is_empty() {
        OTHER_STACKS.to_vec()
    } else {
        out
    }
}

/// 单选项：循环直到有效输入，支持"其他"手动输入
fn prompt_select(question: &str, options: &[&str]) -> Result<String> {
    loop {
        println!(
            "{} ({})",
            display::label(question),
            display::dim(TEXTS.enter_number())
        );
        for (i, opt) in options.iter().enumerate() {
            println!("  {}. {}", i + 1, opt);
        }
        println!("  {}. {}", options.len() + 1, TEXTS.other());
        let line = read_line()?;
        match line.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= options.len() => return Ok(options[n - 1].to_string()),
            Ok(n) if n == options.len() + 1 => {
                let custom = read_line()?;
                if custom.trim().is_empty() {
                    println!("{}", display::warn(TEXTS.invalid_input()));
                    continue;
                }
                return Ok(custom.trim().to_string());
            }
            _ => println!("{}", display::warn(TEXTS.invalid_input())),
        }
    }
}

/// 多选项：逗号分隔，如 "1,3"，返回选中项列表
fn prompt_select_multi(question: &str, options: &[&str]) -> Result<Vec<String>> {
    loop {
        println!(
            "{} ({})",
            display::label(question),
            display::dim(TEXTS.enter_number())
        );
        for (i, opt) in options.iter().enumerate() {
            println!("  {}. {}", i + 1, opt);
        }
        let line = read_line()?;
        let idxs: Vec<usize> = line
            .split(',')
            .map(|s| s.trim().parse::<usize>())
            .filter_map(|r| r.ok())
            .collect();
        let valid = !idxs.is_empty() && idxs.iter().all(|n| (1..=options.len()).contains(n));
        if !valid {
            println!("{}", display::warn(TEXTS.invalid_input()));
            continue;
        }
        let selected: Vec<String> = idxs.iter().map(|n| options[n - 1].to_string()).collect();
        return Ok(selected);
    }
}

fn read_line() -> Result<String> {
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(line)
}
