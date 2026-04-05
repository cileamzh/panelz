use std::sync::Arc;

use crate::globalmgr::GlobalManager;
use axum::{Json, extract::State};
use panelz_core::sysinfo::*;
// --- Handler 实现 ---
pub async fn get_cpu_handler(State(gm): State<Arc<GlobalManager>>) -> Json<Vec<CpuInfo>> {
    Json(gm.sys.provider.get_cpu_info().await)
}

pub async fn get_memory_handler(State(gm): State<Arc<GlobalManager>>) -> Json<MemoryInfo> {
    Json(gm.sys.provider.get_memory_info().await)
}

pub async fn get_processes_handler(State(gm): State<Arc<GlobalManager>>) -> Json<Vec<ProcessInfo>> {
    Json(gm.sys.provider.get_processes().await)
}

pub async fn get_base_info_handler(
    State(gm): State<Arc<GlobalManager>>,
) -> Json<serde_json::Value> {
    Json(gm.sys.provider.get_host_info().await)
}

pub async fn get_disks_handler(State(gm): State<Arc<GlobalManager>>) -> Json<Vec<DiskInfo>> {
    // 磁盘通常变动较慢，可以先触发一次 refresh_all 或依赖后台定时刷新
    // gm.sys.disks.refresh_all().await;
    Json(gm.sys.provider.get_disks().await)
}

// --- Network Handlers ---

pub async fn get_networks_handler(State(gm): State<Arc<GlobalManager>>) -> Json<Vec<NetworkInfo>> {
    // 获取前刷新以确保拿到最新的实时流量
    Json(gm.sys.provider.get_network_info().await)
}
