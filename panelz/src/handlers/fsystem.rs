use crate::globalmgr::GlobalManager;
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{Response, StatusCode, header},
    response::IntoResponse,
};
use panelz_core::fsystem::Entry;
use serde::Deserialize;
use std::path::Path as StdPath;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct FsQuery {
    pub recursive: Option<bool>,
}

/// 1. 获取目录列表 /api/v1/fs/list/*path
pub async fn list_dir(
    State(mgr): State<Arc<GlobalManager>>,
    Path(path): Path<String>,
) -> Result<Json<Vec<Entry>>, StatusCode> {
    let virt_path = StdPath::new(&path);
    mgr.fsm
        .provider
        .scan_directory(virt_path)
        .await
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// 2. 获取文件详情 /api/v1/fs/stat/*path
pub async fn stat_file(
    State(mgr): State<Arc<GlobalManager>>,
    Path(path): Path<String>,
) -> Result<Json<Entry>, StatusCode> {
    let virt_path = StdPath::new(&path);
    mgr.fsm
        .provider
        .get_metadata(virt_path)
        .await
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// 3. 创建目录 /api/v1/fs/mkdir/*path
pub async fn make_dir(
    State(mgr): State<Arc<GlobalManager>>,
    Path(path): Path<String>,
    Query(query): Query<FsQuery>,
) -> impl IntoResponse {
    let virt_path = StdPath::new(&path);
    match mgr
        .fsm
        .provider
        .make_dir(virt_path, query.recursive.unwrap_or(false))
        .await
    {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// 4. 删除文件/目录 /api/v1/fs/remove/*path
pub async fn remove_item(
    State(mgr): State<Arc<GlobalManager>>,
    Path(path): Path<String>,
    Query(query): Query<FsQuery>,
) -> impl IntoResponse {
    let virt_path = StdPath::new(&path);
    match mgr
        .fsm
        .provider
        .delete_item(virt_path, query.recursive.unwrap_or(false))
        .await
    {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// 5. 下载文件 (流式) /api/v1/fs/download/*path
pub async fn download_file(
    State(mgr): State<Arc<GlobalManager>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let virt_path = StdPath::new(&path);

    // 获取流
    let stream = match mgr.fsm.provider.read_file_stream(virt_path).await {
        Ok(s) => s,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    // 转换为 Body
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(stream));

    let filename = virt_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());

    Ok(Response::builder()
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(body)
        .unwrap())
}

/// 6. 重命名/移动 /api/v1/fs/move
#[derive(Deserialize)]
pub struct MoveRequest {
    pub from: String,
    pub to: String,
}

pub async fn move_item(
    State(mgr): State<Arc<GlobalManager>>,
    Json(payload): Json<MoveRequest>,
) -> impl IntoResponse {
    match mgr
        .fsm
        .provider
        .move_item(StdPath::new(&payload.from), StdPath::new(&payload.to))
        .await
    {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// 7. 上传文件 /api/v1/fs/upload/*path
pub async fn upload_file(
    State(mgr): State<Arc<GlobalManager>>,
    Path(path): Path<String>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let virt_path = StdPath::new(&path);

    while let Ok(Some(field)) = multipart.next_field().await {
        if let Ok(data) = field.bytes().await {
            // 注意：这里为了简化直接写入。生产环境建议处理流式写入防止大文件内存溢出。
            if mgr.fsm.provider.write_file(virt_path, &data).await.is_err() {
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        }
    }
    StatusCode::OK
}
