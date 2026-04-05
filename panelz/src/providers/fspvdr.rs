use async_trait::async_trait;
use panelz_core::fsystem::{Entry, ProvideFs};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt};

#[derive(Deserialize)]
struct MountConfig {
    // JSON 中的键名，例如 {"virt": "/Photos", "phys": "/mnt/disk1"}
    virt: String,
    phys: String,
}

/// 本地文件系统提供者
/// 支持将多个物理目录映射到不同的虚拟路径
pub struct LocalFsProvider {
    /// 映射表：Key = 虚拟路径 (如 "/Photos"), Value = 物理根目录 (如 "/mnt/data")
    mounts: Arc<HashMap<String, String>>,
}

impl LocalFsProvider {
    pub async fn new(config_path: &str) -> Self {
        let mut mounts = HashMap::new();

        match fs::read_to_string(config_path).await {
            Ok(content) => {
                // 解析 JSON 数组
                match serde_json::from_str::<Vec<MountConfig>>(&content) {
                    Ok(config_list) => {
                        for item in config_list {
                            let v = item.virt.trim();
                            let p = item.phys.trim();

                            // 路径规范化逻辑
                            let v_key = if v.starts_with('/') {
                                v.to_string()
                            } else {
                                format!("/{}", v)
                            };
                            let v_key = if v_key.len() > 1 {
                                v_key.trim_end_matches('/').to_string()
                            } else {
                                v_key
                            };

                            mounts.insert(v_key, p.to_string());
                        }
                    }
                    Err(e) => eprintln!("❌ JSON 格式错误: {}", e),
                }
                println!("🚀 Panelz FS: 已从 JSON 加载 {} 个挂载点", mounts.len());
            }
            Err(e) => {
                eprintln!("❌ 无法读取文件系统配置 {}: {}", config_path, e);
            }
        }

        Self {
            mounts: Arc::new(mounts),
        }
    }

    /// 内部逻辑：查找最匹配的挂载点
    /// 返回：(计算出的物理路径, 该挂载点的物理根目录)
    fn find_mount(&self, virt_path: &str) -> std::io::Result<(PathBuf, String)> {
        let normalized = if virt_path.starts_with('/') {
            virt_path.to_string()
        } else {
            format!("/{}", virt_path)
        };

        let mut best_v: Option<&String> = None;
        let mut best_p: Option<&String> = None;

        // 寻找最长前缀匹配 (Longest Prefix Match)
        for (v, p) in self.mounts.iter() {
            if normalized.starts_with(v) {
                // 确保是路径分隔符匹配，防止 /test 匹配到 /testing
                if normalized.len() == v.len() || normalized.as_bytes()[v.len()] == b'/' {
                    if best_v.is_none() || v.len() > best_v.unwrap().len() {
                        best_v = Some(v);
                        best_p = Some(p);
                    }
                }
            }
        }

        if let (Some(v), Some(p)) = (best_v, best_p) {
            let relative = normalized.strip_prefix(v).unwrap_or("");
            let relative_trimmed = relative.strip_prefix('/').unwrap_or(relative);

            let mut full_phys = PathBuf::from(p);
            if !relative_trimmed.is_empty() {
                full_phys.push(relative_trimmed);
            }
            Ok((full_phys, p.clone()))
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("未定义挂载点: {}", virt_path),
            ))
        }
    }
}

#[async_trait]
impl ProvideFs for LocalFsProvider {
    /// 安全路径转换：映射虚拟路径并防止 .. 逃逸
    fn resolve_path(&self, virt_path: &str) -> std::io::Result<PathBuf> {
        let (phys_path, phys_root) = self.find_mount(virt_path)?;

        // 核心安全：检查结果是否超出了对应的物理根目录
        // 预防例如 /Photos/../../etc/passwd 这种攻击
        if phys_path.starts_with(Path::new(&phys_root)) {
            Ok(phys_path)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "检测到路径穿越攻击 (Path Traversal)",
            ))
        }
    }

    async fn scan_directory(&self, virt_path: &Path) -> std::io::Result<Vec<Entry>> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy())?;
        let mut entries = Vec::new();
        let mut read_dir = fs::read_dir(real_path).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            let meta = entry.metadata().await?;
            let p = entry.path();

            entries.push(Entry {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: meta.is_dir(),
                size: meta.len(),
                modified: meta
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                extension: p
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string()),
                mime_type: if meta.is_dir() {
                    "inode/directory".into()
                } else {
                    mime_guess::from_path(&p)
                        .first_or_octet_stream()
                        .to_string()
                },
            });
        }
        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
        Ok(entries)
    }

    async fn read_file_stream(
        &self,
        virt_path: &Path,
    ) -> std::io::Result<Pin<Box<dyn AsyncRead + Send>>> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy())?;
        let file = File::open(real_path).await?;
        Ok(Box::pin(file))
    }

    async fn read_file_range(
        &self,
        virt_path: &Path,
        offset: u64,
        length: u64,
    ) -> std::io::Result<Pin<Box<dyn AsyncRead + Send>>> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy())?;
        let mut file = File::open(real_path).await?;
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        Ok(Box::pin(file.take(length)))
    }

    async fn exists(&self, virt_path: &Path) -> bool {
        if let Ok(real_path) = self.resolve_path(&virt_path.to_string_lossy()) {
            fs::metadata(real_path).await.is_ok()
        } else {
            false
        }
    }

    async fn get_metadata(&self, virt_path: &Path) -> std::io::Result<Entry> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy())?;
        let meta = fs::metadata(&real_path).await?;
        Ok(Entry {
            name: virt_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            is_dir: meta.is_dir(),
            size: meta.len(),
            modified: meta
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            extension: virt_path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string()),
            mime_type: if meta.is_dir() {
                "inode/directory".into()
            } else {
                mime_guess::from_path(&real_path)
                    .first_or_octet_stream()
                    .to_string()
            },
        })
    }

    async fn make_dir(&self, virt_path: &Path, recursive: bool) -> std::io::Result<()> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy())?;
        if recursive {
            fs::create_dir_all(real_path).await
        } else {
            fs::create_dir(real_path).await
        }
    }

    async fn delete_item(&self, virt_path: &Path, recursive: bool) -> std::io::Result<()> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy())?;
        let meta = fs::metadata(&real_path).await?;
        if meta.is_dir() {
            if recursive {
                fs::remove_dir_all(real_path).await
            } else {
                fs::remove_dir(real_path).await
            }
        } else {
            fs::remove_file(real_path).await
        }
    }

    async fn move_item(&self, from_virt: &Path, to_virt: &Path) -> std::io::Result<()> {
        let src = self.resolve_path(&from_virt.to_string_lossy())?;
        let dst = self.resolve_path(&to_virt.to_string_lossy())?;
        fs::rename(src, dst).await
    }

    async fn write_file(&self, virt_path: &Path, data: &[u8]) -> std::io::Result<()> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy())?;
        let mut file = File::create(real_path).await?;
        use tokio::io::AsyncWriteExt;
        file.write_all(data).await?;
        Ok(())
    }
}
