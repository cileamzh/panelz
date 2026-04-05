use async_trait::async_trait;
use panelz_core::usersys::{ProvideUser, User};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use uuid::Uuid; // 需要在 Cargo.toml 添加 uuid = { version = "1", features = ["v4"] }

pub struct JsonUserProvider {
    file_path: PathBuf,
    /// 磁盘用户数据的内存缓存
    cache: Arc<RwLock<Vec<User>>>,
    /// 在线 Session: Token -> User
    sessions: Arc<RwLock<HashMap<String, User>>>,
}

impl JsonUserProvider {
    pub async fn new(path: &str) -> Self {
        let file_path = PathBuf::from(path);
        let mut users = Vec::new();

        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path).await {
                users = serde_json::from_str(&content).unwrap_or_else(|e| {
                    eprintln!("解析用户 JSON 失败: {}", e);
                    Vec::new()
                });
            }
        }

        println!("{:?}", users);

        Self {
            file_path,
            cache: Arc::new(RwLock::new(users)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 将内存缓存同步到磁盘
    async fn persist(&self) -> std::io::Result<()> {
        let users = self.cache.read().await;
        let content = serde_json::to_string_pretty(&*users)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(&self.file_path, content).await
    }
}

#[async_trait]
impl ProvideUser for JsonUserProvider {
    async fn get_user(&self, username: &str) -> Option<User> {
        let users = self.cache.read().await;
        users.iter().find(|u| u.username == username).cloned()
    }

    async fn save_user(&self, user: &User) -> std::io::Result<()> {
        {
            let mut users = self.cache.write().await;
            // 简单防止 ID 重复
            if users.iter().any(|u| u.username == user.username) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "User ID exists",
                ));
            }
            users.push(user.clone());
        }
        self.persist().await
    }

    async fn update_user(&self, user: &User) -> std::io::Result<()> {
        let found = {
            let mut users = self.cache.write().await;
            if let Some(pos) = users.iter().position(|u| u.username == user.username) {
                users[pos] = user.clone();
                true
            } else {
                false
            }
        };

        if found {
            self.persist().await
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "User not found",
            ))
        }
    }

    async fn list_all_users(&self) -> Vec<User> {
        self.cache.read().await.clone()
    }

    async fn delete_user(&self, username: &str) -> std::io::Result<()> {
        {
            let mut users = self.cache.write().await;
            users.retain(|u| u.username != username);
        }
        self.persist().await
    }

    /// 通过 Token 获取用户信息
    async fn get_user_from_sessions(&self, key: &str) -> Option<User> {
        let map = self.sessions.read().await;
        map.get(key).cloned()
    }

    /// 登录校验逻辑
    /// 成功则返回随机 Token，失败返回 None
    async fn set_user_session(&self, name: &str, password: &str) -> Option<String> {
        // 1. 查找用户
        let user = self.get_user(name).await?;

        // 2. 校验密码 (注意：实际生产应使用 bcrypt/argon2 校验 hash)
        if user.password_hash != password {
            return None;
        }

        // 3. 生成 Token
        let token = Uuid::new_v4().to_string();

        // 4. 存入 Session 映射
        let mut map = self.sessions.write().await;
        map.insert(token.clone(), user);

        Some(token)
    }
}
