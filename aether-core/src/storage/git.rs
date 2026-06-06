use crate::domain::commit::{Commit, CommitId, FileChange, ChangeType};
use crate::domain::semantic::CurrentState;
use crate::utils::Result;
use std::path::{Path, PathBuf};

/// Git 仓库操作抽象
#[async_trait::async_trait]
pub trait GitOperations: Send + Sync {
    /// 获取所有 commit
    async fn list_commits(&self) -> Result<Vec<Commit>>;

    /// 获取指定 commit
    async fn get_commit(&self, hash: &str) -> Result<Option<Commit>>;

    /// 获取 commit 的 diff
    async fn get_commit_diff(&self, hash: &str) -> Result<String>;

    /// 获取两个 commit 之间的 diff
    async fn get_diff_between(&self, from_ref: &str, to_ref: &str) -> Result<String>;

    /// 获取指定范围内的 commits
    async fn get_commits_in_range(&self, from_ref: &str, to_ref: &str) -> Result<Vec<Commit>>;

    /// 获取当前状态
    async fn current_state(&self) -> Result<CurrentState>;

    /// 仓库路径
    fn repo_path(&self) -> &Path;
}

/// 基于 git2 的 Git 仓库实现
pub struct GitRepository {
    path: PathBuf,
}

impl GitRepository {
    /// 打开指定路径的 Git 仓库
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        // 验证仓库存在
        let _repo = git2::Repository::open(&path).map_err(|e| {
            crate::utils::AetherError::Git(format!("Failed to open repository: {}", e))
        })?;

        Ok(Self { path })
    }

    /// 创建新的 GitRepository（不验证路径）
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn open_repo(&self) -> Result<git2::Repository> {
        git2::Repository::open(&self.path).map_err(|e| {
            crate::utils::AetherError::Git(format!("Failed to open repository: {}", e))
        })
    }
}

#[async_trait::async_trait]
impl GitOperations for GitRepository {
    async fn list_commits(&self) -> Result<Vec<Commit>> {
        let repo = self.open_repo()?;
        let mut revwalk = repo.revwalk().map_err(|e| {
            crate::utils::AetherError::Git(format!("Failed to create revwalk: {}", e))
        })?;

        revwalk.push_head().map_err(|e| {
            crate::utils::AetherError::Git(format!("Failed to push head: {}", e))
        })?;

        let mut commits = Vec::new();
        for oid_result in revwalk {
            let oid = oid_result.map_err(|e| {
                crate::utils::AetherError::Git(format!("Revwalk error: {}", e))
            })?;

            let git_commit = repo.find_commit(oid).map_err(|e| {
                crate::utils::AetherError::Git(format!("Failed to find commit: {}", e))
            })?;

            let author = git_commit.author();
            let commit = Commit::new(
                oid.to_string(),
                git_commit.message().unwrap_or("").to_string(),
                crate::domain::commit::Author::new(
                    author.name().unwrap_or("unknown"),
                    author.email().unwrap_or("unknown"),
                ),
                chrono::DateTime::from_timestamp(git_commit.time().seconds(), 0)
                    .unwrap_or_default(),
                git_commit.parent_ids().map(|id| id.to_string()).collect(),
            );

            commits.push(commit);
        }

        Ok(commits)
    }

    async fn get_commit(&self, hash: &str) -> Result<Option<Commit>> {
        let repo = self.open_repo()?;
        let oid = git2::Oid::from_str(hash).map_err(|e| {
            crate::utils::AetherError::Git(format!("Invalid hash: {}", e))
        })?;

        let commit_result = repo.find_commit(oid);
        match commit_result {
            Ok(git_commit) => {
                let author = git_commit.author();
                let commit = Commit::new(
                    oid.to_string(),
                    git_commit.message().unwrap_or("").to_string(),
                    crate::domain::commit::Author::new(
                        author.name().unwrap_or("unknown"),
                        author.email().unwrap_or("unknown"),
                    ),
                    chrono::DateTime::from_timestamp(git_commit.time().seconds(), 0)
                        .unwrap_or_default(),
                    git_commit.parent_ids().map(|id| id.to_string()).collect(),
                );
                Ok(Some(commit))
            }
            Err(_) => Ok(None),
        }
    }

