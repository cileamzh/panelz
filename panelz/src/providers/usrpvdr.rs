use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
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
    sessions: Arc<RwLock<HashMap<String, SessionData>>>,
}

// 建议增加一个 Session 包装类来处理过期
#[derive(Clone)]
struct SessionData {
    pub user: User,
    pub expires_at: DateTime<Utc>,
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
    /// 查找用户
    async fn get_user(&self, username: &str) -> Option<User> {
        let users = self.cache.read().await;
        users.iter().find(|u| u.username == username).cloned()
    }

    /// 保存新用户
    async fn save_user(&self, user: &User) -> std::io::Result<()> {
        {
            let mut users = self.cache.write().await;
            if users.iter().any(|u| u.username == user.username) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!("User '{}' already exists", user.username),
                ));
            }
            // 可以在此处强制进行密码哈希处理，如果传入的是明文的话
            users.push(user.clone());
        }
        self.persist().await
    }

    /// 更新用户信息
    async fn update_user(&self, user: &User) -> std::io::Result<()> {
        let mut users = self.cache.write().await;
        if let Some(pos) = users.iter().position(|u| u.username == user.username) {
            users[pos] = user.clone();
            drop(users); // 提前释放锁再进行 IO
            self.persist().await
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "User not found",
            ))
        }
    }

    /// 删除用户并清理其相关的 Session
    async fn delete_user(&self, username: &str) -> std::io::Result<()> {
        {
            let mut users = self.cache.write().await;
            users.retain(|u| u.username != username);
        }
        // 清理 Session (可选，防止已删除的用户通过旧 Token 访问)
        {
            let mut sessions = self.sessions.write().await;
            sessions.retain(|_, data| data.user.username != username);
        }
        self.persist().await
    }

    /// 获取所有用户
    async fn list_all_users(&self) -> Vec<User> {
        self.cache.read().await.clone()
    }

    /// 验证 Session Token 并检查是否过期
    async fn get_user_from_sessions(&self, key: &str) -> Option<User> {
        let map = self.sessions.read().await;
        if let Some(session) = map.get(key) {
            if session.expires_at > Utc::now() {
                return Some(session.user.clone());
            }
        }
        None
    }

    /// 登录并生成 Token
    async fn set_user_session(&self, name: &str, password_plain: &str) -> Option<String> {
        // 1. 获取用户
        let user = self.get_user(name).await?;

        // 2. 校验密码 (推荐使用 bcrypt)
        // let valid = bcrypt::verify(password_plain, &user.password_hash).unwrap_or(false);
        // 此处暂存逻辑：如果是简单 demo，请至少确保 password_hash 是已经 hash 过的
        if user.password_hash != password_plain {
            return None;
        }

        // 3. 生成 Token
        let token = Uuid::new_v4().to_string();

        // 4. 存入带有过期时间的 Session (例如 24 小时有效)
        let mut map = self.sessions.write().await;
        map.insert(
            token.clone(),
            SessionData {
                user,
                expires_at: Utc::now() + Duration::hours(24),
            },
        );

        // 5. 定期清理过期 Session (可选逻辑：可以每 100 次登录清理一次)
        if map.len() > 1000 {
            map.retain(|_, data| data.expires_at > Utc::now());
        }

        Some(token)
    }

    async fn verify_user(&self, username: &str, password_plain: &str) -> Option<User> {
        todo!()
    }
}
