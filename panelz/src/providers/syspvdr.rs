use async_trait::async_trait;
use panelz_core::sysinfo::*;
use std::sync::Arc;
use sysinfo::{Disks, Networks, Pid, System};
use tokio::sync::RwLock;

pub struct LocalSysProvider {
    sys: Arc<RwLock<System>>,
    dks: Arc<RwLock<Disks>>,
    ntw: Arc<RwLock<Networks>>,
}

impl LocalSysProvider {
    pub async fn new(_cfg: &str) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        Self {
            sys: Arc::new(RwLock::new(sys)),
            dks: Arc::new(RwLock::new(Disks::new_with_refreshed_list())),
            ntw: Arc::new(RwLock::new(Networks::new_with_refreshed_list())),
        }
    }
}

#[async_trait]
impl ProvideSystem for LocalSysProvider {
    // --- 核心调度 ---

    /// 快速刷新：仅刷新 CPU, 内存, 进程使用率
    async fn refresh_quick(&self) {
        let mut sys = self.sys.write().await;
        sys.refresh_cpu_frequency();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        // refresh_processes 默认只刷新已存在进程的 cpu/mem，不扫描新进程，性能较好
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let mut ntw = self.ntw.write().await;
        ntw.refresh(true);
    }

    /// 全量刷新：磁盘、网络、系统全状态
    async fn refresh_all(&self) {
        {
            let mut sys = self.sys.write().await;
            sys.refresh_all();
        }
        {
            let mut dks = self.dks.write().await;

            dks.refresh(true);
        }
        {
            let mut ntw = self.ntw.write().await;
            ntw.refresh(true);
        }
    }

    // --- 基础信息 ---

    async fn get_cpu_info(&self) -> Vec<CpuInfo> {
        let sys = self.sys.read().await;
        sys.cpus()
            .iter()
            .map(|cpu| CpuInfo {
                usage: cpu.cpu_usage(),
                brand: cpu.brand().to_string(),
                frequency: cpu.frequency(),
            })
            .collect()
    }

    async fn get_memory_info(&self) -> MemoryInfo {
        let sys = self.sys.read().await;
        MemoryInfo {
            total: sys.total_memory(),
            used: sys.used_memory(),
            free: sys.free_memory(),
        }
    }

    async fn get_host_info(&self) -> serde_json::Value {
        serde_json::json!({
            "os_name": System::name(),
            "kernel_version": System::kernel_version(),
            "os_version": System::os_version(),
            "host_name": System::host_name(),
            "uptime": System::uptime(),
            "cpu_arch": System::cpu_arch(),
        })
    }

    // --- 磁盘信息 ---

    async fn get_disks(&self) -> Vec<DiskInfo> {
        let dks = self.dks.read().await;
        dks.iter()
            .map(|d| DiskInfo {
                name: d.name().to_string_lossy().into_owned(),
                kind: format!("{:?}", d.kind()),
                file_system: d.file_system().to_string_lossy().to_string(),
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                total_space: d.total_space(),
                available_space: d.available_space(),
                is_removable: d.is_removable(),
            })
            .collect()
    }

    // --- 进程管理 ---

    async fn get_processes(&self) -> Vec<ProcessInfo> {
        let sys = self.sys.read().await;
        sys.processes()
            .iter()
            .map(|(pid, p)| ProcessInfo {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().to_string(),
                cpu_usage: p.cpu_usage(),
                memory: p.memory(),
                virtual_memory: p.virtual_memory(),
                parent_pid: p.parent().map(|id| id.as_u32()),
                status: format!("{:?}", p.status()),
                exe_path: p
                    .exe()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_default(),
            })
            .collect()
    }

    async fn get_process_by_pid(&self, pid: u32) -> Option<ProcessInfo> {
        let sys = self.sys.read().await;
        sys.process(Pid::from_u32(pid)).map(|p| ProcessInfo {
            pid,
            name: p.name().to_string_lossy().to_string(),
            cpu_usage: p.cpu_usage(),
            memory: p.memory(),
            virtual_memory: p.virtual_memory(),
            parent_pid: p.parent().map(|id| id.as_u32()),
            status: format!("{:?}", p.status()),
            exe_path: p
                .exe()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
        })
    }

    async fn kill_process(&self, pid: u32) -> std::io::Result<()> {
        let sys = self.sys.read().await;
        if let Some(p) = sys.process(Pid::from_u32(pid)) {
            if p.kill() {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Kill failed",
                ))
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Process not found",
            ))
        }
    }

    // --- GPU 信息 (sysinfo 不直接支持，预留接口) ---

    async fn get_gpu_info(&self) -> Vec<GpuInfo> {
        // 如果需要真实数据，建议引入 nvml-wrapper (Nvidia)
        // 或者解析 /sys/class/drm/renderD128/device/gpu_busy_percent (Linux)
        vec![]
    }

    async fn has_gpu(&self) -> bool {
        false
    }

    // --- 网络信息 ---

    async fn get_network_info(&self) -> Vec<NetworkInfo> {
        let ntw = self.ntw.read().await;
        ntw.iter()
            .map(|(name, data)| NetworkInfo {
                name: name.to_string(),
                received: data.received(),
                transmitted: data.transmitted(),
                packets_received: data.packets_received(),
                packets_transmitted: data.packets_transmitted(),
                mac_address: data.mac_address().to_string(),
            })
            .collect()
    }
}
