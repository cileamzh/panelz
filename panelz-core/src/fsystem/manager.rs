use crate::fsystem::ProvideFs;
use std::sync::Arc;

pub struct FsManager {
    pub provider: Arc<dyn ProvideFs>,
}

impl FsManager {
    /// 初始化时注入具体的 Provider 实现
    pub fn new(provider: Arc<dyn ProvideFs>) -> Self {
        Self { provider }
    }
}
