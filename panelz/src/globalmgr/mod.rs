use std::{sync::Arc, time::Duration};

use panelz_core::{filesys::FsManager, infosys::SysManager, usersys::UserManager};

use crate::{
    cfgtable::ConfigKey,
    providers::{fspvdr::LocalFsProvider, syspvdr::LocalSysProvider, usrpvdr::JsonUserProvider},
};
pub struct GlobalManager {
    // 核心系统数据管理器
    pub sys: Arc<SysManager>,
    // 用户管理
    pub usr: Arc<UserManager>,
    // 文件管理
    pub fsm: Arc<FsManager>,
}

impl GlobalManager {
    pub async fn local() -> Self {
        // 加载 sysinfo provider
        let syspvdr = Arc::new(LocalSysProvider::new("").await);
        let sys = Arc::new(SysManager::new(syspvdr));
        // 加载 userprovider
        let usrpvdr = Arc::new(JsonUserProvider::new(ConfigKey::UserSystem.path()).await);
        let usr = Arc::new(UserManager::new(usrpvdr));
        // 加载 fs provider
        let fspvdr = Arc::new(LocalFsProvider::new(ConfigKey::FileSystem.path()).await);
        let fsm = Arc::new(FsManager::new(fspvdr));

        let sys_clone = sys.clone();
        // 启动后台刷新任务
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                sys_clone.provider.refresh_quick().await;
                interval.tick().await;
            }
        });
        let sys_clone = sys.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                sys_clone.provider.refresh_all().await;
                interval.tick().await;
            }
        });

        Self { sys, usr, fsm }
    }
}
