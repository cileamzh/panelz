use async_trait::async_trait;
use panelz_core::filesys::{Entry, MountPoint, ProvideFs};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt};

#[derive(Deserialize)]
struct MountConfig {
    virt: String,
    phys: String,
    #[serde(default = "default_can_write")]
    can_write: bool, // 建议在 JSON 中加入权限控制，默认给 true
}

fn default_can_write() -> bool {
    true
}

pub struct LocalFsProvider {
    /// 修改点：确保这里存储的是完整的 MountPoint 结构体
    mounts: Arc<HashMap<String, MountPoint>>,
}

impl LocalFsProvider {
    pub async fn new(config_path: &str) -> Self {
        let mut mounts = HashMap::new();

        if let Ok(content) = fs::read_to_string(config_path).await {
            if let Ok(config_list) = serde_json::from_str::<Vec<MountConfig>>(&content) {
                for item in config_list {
                    let v = item.virt.trim();
                    let p = item.phys.trim();

                    // 1. 规范化虚拟路径 key
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

                    // 2. 构造 MountPoint 存入 map
                    mounts.insert(
                        v_key.clone(),
                        MountPoint {
                            virt_path: v_key,
                            physical_root: p.to_string(),
                            can_write: item.can_write,
                        },
                    );
                }
            }
        }

        Self {
            mounts: Arc::new(mounts),
        }
    }

    /// 内部逻辑优化：直接返回匹配到的 MountPoint 引用
    fn find_mount_point(&self, virt_path: &str) -> std::io::Result<&MountPoint> {
        let normalized = if virt_path.starts_with('/') {
            virt_path.to_string()
        } else {
            format!("/{}", virt_path)
        };
        let normalized = if normalized.len() > 1 {
            normalized.trim_end_matches('/').to_string()
        } else {
            normalized
        };

        self.mounts
            .iter()
            .filter(|(v, _)| normalized == **v || normalized.starts_with(&format!("{}/", v)))
            .max_by_key(|(v, _)| v.len())
            .map(|(_, mp)| mp)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("未定义挂载点: {}", virt_path),
                )
            })
    }
}

#[async_trait]
impl ProvideFs for LocalFsProvider {
    /// 修复点：正确填充并返回 MountPoint
    async fn validate_virtual_path(&self, virt_path: &str) -> std::io::Result<MountPoint> {
        let mp = self.find_mount_point(virt_path)?;
        Ok(mp.clone()) // 假设 MountPoint 实现了 Clone
    }

    async fn resolve_path(&self, virt_path: &str) -> std::io::Result<PathBuf> {
        let mp = self.find_mount_point(virt_path)?;

        let normalized = if virt_path.starts_with('/') {
            virt_path.to_string()
        } else {
            format!("/{}", virt_path)
        };
        let relative = normalized.strip_prefix(&mp.virt_path).unwrap_or("");
        let relative_trimmed = relative.strip_prefix('/').unwrap_or(relative);

        let mut full_phys = PathBuf::from(&mp.physical_root);
        if !relative_trimmed.is_empty() {
            full_phys.push(relative_trimmed);
        }

        // 安全检查：物理路径必须以物理根目录开头（防止 ../ 逃逸）
        if full_phys.starts_with(&mp.physical_root) {
            Ok(full_phys)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "路径逃逸检测",
            ))
        }
    }

    async fn scan_directory(&self, virt_path: &Path) -> std::io::Result<Vec<Entry>> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy()).await?;
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
        let real_path = self.resolve_path(&virt_path.to_string_lossy()).await?;
        let file = File::open(real_path).await?;
        Ok(Box::pin(file))
    }

    async fn read_file_range(
        &self,
        virt_path: &Path,
        offset: u64,
        length: u64,
    ) -> std::io::Result<Pin<Box<dyn AsyncRead + Send>>> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy()).await?;
        let mut file = File::open(real_path).await?;
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        Ok(Box::pin(file.take(length)))
    }

    async fn exists(&self, virt_path: &Path) -> bool {
        self.resolve_path(&virt_path.to_string_lossy())
            .await
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    async fn get_metadata(&self, virt_path: &Path) -> std::io::Result<Entry> {
        let real_path = self.resolve_path(&virt_path.to_string_lossy()).await?;
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
        let mp = self
            .validate_virtual_path(&virt_path.to_string_lossy())
            .await?;
        if !mp.can_write {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "ReadOnly",
            ));
        }
        let real_path = self.resolve_path(&virt_path.to_string_lossy()).await?;
        if recursive {
            fs::create_dir_all(real_path).await
        } else {
            fs::create_dir(real_path).await
        }
    }

    async fn delete_item(&self, virt_path: &Path, recursive: bool) -> std::io::Result<()> {
        let mp = self
            .validate_virtual_path(&virt_path.to_string_lossy())
            .await?;
        if !mp.can_write {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "ReadOnly",
            ));
        }
        let real_path = self.resolve_path(&virt_path.to_string_lossy()).await?;
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
        let mp = self
            .validate_virtual_path(&from_virt.to_string_lossy())
            .await?;
        if !mp.can_write {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "ReadOnly",
            ));
        }
        let src = self.resolve_path(&from_virt.to_string_lossy()).await?;
        let dst = self.resolve_path(&to_virt.to_string_lossy()).await?;
        fs::rename(src, dst).await
    }

    async fn write_file(&self, virt_path: &Path, data: &[u8]) -> std::io::Result<()> {
        let mp = self
            .validate_virtual_path(&virt_path.to_string_lossy())
            .await?;
        if !mp.can_write {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "ReadOnly",
            ));
        }
        let real_path = self.resolve_path(&virt_path.to_string_lossy()).await?;
        let mut file = File::create(real_path).await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, data).await?;
        Ok(())
    }
}
