use crate::globalmgr::GlobalManager;
use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use panelz_core::usersys::UserRole;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub async fn auth_admin(
    State(gm): State<Arc<GlobalManager>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // --- 1. 提取 Token (支持 Header 或 Cookie) ---
    let auth_token = {
        let headers = request.headers();

        // A. 尝试从自定义 Header 获取
        let from_header = headers
            .get("z-authen")
            .and_then(|h| h.to_str().ok())
            .map(|h| h.trim().to_string());

        // B. 如果 Header 没有，尝试从 Cookie 获取
        from_header.or_else(|| {
            headers
                .get(header::COOKIE)
                .and_then(|h| h.to_str().ok())
                .and_then(|cookie_str| {
                    // 解析形如 "key1=val1; z-authen=YOUR_TOKEN; key2=val2" 的字符串
                    cookie_str
                        .split(';')
                        .map(|s| s.trim())
                        .find(|s| s.starts_with("z-authen="))
                        .map(|s| s["z-authen=".len()..].to_string())
                })
        })
    }
    .ok_or(StatusCode::UNAUTHORIZED)?;

    // --- 2. 校验 Session ---
    let user = gm
        .usr
        .provider
        .get_user_from_sessions(&auth_token) // 注意这里用了引用
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // --- 3. 权限校验 ---
    match user.role {
        UserRole::Admin => {
            request.extensions_mut().insert(user);
            Ok(next.run(request).await)
        }
        _ => Err(StatusCode::FORBIDDEN),
    }
}

#[derive(Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

pub async fn login_handler(
    State(gm): State<Arc<GlobalManager>>,
    Json(payload): Json<LoginPayload>,
) -> Result<impl IntoResponse, StatusCode> {
    let token = gm
        .usr
        .provider
        .set_user_session(&payload.username, &payload.password)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    // 5. 返回结果
    println!("{}", token);
    Ok(Json(LoginResponse { token }))
}
