//! 终端颜色输出（设计文档 4.7）

use colored::Colorize;

/// 青色，用于标题行
pub fn title(text: &str) -> String {
    text.cyan().to_string()
}

/// 蓝色，用于字段名（"得分:"）
pub fn label(text: &str) -> String {
    text.blue().to_string()
}

/// 黄色，用于高亮值
pub fn highlight(text: &str) -> String {
    text.yellow().to_string()
}

/// 暗色，用于提示
pub fn dim(text: &str) -> String {
    text.dimmed().to_string()
}

/// 绿色，用于正面信息
pub fn good(text: &str) -> String {
    text.green().to_string()
}

/// 紫色，用于警告
pub fn warn(text: &str) -> String {
    text.purple().to_string()
}

/// 红色，用于错误
pub fn bad(text: &str) -> String {
    text.red().to_string()
}