    async fn get_commit_diff(&self, hash: &str) -> Result<String> {
        let repo = self.open_repo()?;
        let oid = git2::Oid::from_str(hash).map_err(|e| {
            crate::utils::AetherError::Git(format!("Invalid hash: {}", e))
        })?;

        let commit = repo.find_commit(oid).map_err(|e| {
            crate::utils::AetherError::Git(format!("Failed to find commit: {}", e))
        })?;

        let tree = commit.tree().map_err(|e| {
            crate::utils::AetherError::Git(format!("Failed to get tree: {}", e))
        })?;

        let parent_tree = if commit.parent_count() > 0 {
            Some(
                commit
                    .parent(0)
                    .map_err(|e| {
                        crate::utils::AetherError::Git(format!("Failed to get parent: {}", e))
                    })?
                    .tree()
                    .map_err(|e| {
                        crate::utils::AetherError::Git(format!("Failed to get parent tree: {}", e))
                    })?,
            )
        } else {
            None
        };

        let mut diff_opts = git2::DiffOptions::new();
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))
            .map_err(|e| {
                crate::utils::AetherError::Git(format!("Failed to diff: {}", e))
            })?;

        let mut diff_text = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let content = String::from_utf8_lossy(line.content());
            diff_text.push_str(&content);
            true
        })
        .map_err(|e| {
            crate::utils::AetherError::Git(format!("Failed to print diff: {}", e))
        })?;

        Ok(diff_text)
    }

    async fn get_diff_between(&self, from_ref: &str, to_ref: &str) -> Result<String> {
        let repo = self.open_repo()?;

        let from_oid = repo
            .revparse_single(from_ref)
            .map_err(|e| crate::utils::AetherError::Git(format!("Invalid from ref '{}': {}", from_ref, e)))?
            .id();
        let from_commit = repo.find_commit(from_oid)
            .map_err(|e| crate::utils::AetherError::Git(format!("Commit not found: {}", e)))?;
        let from_tree = from_commit.tree()
            .map_err(|e| crate::utils::AetherError::Git(format!("Tree error: {}", e)))?;

        let to_oid = repo
            .revparse_single(to_ref)
            .map_err(|e| crate::utils::AetherError::Git(format!("Invalid to ref '{}': {}", to_ref, e)))?
            .id();
        let to_commit = repo.find_commit(to_oid)
            .map_err(|e| crate::utils::AetherError::Git(format!("Commit not found: {}", e)))?;
        let to_tree = to_commit.tree()
            .map_err(|e| crate::utils::AetherError::Git(format!("Tree error: {}", e)))?;

        let mut diff_opts = git2::DiffOptions::new();
        let diff = repo
            .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut diff_opts))
            .map_err(|e| crate::utils::AetherError::Git(format!("Diff error: {}", e)))?;

        let mut diff_text = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let content = String::from_utf8_lossy(line.content());
            diff_text.push_str(&content);
            true
        })
        .map_err(|e| crate::utils::AetherError::Git(format!("Print diff error: {}", e)))?;

        Ok(diff_text)
    }

    async fn get_commits_in_range(&self, from_ref: &str, to_ref: &str) -> Result<Vec<Commit>> {
        let repo = self.open_repo()?;

        let range_spec = format!("{}..{}", from_ref, to_ref);
        let mut revwalk = repo.revwalk()
            .map_err(|e| crate::utils::AetherError::Git(format!("Revwalk error: {}", e)))?;
        revwalk.push_range(&range_spec)
            .map_err(|e| crate::utils::AetherError::Git(format!("Range error: {}", e)))?;

        let mut commits = Vec::new();
        for oid_result in revwalk {
            let oid = oid_result.map_err(|e| {
                crate::utils::AetherError::Git(format!("Revwalk iter error: {}", e))
            })?;
            let git_commit = repo.find_commit(oid).map_err(|e| {
                crate::utils::AetherError::Git(format!("Find commit error: {}", e))
            })?;
            let author = git_commit.author();
            let commit = Commit::new(
                oid.to_string(),
                git_commit.message().unwrap_or("").to_string(),
                crate::domain::commit::Author::new(
                    author.name().unwrap_or("unknown"),
                    author.email().unwrap_or("unknown"),
                ),
                chrono::DateTime::from_timestamp(git_commit.time().seconds(), 0)
                    .unwrap_or_default(),
                git_commit.parent_ids().map(|id| id.to_string()).collect(),
            );
            commits.push(commit);
        }

        Ok(commits)
    }

    async fn current_state(&self) -> Result<CurrentState> {
        let repo = self.open_repo()?;
        let head = repo.head().map_err(|e| {
            crate::utils::AetherError::Git(format!("Failed to get HEAD: {}", e))
        })?;

        let branch = head.shorthand().unwrap_or("HEAD").to_string();
        let commit = head.peel_to_commit().map_err(|e| {
            crate::utils::AetherError::Git(format!("Failed to get commit: {}", e))
        })?;

        Ok(CurrentState::new(branch, commit.id().to_string()))
    }

    fn repo_path(&self) -> &Path {
        &self.path
    }
}

// 单元测试
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::process::Command;

    fn init_test_repo(path: &Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    fn make_commit(path: &Path, message: &str) {
        let file = path.join("test.txt");
        std::fs::write(&file, message).unwrap();

        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(path)
            .output()
            .unwrap();
    }

    #[test]
    fn test_open_valid_repo() {
        let dir = TempDir::new().unwrap();
        init_test_repo(dir.path());

        let repo = GitRepository::open(dir.path()).unwrap();
        assert!(repo.repo_path() == dir.path());
    }

    #[test]
    fn test_open_invalid_path() {
        let result = GitRepository::open("/nonexistent/path");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_commits() {
        let dir = TempDir::new().unwrap();
        init_test_repo(dir.path());
        make_commit(dir.path(), "first commit");
        make_commit(dir.path(), "second commit");

        let repo = GitRepository::open(dir.path()).unwrap();
        let commits = repo.list_commits().await.unwrap();

        assert_eq!(commits.len(), 2);
        assert!(commits[0].message.contains("second"));
    }

    #[tokio::test]
    async fn test_get_commit_diff() {
        let dir = TempDir::new().unwrap();
        init_test_repo(dir.path());
        make_commit(dir.path(), "initial");

        let repo = GitRepository::open(dir.path()).unwrap();
        let commits = repo.list_commits().await.unwrap();
        let diff = repo.get_commit_diff(&commits[0].id.0).await.unwrap();

        assert!(!diff.is_empty());
    }

    #[tokio::test]
    async fn test_current_state() {
        let dir = TempDir::new().unwrap();
        init_test_repo(dir.path());
        make_commit(dir.path(), "test");

        let repo = GitRepository::open(dir.path()).unwrap();
        let state = repo.current_state().await.unwrap();

        assert!(state.current_branch.contains("master") || state.current_branch.contains("main"));
    }
}
