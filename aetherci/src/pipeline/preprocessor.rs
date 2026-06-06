//! 预处理器：解析 diff 文本，提取结构化的变更实体
//!
//! 支持 Python 和 TypeScript 的启发式 AST 解析。
//! 识别函数、类、接口、模块等级别的实体变更。

use crate::domain::{ChangedEntity, DiffStatistics, EntityOperation, IntentGroup};
use std::collections::HashMap;
use tracing::info;

/// 预处理器
pub struct Preprocessor {
    /// 支持的语言列表
    supported_languages: Vec<String>,
}

impl Preprocessor {
    pub fn new() -> Self {
        Self {
            supported_languages: vec![
                "python".to_string(),
                "typescript".to_string(),
                "javascript".to_string(),
                "tsx".to_string(),
                "jsx".to_string(),
            ],
        }
    }

    /// 解析 diff，返回 DiffStatistics
    pub fn analyze(&self, diff: &str, _repo_path: &str) -> DiffStatistics {
        let mut files_changed: u32 = 0;
        let mut additions: u32 = 0;
        let mut deletions: u32 = 0;
        let mut languages: Vec<String> = Vec::new();
        let mut entities: Vec<ChangedEntity> = Vec::new();
        let mut current_file: Option<String> = None;

        for line in diff.lines() {
            // 文件头检测
            if line.starts_with("--- ") || line.starts_with("+++ ") {
                if let Some(path) = line[4..].trim().strip_prefix("a/")
                    .or_else(|| line[4..].trim().strip_prefix("b/"))
                {
                    let path = path.to_string();
                    if path != "/dev/null" && !current_file.as_ref().map_or(false, |c| c == &path) {
                        if current_file.is_some() {
                            files_changed += 1;
                        }
                        current_file = Some(path.clone());
                        // 识别语言
                        if let Some(lang) = Self::detect_language(&path) {
                            if !languages.contains(&lang) {
                                languages.push(lang.clone());
                            }
                        }
                    }
                }
            }

            // 统计新增/删除
            if line.starts_with('+') && !line.starts_with("+++ ") {
                additions += 1;
                // 尝试检测新增实体
                if let Some(file) = &current_file {
                    if let Some(entity) = Self::detect_entity(line, file, EntityOperation::Added) {
                        entities.push(entity);
                    }
                }
            } else if line.starts_with('-') && !line.starts_with("--- ") {
                deletions += 1;
                if let Some(file) = &current_file {
                    if let Some(entity) = Self::detect_entity(line, file, EntityOperation::Deleted) {
                        entities.push(entity);
                    }
                }
            }

            // 检测修改的实体（非 +/- 行，但包含定义语句）
            if !line.starts_with('+') && !line.starts_with('-') && !line.starts_with('@') {
                if let Some(file) = &current_file {
                    if let Some(entity) = Self::detect_entity(line, file, EntityOperation::Modified) {
                        entities.push(entity);
                    }
                }
            }
        }

        // 最后的文件计数
        if current_file.is_some() {
            files_changed += 1;
        }

        // 去重实体
        let entities = Self::deduplicate_entities(entities);

        // 生成意图分组
        let intent_groups = self.group_by_intent(&entities, &languages);

        info!(
            "预处理器完成: {} 文件, +{} -{} 行, {} 实体, {} 分组",
            files_changed,
            additions,
            deletions,
            entities.len(),
            intent_groups.len()
        );

        DiffStatistics {
            files_changed,
            additions,
            deletions,
            languages,
            entities,
            intent_groups,
        }
    }

    /// 检测文件语言
    fn detect_language(file_path: &str) -> Option<String> {
        let path_lower = file_path.to_lowercase();
        if path_lower.ends_with(".py") {
            Some("python".to_string())
        } else if path_lower.ends_with(".ts") {
            Some("typescript".to_string())
        } else if path_lower.ends_with(".tsx") {
            Some("typescript".to_string())
        } else if path_lower.ends_with(".js") {
            Some("javascript".to_string())
        } else if path_lower.ends_with(".jsx") {
            Some("javascript".to_string())
        } else if path_lower.ends_with(".rs") {
            Some("rust".to_string())
        } else if path_lower.ends_with(".go") {
            Some("go".to_string())
        } else if path_lower.ends_with(".java") {
            Some("java".to_string())
        } else if path_lower.ends_with(".json") || path_lower.ends_with(".toml")
            || path_lower.ends_with(".yaml") || path_lower.ends_with(".yml")
        {
            Some("config".to_string())
        } else {
            None
        }
    }

