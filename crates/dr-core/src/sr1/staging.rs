//! SR1 PC 本体分级语义；与编辑器的严格分级模型相互独立。
//!
//! `moved` 是加载后重新建立的内部状态：PC 忽略 XML 中的同名属性，
//! 对成功解析、且属于当前部件树的引用一律置为 `true`。

use crate::sr1::scalars::{read_i32, write_bool};
use crate::sr1::xml::{self, Element};
use crate::sr1::{Error, Result};

/// 一个分级动作引用。`part_id` 使用原生 PC 的有符号 32 位 ID。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedPart {
    pub part_id: i32,
    pub moved: bool,
}

/// 空步骤也是有效步骤，过滤失效引用时不会删除它。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StageStep {
    pub parts: Vec<StagedPart>,
}

/// Pod 自己的分级表；`current_stage` 不执行范围裁剪。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Staging {
    pub current_stage: i32,
    pub steps: Vec<StageStep>,
}

impl Staging {
    /// 先保留全部 ID；船体建树完成后调用 [`Self::retain_part_ids`]。
    pub fn from_element(element: &Element) -> Result<Self> {
        Self::from_element_with_current_stage(element, 0)
    }

    /// 旧格式将 Ship 的 `currentStage` 作为初值；Staging 自己的有效属性优先。
    pub fn from_element_with_current_stage(element: &Element, current_stage: i32) -> Result<Self> {
        if element.name != "Staging" {
            return Err(Error::WrongRoot {
                expected: "Staging",
                found: element.name.clone(),
            });
        }
        let mut steps = Vec::new();
        // TinyXML FirstChildElement("Step") 之后使用无名称 NextSiblingElement。
        // 因此首个 Step 后的所有元素兄弟都会占一个步骤。
        if let Some(start) = element
            .children
            .iter()
            .position(|child| child.name == "Step")
        {
            for step in &element.children[start..] {
                let mut parts = Vec::new();
                if let Some(start) = step
                    .children
                    .iter()
                    .position(|child| child.name == "Activate")
                {
                    for activation in &step.children[start..] {
                        parts.push(StagedPart {
                            part_id: read_i32(activation.attr("Id"), 0),
                            moved: true,
                        });
                    }
                }
                steps.push(StageStep { parts });
            }
        }
        Ok(Self {
            current_stage: read_i32(element.attr("currentStage"), current_stage),
            steps,
        })
    }

    pub fn from_xml(source: &str) -> Result<Self> {
        Self::from_element(&xml::parse(source)?)
    }

    /// 仅保留当前部件树中存在的 ID，保留顺序、重复动作与空步骤。
    ///
    /// PC 按部件遍历顺序取首个同 ID 对象；本层的引用以 ID 表示，不合并重复动作。
    pub fn retain_part_ids(&mut self, part_ids: &[i32]) {
        for step in &mut self.steps {
            step.parts.retain(|part| part_ids.contains(&part.part_id));
        }
    }

    /// 写出当前内部 `moved`，并非强制写 `1`；下一次加载仍会重新置为 true。
    pub fn to_element(&self) -> Element {
        let mut element = Element::new("Staging");
        element.set_attr("currentStage", self.current_stage.to_string());
        for step in &self.steps {
            let mut child = Element::new("Step");
            for part in &step.parts {
                let mut activation = Element::new("Activate");
                activation.set_attr("Id", part.part_id.to_string());
                activation.set_attr("moved", write_bool(part.moved));
                child.children.push(activation);
            }
            element.children.push(child);
        }
        element
    }

    pub fn to_xml(&self) -> Result<String> {
        xml::to_string(&self.to_element())
    }
}

#[cfg(test)]
mod tests;
