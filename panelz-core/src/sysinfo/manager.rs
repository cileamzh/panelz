use crate::sysinfo::ProvideSystem;
use std::sync::Arc;

pub struct SysManager {
    pub provider: Arc<dyn ProvideSystem>,
}

impl SysManager {
    /// 注入具体的 Provider 实现
    pub fn new(provider: Arc<dyn ProvideSystem>) -> Self {
        Self { provider }
    }
}