    /// 检测代码行中的实体定义
    fn detect_entity(line: &str, file_path: &str, operation: EntityOperation) -> Option<ChangedEntity> {
        let trimmed = line.trim();
        // 跳过非代码行和纯符号行
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
            || trimmed.starts_with("import")
            || trimmed.starts_with("from ")
            || trimmed.starts_with("use ")
            || trimmed == "{" || trimmed == "}" || trimmed == "(" || trimmed == ")"
        {
            return None;
        }

        let lang = Self::detect_language(file_path).unwrap_or_default();

        // Python 实体检测
        if lang == "python" {
            if trimmed.starts_with("class ") {
                let name = Self::extract_name_after(trimmed, "class ");
                if let Some(name) = name {
                    return Some(ChangedEntity {
                        name,
                        entity_type: "class".to_string(),
                        file_path: file_path.to_string(),
                        operation: operation.clone(),
                        previous_name: None,
                    });
                }
            }
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let name = if trimmed.starts_with("async def ") {
                    Self::extract_name_after(trimmed, "async def ")
                } else {
                    Self::extract_name_after(trimmed, "def ")
                };
                if let Some(name) = name {
                    return Some(ChangedEntity {
                        name,
                        entity_type: "function".to_string(),
                        file_path: file_path.to_string(),
                        operation: operation.clone(),
                        previous_name: None,
                    });
                }
            }
        }

        // TypeScript/JavaScript 实体检测
        if lang == "typescript" || lang == "javascript" {
            if trimmed.contains("class ") && (trimmed.starts_with("class ") || trimmed.starts_with("export class ")) {
                let prefix = if trimmed.starts_with("export class ") { "export class " } else { "class " };
                let name = Self::extract_name_after(trimmed, prefix);
                if let Some(name) = name {
                    return Some(ChangedEntity {
                        name,
                        entity_type: "class".to_string(),
                        file_path: file_path.to_string(),
                        operation: operation.clone(),
                        previous_name: None,
                    });
                }
            }

            if (trimmed.starts_with("function ") || trimmed.starts_with("export function ")
                || trimmed.starts_with("async function "))
                && trimmed.contains('(') {
                let prefix = if trimmed.starts_with("export async function ") {
                    "export async function "
                } else if trimmed.starts_with("export function ") {
                    "export function "
                } else if trimmed.starts_with("async function ") {
                    "async function "
                } else {
                    "function "
                };
                let name = Self::extract_name_after(trimmed, prefix);
                if let Some(name) = name {
                    return Some(ChangedEntity {
                        name,
                        entity_type: "function".to_string(),
                        file_path: file_path.to_string(),
                        operation: operation.clone(),
                        previous_name: None,
                    });
                }
            }

            // 接口检测
            if trimmed.starts_with("interface ") || trimmed.starts_with("export interface ") {
                let prefix = if trimmed.starts_with("export interface ") {
                    "export interface "
                } else {
                    "interface "
                };
                let name = Self::extract_name_after(trimmed, prefix);
                if let Some(name) = name {
                    return Some(ChangedEntity {
                        name,
                        entity_type: "interface".to_string(),
                        file_path: file_path.to_string(),
                        operation: operation.clone(),
                        previous_name: None,
                    });
                }
            }

            // 类型定义检测
            if trimmed.starts_with("type ") && trimmed.contains('=') {
                let name = Self::extract_name_after(trimmed, "type ");
                if let Some(name) = name {
                    return Some(ChangedEntity {
                        name,
                        entity_type: "type".to_string(),
                        file_path: file_path.to_string(),
                        operation: operation.clone(),
                        previous_name: None,
                    });
                }
            }

            // const/let/var 导出检测（可能的模块级变量）
            if (trimmed.starts_with("export const ") || trimmed.starts_with("export let "))
                && trimmed.contains('=') {
                let prefix = if trimmed.starts_with("export const ") {
                    "export const "
                } else {
                    "export let "
                };
                let name = Self::extract_name_after(trimmed, prefix);
                if let Some(name) = name {
                    return Some(ChangedEntity {
                        name,
                        entity_type: "variable".to_string(),
                        file_path: file_path.to_string(),
                        operation: operation.clone(),
                        previous_name: None,
                    });
                }
            }

            // 箭头函数常量检测
            if (trimmed.starts_with("const ") || trimmed.starts_with("export const "))
                && (trimmed.contains("=>") || trimmed.contains(": (") || trimmed.contains(": (")) {
                let prefix = if trimmed.starts_with("export const ") {
                    "export const "
                } else {
                    "const "
                };
                let name = Self::extract_name_after(trimmed, prefix);
                if let Some(name) = name {
                    return Some(ChangedEntity {
                        name,
                        entity_type: "function".to_string(),
                        file_path: file_path.to_string(),
                        operation: operation.clone(),
                        previous_name: None,
                    });
                }
            }
        }

