use crate::usersys::*;
use std::sync::Arc;

pub struct UserManager {
    // 底层存储：如 SqliteUserProvider
    pub provider: Arc<dyn ProvideUser>,
}

impl UserManager {
    pub fn new(provider: Arc<dyn ProvideUser>) -> Self {
        Self { provider }
    }
}
