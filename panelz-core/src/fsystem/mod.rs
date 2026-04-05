use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::io::AsyncRead; // 需要 tokio

pub mod manager;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MountPoint {
    pub virt_path: String,     // 虚拟路径，例如 "/Photos"
    pub physical_root: String, // 物理根目录，例如 "/mnt/disk1/data"
    pub can_write: bool,       // 权限控制
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub extension: Option<String>, // 扩展名可能不存在
    pub mime_type: String,         // 增加 MIME 类型方便前端展示
}

#[async_trait]
pub trait ProvideFs: Send + Sync {
    // --- 路径转换 (核心安全逻辑) ---
    /// 将虚拟路径转换为物理路径，并校验是否越界（防止路径穿越攻击）
    fn resolve_path(&self, virt_path: &str) -> std::io::Result<PathBuf>;

    // --- 基础读取功能 ---
    async fn scan_directory(&self, path: &Path) -> std::io::Result<Vec<Entry>>;

    /// 使用流式读取代替 Vec<u8> (关键优化：避免大文件撑爆内存)
    /// 返回一个支持异步读取的 Trait Object
    async fn read_file_stream(
        &self,
        path: &Path,
    ) -> std::io::Result<Pin<Box<dyn AsyncRead + Send>>>;

    /// 范围读取优化：返回流而非全量载入内存
    async fn read_file_range(
        &self,
        path: &Path,
        offset: u64,
        length: u64,
    ) -> std::io::Result<Pin<Box<dyn AsyncRead + Send>>>;

    // --- 元数据与快速校验 ---
    async fn exists(&self, path: &Path) -> bool;
    async fn get_metadata(&self, path: &Path) -> std::io::Result<Entry>;

    // --- 写入与管理功能 ---
    async fn make_dir(&self, path: &Path, recursive: bool) -> std::io::Result<()>;
    async fn delete_item(&self, path: &Path, recursive: bool) -> std::io::Result<()>;

    /// 重命名或移动
    async fn move_item(&self, from: &Path, to: &Path) -> std::io::Result<()>;

    /// 大文件写入：建议支持流式写入或通过特定的 Handle
    async fn write_file(&self, path: &Path, data: &[u8]) -> std::io::Result<()>;
}
