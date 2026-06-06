use crate::domain::agent::AgentTask;
use crate::domain::commit::CurrentState;
use crate::domain::recovery::RecoveryRequest;
use crate::utils::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 自然语言指令解析器
pub struct NaturalLanguageParser;

/// 解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCommand {
    pub command_type: CommandType,
    pub intent: String,
    pub entities: HashMap<String, Entity>,
    pub confidence: f32,
    pub raw_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommandType {
    Recovery,
    Query,
    Merge,
    Deploy,
    Analyze,
    Rollback,
    Search,
    Tag,
    Branch,
}

impl std::fmt::Display for CommandType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandType::Recovery => write!(f, "Recovery"),
            CommandType::Query => write!(f, "Query"),
            CommandType::Merge => write!(f, "Merge"),
            CommandType::Deploy => write!(f, "Deploy"),
            CommandType::Analyze => write!(f, "Analyze"),
            CommandType::Rollback => write!(f, "Rollback"),
            CommandType::Search => write!(f, "Search"),
            CommandType::Tag => write!(f, "Tag"),
            CommandType::Branch => write!(f, "Branch"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_type: EntityType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityType {
    CommitHash,
    BranchName,
    TagName,
    ModuleName,
    FileName,
    TimeRange,
    Author,
    Keyword,
}

impl NaturalLanguageParser {
    pub fn new() -> Self {
        Self
    }

    /// 基于规则的简单解析（不依赖 LLM）
    pub fn parse(&self, text: &str) -> ParsedCommand {
        let text_lower = text.to_lowercase();
        let command_type = self.infer_command_type(&text_lower);
        let entities = self.extract_entities(text);

        ParsedCommand {
            command_type,
            intent: text.to_string(),
            entities,
            confidence: 0.85,
            raw_text: text.to_string(),
        }
    }

    fn infer_command_type(&self, text: &str) -> CommandType {
        if text.contains("恢复") || text.contains("恢复回来") || text.contains("恢复") || text.contains("recover") {
            CommandType::Recovery
        } else if text.contains("合并") || text.contains("merge") {
            CommandType::Merge
        } else if text.contains("部署") || text.contains("deploy") {
            CommandType::Deploy
        } else if text.contains("回滚") || text.contains("rollback") {
            CommandType::Rollback
        } else if text.contains("搜索") || text.contains("search") || text.contains("查找") {
            CommandType::Search
        } else if text.contains("分析") || text.contains("analyze") {
            CommandType::Analyze
        } else if text.contains("tag") || text.contains("标签") {
            CommandType::Tag
        } else if text.contains("查询") || text.contains("query") || text.contains("查看") {
            CommandType::Query
        } else {
            CommandType::Search // 默认为搜索
        }
    }

    fn extract_entities(&self, text: &str) -> HashMap<String, Entity> {
        let mut entities = HashMap::new();

        // 提取时间范围
        if text.contains("上周") || text.contains("上周") {
            entities.insert(
                "time_range".to_string(),
                Entity {
                    entity_type: EntityType::TimeRange,
                    value: "last_week".to_string(),
                },
            );
        } else if text.contains("昨天") || text.contains("昨日") {
            entities.insert(
                "time_range".to_string(),
                Entity {
                    entity_type: EntityType::TimeRange,
                    value: "yesterday".to_string(),
                },
            );
        }

        // 提取关键词
        if let Some(keyword) = self.extract_quoted_text(text) {
            entities.insert(
                "keyword".to_string(),
                Entity {
                    entity_type: EntityType::Keyword,
                    value: keyword,
                },
            );
        }

        // 提取模块名（中文名词短语 + 模块）
        for module_kw in &["模块", "组件", "功能", "服务"] {
            if let Some(pos) = text.find(module_kw) {
                let before = &text[..pos];
                if let Some(last_space) = before.rfind(|c: char| !c.is_alphanumeric() && c != '_') {
                    let module_name = before[last_space + 1..].trim().to_string();
                    if !module_name.is_empty() {
                        entities.insert(
                            "module".to_string(),
                            Entity {
                                entity_type: EntityType::ModuleName,
                                value: module_name,
                            },
                        );
                    }
                }
            }
        }

        // 提取环境
        if text.contains("测试环境") || text.contains("staging") || text.contains("test") {
            entities.insert(
                "environment".to_string(),
                Entity {
                    entity_type: EntityType::TagName,
                    value: "test".to_string(),
                },
            );
        }

        entities
    }

    fn extract_quoted_text(&self, text: &str) -> Option<String> {
        // 提取单引号内容
        if let Some(start) = text.find('\'') {
            if let Some(end) = text[start + 1..].find('\'') {
                return Some(text[start + 1..start + 1 + end].to_string());
            }
        }
        // 提取双引号内容
        if let Some(start) = text.find('"') {
            if let Some(end) = text[start + 1..].find('"') {
                return Some(text[start + 1..start + 1 + end].to_string());
            }
        }
        None
    }
}

impl Default for NaturalLanguageParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_recovery_command() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse("把上周agent实现的用户画像分析模块恢复回来");
        assert_eq!(result.command_type, CommandType::Recovery);
        assert!(result.entities.contains_key("time_range"));
    }

    #[test]
    fn test_parse_merge_command() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse("合并最近的PR");
        assert_eq!(result.command_type, CommandType::Merge);
    }

    #[test]
    fn test_parse_search_command() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse("搜索认证相关的提交");
        assert_eq!(result.command_type, CommandType::Search);
    }

    #[test]
    fn test_parse_deploy_command() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse("部署到测试环境");
        assert_eq!(result.command_type, CommandType::Deploy);
        assert!(result.entities.contains_key("environment"));
    }
}
