use axum::middleware;
use axum::routing::post;
use axum::{Router, routing::get};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, services::ServeFile};

mod cfgtable;
mod globalmgr;
mod handlers;
mod providers;

use crate::globalmgr::GlobalManager;
use crate::handlers::usersys::{auth_admin, login_handler};
use crate::handlers::{fsystem, sysinfo};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_server().await
}

async fn run_server() -> anyhow::Result<()> {
    // --- 1. 初始化架构层 ---
    // 这里会加载 zconfig 和 各类 Manager
    let global_manager = Arc::new(GlobalManager::local().await);

    // --- 2. 路由拆分 ---
    let sys_routes = Router::new()
        .route("/cpu-info", get(sysinfo::get_cpu_handler))
        .route("/memory-info", get(sysinfo::get_memory_handler))
        .route("/process-info", get(sysinfo::get_processes_handler))
        .route("/host-info", get(sysinfo::get_base_info_handler))
        .route("/disk-info", get(sysinfo::get_disks_handler))
        .route("/network-info", get(sysinfo::get_networks_handler))
        .layer(middleware::from_fn_with_state(
            global_manager.clone(),
            auth_admin,
        ));

    let fs_routes = Router::new()
        // 获取类信息
        .route("/list/{*path}", get(fsystem::list_dir))
        .route("/stat/{*path}", get(fsystem::stat_file))
        .route("/download/{*path}", get(fsystem::download_file))
        // 动作类信息 (使用 POST)
        .route("/mkdir/{*path}", post(fsystem::make_dir))
        .route("/remove/{*path}", post(fsystem::remove_item))
        .route("/move", post(fsystem::move_item))
        .route("/upload/{*path}", post(fsystem::upload_file))
        // 统一添加 Admin 鉴权中间件
        .layer(middleware::from_fn_with_state(
            global_manager.clone(),
            auth_admin,
        ));

    // 后续开启 fs 和 user 路由...
    let user_routes = Router::new().route("/login", post(login_handler));

    let app = Router::new()
        .nest("/api/v1/sys", sys_routes)
        .nest("/api/v1/usr", user_routes)
        .nest("/api/v1/fsm", fs_routes)
        .route("/t", get(async || ""))
        .route_service("/", ServeFile::new("index.html"))
        .layer(CorsLayer::permissive())
        .with_state(global_manager);

    // --- 3. 启动监听 ---
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;

    println!("🚀 Panelz Instance 运行在: http://{}", addr);

    // 运行服务直到收到退出信号
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("Axum Server Error: {}", e))?;

    Ok(())
}
