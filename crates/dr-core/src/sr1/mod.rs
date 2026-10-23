//! SimpleRockets 1 PC 本体的数据读取、构造及保存语义。
//!
//! 本层与编辑器的严格、安全读写入口分离，不隐式改变已有文档或编辑事务。
//! 原生行为按逐项证据实现；不等同于完整物理模拟，也不承诺原 XML 词法保真。

pub mod catalog;
pub mod parts;
pub mod scalars;
pub mod ship;
pub mod staging;
pub mod xml;

/// 本体数据的 XML 传输或已明确的数据前置条件错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("SR1 XML 处理失败：{0}")]
    Xml(String),
    #[error("SR1 根元素应为 {expected}，实际为 {found}")]
    WrongRoot {
        expected: &'static str,
        found: String,
    },
    #[error("SR1 数据无法处理：{0}")]
    InvalidData(String),
}

pub type Result<T> = std::result::Result<T, Error>;
