use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CpuInfo {
    pub usage: f32,
    pub brand: String,
    pub frequency: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub free: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskInfo {
    pub name: String,         // 磁盘名称 (如 /dev/sda)
    pub kind: String,         // 磁盘类型 (如 SSD, HDD)
    pub file_system: String,  // 文件系统 (如 ext4, xfs, ntfs)
    pub mount_point: String,  // 挂载点 (如 /)
    pub total_space: u64,     // 总容量 (Bytes)
    pub available_space: u64, // 可用容量 (Bytes)
    pub is_removable: bool,   // 是否为可移动设备
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory: u64,         // 字节 (Bytes)
    pub virtual_memory: u64, // 虚拟内存 (Bytes)
    pub parent_pid: Option<u32>,
    pub status: String, // 如 "Running", "Sleeping", "Idle"
    pub exe_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GpuInfo {
    pub name: String,           // 如 "NVIDIA GeForce RTX 4090"
    pub brand: String,          // NVIDIA, AMD, Intel
    pub usage: f32,             // 核心利用率 (百分比)
    pub memory_total: u64,      // 总显存 (Bytes)
    pub memory_used: u64,       // 已用显存 (Bytes)
    pub temperature: i32,       // 温度 (摄氏度)
    pub power_usage: f32,       // 实时功耗 (Watts)
    pub driver_version: String, // 驱动版本
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkInfo {
    pub name: String,
    pub received: u64,    // 累计接收字节
    pub transmitted: u64, // 累计发送字节
    pub packets_received: u64,
    pub packets_transmitted: u64,
    pub mac_address: String,
}

pub struct SysManager {
    pub provider: Arc<dyn ProvideSystem>,
}

impl SysManager {
    /// 注入具体的 Provider 实现
    pub fn new(provider: Arc<dyn ProvideSystem>) -> Self {
        Self { provider }
    }
}

#[async_trait]
pub trait ProvideSystem: Send + Sync {
    // --- 核心调度 ---

    /// 全量刷新：一次性刷新 CPU, 内存, 磁盘, 网络, GPU, 进程 (适合低频全量更新)
    async fn refresh_all(&self);

    /// 快速刷新：仅刷新 CPU, 内存, 进程使用率 (适合高频实时数据)
    async fn refresh_quick(&self);

    // --- 基础信息 (CPU & Memory) ---
    async fn get_cpu_info(&self) -> Vec<CpuInfo>;
    async fn get_memory_info(&self) -> MemoryInfo;

    /// 获取系统基础摘要（包含运行时间、内核版本等）
    async fn get_host_info(&self) -> serde_json::Value;

    // --- 磁盘信息 ---
    async fn get_disks(&self) -> Vec<DiskInfo>;

    // --- 进程管理 ---
    async fn get_processes(&self) -> Vec<ProcessInfo>;
    async fn get_process_by_pid(&self, pid: u32) -> Option<ProcessInfo>;
    async fn kill_process(&self, pid: u32) -> std::io::Result<()>;

    // --- GPU 信息 ---
    async fn get_gpu_info(&self) -> Vec<GpuInfo>;
    async fn has_gpu(&self) -> bool;

    // --- 网络信息 ---
    async fn get_network_info(&self) -> Vec<NetworkInfo>;
}