        None
    }

    /// 从前缀后提取标识符名称
    fn extract_name_after<'a>(text: &'a str, prefix: &str) -> Option<String> {
        let after = text.strip_prefix(prefix)?;
        let name = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect::<String>();
        if name.is_empty() { None } else { Some(name) }
    }

    /// 去重实体（按 name + file_path + entity_type 去重）
    fn deduplicate_entities(entities: Vec<ChangedEntity>) -> Vec<ChangedEntity> {
        let mut seen: HashMap<String, ChangedEntity> = HashMap::new();
        for entity in entities {
            let key = format!("{}:{}:{}", entity.file_path, entity.entity_type, entity.name);
            // 优先保留修改/删除，而非新增
            match entity.operation {
                EntityOperation::Modified | EntityOperation::Deleted
                | EntityOperation::Renamed | EntityOperation::Moved => {
                    seen.insert(key, entity);
                }
                EntityOperation::Added => {
                    seen.entry(key).or_insert(entity);
                }
            }
        }
        seen.into_values().collect()
    }

    /// 按意图分组实体
    fn group_by_intent(
        &self,
        entities: &[ChangedEntity],
        _languages: &[String],
    ) -> Vec<IntentGroup> {
        // 按文件分组，同一文件的变更通常属于同一意图
        let mut file_groups: HashMap<String, Vec<&ChangedEntity>> = HashMap::new();
        for entity in entities {
            file_groups
                .entry(entity.file_path.clone())
                .or_default()
                .push(entity);
        }

        file_groups
            .into_iter()
            .enumerate()
            .map(|(i, (file, ents))| IntentGroup {
                label: if ents.is_empty() {
                    format!("变更组_{}", i + 1)
                } else {
                    let types: Vec<&str> = ents.iter().map(|e| e.entity_type.as_str()).collect();
                    let unique_types: Vec<&str> = {
                        let mut t = types.clone();
                        t.sort();
                        t.dedup();
                        t
                    };
                    format!("{}_{}", unique_types.join("/"), file.rsplit('/').next().unwrap_or(&file))
                },
                files: vec![file],
                entities: ents.iter().map(|e| e.name.clone()).collect(),
            })
            .collect()
    }
}

impl Default for Preprocessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(Preprocessor::detect_language("src/main.py"), Some("python".to_string()));
        assert_eq!(Preprocessor::detect_language("src/app.ts"), Some("typescript".to_string()));
        assert_eq!(Preprocessor::detect_language("src/util.js"), Some("javascript".to_string()));
        assert_eq!(Preprocessor::detect_language("Cargo.toml"), Some("config".to_string()));
    }

    #[test]
    fn test_detect_python_entity() {
        let entity = Preprocessor::detect_entity(
            "def hello_world():",
            "src/main.py",
            EntityOperation::Added,
        );
        assert!(entity.is_some());
        assert_eq!(entity.unwrap().name, "hello_world");
    }

    #[test]
    fn test_detect_ts_entity() {
        let entity = Preprocessor::detect_entity(
            "export class MyService {",
            "src/service.ts",
            EntityOperation::Added,
        );
        assert!(entity.is_some());
        assert_eq!(entity.unwrap().name, "MyService");
    }

    #[test]
    fn test_extract_name_after() {
        assert_eq!(
            Preprocessor::extract_name_after("def hello_world(", "def "),
            Some("hello_world".to_string())
        );
        assert_eq!(
            Preprocessor::extract_name_after("  function fooBar<T>(", "function "),
            Some("fooBar".to_string())
        );
    }

    #[test]
    fn test_analyze_basic_diff() {
        let preprocessor = Preprocessor::new();
        let diff = r#"--- a/src/main.py
+++ b/src/main.py
@@ -1,3 +1,5 @@
 def old_func():
     pass
+def new_func():
+    return True
-class OldClass:
-    pass
"#;
        let stats = preprocessor.analyze(diff, ".");
        assert!(stats.files_changed >= 1);
        assert!(stats.additions > 0);
        assert!(stats.languages.contains(&"python".to_string()));
    }
}
