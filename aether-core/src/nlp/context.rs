use crate::nlp::parser::{CommandType, ParsedCommand};
use crate::utils::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 会话上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: Uuid,
    pub current_branch: String,
    pub current_commit: String,
    pub history: Vec<ConversationTurn>,
}

impl SessionContext {
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            current_branch: "main".to_string(),
            current_commit: String::new(),
            history: Vec::new(),
        }
    }

    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.current_branch = branch.into();
        self
    }
}

impl Default for SessionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 对话轮次
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub user_input: String,
    pub parsed_command: Option<ParsedCommand>,
    pub system_response: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 对话上下文管理器
pub struct ContextManager {
    sessions: std::sync::Mutex<HashMap<Uuid, SessionContext>>,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            sessions: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// 创建新会话
    pub fn create_session(&self) -> Uuid {
        let session_id = Uuid::new_v4();
        let context = SessionContext::new();
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(session_id, context);
        session_id
    }

    /// 获取会话
    pub fn get_session(&self, session_id: Uuid) -> Option<SessionContext> {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(&session_id).cloned()
    }

    /// 添加对话记录
    pub fn add_turn(
        &self,
        session_id: Uuid,
        user_input: &str,
        parsed: Option<&ParsedCommand>,
        response: &str,
    ) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(context) = sessions.get_mut(&session_id) {
            context.history.push(ConversationTurn {
                user_input: user_input.to_string(),
                parsed_command: parsed.cloned(),
                system_response: response.to_string(),
                timestamp: chrono::Utc::now(),
            });
        }
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let manager = ContextManager::new();
        let id = manager.create_session();
        assert!(manager.get_session(id).is_some());
    }

    #[test]
    fn test_add_turn() {
        let manager = ContextManager::new();
        let id = manager.create_session();
        manager.add_turn(id, "hello", None, "hi");

        let session = manager.get_session(id).unwrap();
        assert_eq!(session.history.len(), 1);
    }
}
