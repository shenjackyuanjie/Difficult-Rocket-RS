//! 面向 SR1 属性数据的有序 XML 元素适配。
//!
//! 属性与子元素保序，实体解码一次；注释、声明和原始排版不进入数据模型。
//! 本体数据以属性为主；text 聚合直接文本，不提供一般混合内容的词法往返。
//! 本模块不做数值转换，缺省/错误数值如何处理由本体语义模块决定。

use super::{Error, Result};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};

/// 防止构造或输出异常深度的输入耗尽调用栈；不是游戏格式的业务字段限制。
pub const MAX_DEPTH: usize = 256;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Element>,
    pub text: String,
}

impl Element {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// 修改既有属性不改变位置；新增属性追加到末尾。
    pub fn set_attr(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        let name = name.into();
        let value = value.into();
        if let Some((_, current)) = self.attributes.iter_mut().find(|(key, _)| key == &name) {
            *current = value;
        } else {
            self.attributes.push((name, value));
        }
        self
    }

    pub fn remove_attr(&mut self, name: &str) -> Option<String> {
        let index = self.attributes.iter().position(|(key, _)| key == name)?;
        Some(self.attributes.remove(index).1)
    }

    pub fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|child| child.name == name)
    }

    pub fn child_mut(&mut self, name: &str) -> Option<&mut Self> {
        self.children.iter_mut().find(|child| child.name == name)
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Self> {
        self.children.iter().filter(move |child| child.name == name)
    }

    pub fn require_name(&self, expected: &'static str) -> Result<()> {
        if self.name == expected {
            Ok(())
        } else {
            Err(Error::WrongRoot {
                expected,
                found: self.name.clone(),
            })
        }
    }
}

fn xml_error(error: impl std::fmt::Display) -> Error {
    Error::Xml(error.to_string())
}

fn read_element(start: &BytesStart<'_>) -> Result<Element> {
    let mut element = Element::new(start.name().as_ref());
    for attr in start.attributes() {
        let attr = attr.map_err(xml_error)?;
        element.attributes.push((
            attr.key.as_ref().to_owned(),
            quick_xml::escape::unescape(attr.value.as_ref())
                .map_err(xml_error)?
                .into_owned(),
        ));
    }
    Ok(element)
}

fn attach(element: Element, stack: &mut [Element], root: &mut Option<Element>) -> Result<()> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(element);
    } else if root.is_some() {
        return Err(Error::Xml("文档包含多个根元素".into()));
    } else {
        *root = Some(element);
    }
    Ok(())
}

pub fn parse(data: &str) -> Result<Element> {
    let mut reader = Reader::from_str(data);
    reader.config_mut().check_comments = true;
    let mut stack: Vec<Element> = Vec::new();
    let mut root = None;
    loop {
        match reader.read_event().map_err(xml_error)? {
            Event::Start(start) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(Error::Xml("XML 嵌套超过支持深度".into()));
                }
                stack.push(read_element(&start)?);
            }
            Event::Empty(start) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(Error::Xml("XML 嵌套超过支持深度".into()));
                }
                attach(read_element(&start)?, &mut stack, &mut root)?;
            }
            Event::End(_) => {
                let element = stack
                    .pop()
                    .ok_or_else(|| Error::Xml("多余的闭合标签".into()))?;
                attach(element, &mut stack, &mut root)?;
            }
            Event::Text(text) => {
                if let Some(parent) = stack.last_mut() {
                    parent.text.push_str(text.as_ref());
                } else if !text
                    .as_ref()
                    .bytes()
                    .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
                {
                    return Err(Error::Xml("根元素外存在文本".into()));
                }
            }
            Event::CData(text) => {
                let parent = stack
                    .last_mut()
                    .ok_or_else(|| Error::Xml("根元素外存在 CDATA".into()))?;
                parent.text.push_str(text.as_ref());
            }
            Event::GeneralRef(reference) => {
                let parent = stack
                    .last_mut()
                    .ok_or_else(|| Error::Xml("根元素外存在字符引用".into()))?;
                let value = quick_xml::escape::unescape(&format!("&{};", reference.as_ref()))
                    .map_err(xml_error)?
                    .into_owned();
                parent.text.push_str(&value);
            }
            Event::Eof => {
                if !stack.is_empty() {
                    return Err(Error::Xml("XML 元素尚未闭合".into()));
                }
                return root.ok_or_else(|| Error::Xml("文档缺少根元素".into()));
            }
            Event::Decl(_) | Event::PI(_) | Event::Comment(_) | Event::DocType(_) => {}
        }
    }
}

/// PC TinyXML 打印器的五字符转义；不调用 0.42 的属性转义以免额外改写控制字符。
fn escape(value: &str, output: &mut String) -> Result<()> {
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '\'' => output.push_str("&apos;"),
            '"' => output.push_str("&quot;"),
            ch if (ch as u32) < 0x20 && !matches!(ch, '\t' | '\n' | '\r') => {
                return Err(Error::Xml("XML 不支持该控制字符".into()));
            }
            _ => output.push(ch),
        }
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let start = |ch: char| ch.is_ascii_alphabetic() || ch == '_' || ch == ':' || ch as u32 >= 0x80;
    chars.next().is_some_and(start)
        && chars.all(|ch| start(ch) || ch.is_ascii_digit() || matches!(ch, '-' | '.'))
}

fn write_element(element: &Element, output: &mut String, depth: usize) -> Result<()> {
    if depth >= MAX_DEPTH {
        return Err(Error::Xml("XML 嵌套超过支持深度".into()));
    }
    if !valid_name(&element.name) {
        return Err(Error::Xml("XML 元素名称无效".into()));
    }
    output.push('<');
    output.push_str(&element.name);
    let mut seen = std::collections::HashSet::new();
    for (key, value) in &element.attributes {
        if !valid_name(key) || !seen.insert(key) {
            return Err(Error::Xml("XML 属性名称无效或重复".into()));
        }
        output.push(' ');
        output.push_str(key);
        output.push_str("=\"");
        escape(value, output)?;
        output.push('"');
    }
    if element.children.is_empty() && element.text.is_empty() {
        output.push_str("/>");
    } else {
        output.push('>');
        escape(&element.text, output)?;
        for child in &element.children {
            write_element(child, output, depth + 1)?;
        }
        output.push_str("</");
        output.push_str(&element.name);
        output.push('>');
    }
    Ok(())
}

/// 紧凑输出元素，不插入声明、缩进或重排属性；原文排版不属于保存合同。
pub fn to_string(element: &Element) -> Result<String> {
    let mut output = String::new();
    write_element(element, &mut output, 0)?;
    Ok(output)
}

#[cfg(test)]
mod tests;
