use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum UserRole {
    Admin, // 超级管理员：可管理挂载点和用户
    User,  // 普通用户：仅能访问分配给自己的虚拟路径
    Guest, // 访客：通常仅限只读
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub username: String,
    pub password_hash: String, // 实际存储应为哈希值
    pub role: UserRole,
}

pub struct UserManager {
    // 底层存储：如 SqliteUserProvider
    pub provider: Arc<dyn ProvideUser>,
}

impl UserManager {
    pub fn new(provider: Arc<dyn ProvideUser>) -> Self {
        Self { provider }
    }
}

#[async_trait]
pub trait ProvideUser: Send + Sync {
    // --- 基础管理 ---
    async fn get_user(&self, username: &str) -> Option<User>;
    async fn save_user(&self, user: &User) -> std::io::Result<()>;
    async fn update_user(&self, user: &User) -> std::io::Result<()>;
    async fn list_all_users(&self) -> Vec<User>;
    async fn delete_user(&self, username: &str) -> std::io::Result<()>;
    // --- 核心验证逻辑 ---
    /// 1. 验证用户身份 (用于登录)
    /// 返回验证成功的 User 对象，如果密码错误或用户不存在则返回 None
    async fn verify_user(&self, username: &str, password_plain: &str) -> Option<User>;
    /// 2. 检查用户是否有权访问某个虚拟路径
    /// virt_path: 请求访问的路径, need_write: 是否需要写入权限
    // async fn check_permission(&self, username: &str, virt_path: &str, need_write: bool) -> bool;
    // --- 会话管理 ---
    async fn set_user_session(&self, name: &str, password: &str) -> Option<String>;
    async fn get_user_from_sessions(&self, key: &str) -> Option<User>;
}
