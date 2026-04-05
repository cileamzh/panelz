use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod manager;
pub use manager::UserManager;

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
    // pub allowed_mounts: Vec<String>, // 该用户有权访问的虚拟路由列表 (如 ["/Photos", "/Public"])
}

#[async_trait]
pub trait ProvideUser: Send + Sync {
    async fn get_user(&self, username: &str) -> Option<User>;
    async fn save_user(&self, user: &User) -> std::io::Result<()>;
    async fn update_user(&self, user: &User) -> std::io::Result<()>; // 新增：修改功能
    async fn list_all_users(&self) -> Vec<User>;
    async fn delete_user(&self, username: &str) -> std::io::Result<()>;
    async fn set_user_session(&self, name: &str, password: &str) -> Option<String>;
    async fn get_user_from_sessions(&self, key: &str) -> Option<User>;
}
