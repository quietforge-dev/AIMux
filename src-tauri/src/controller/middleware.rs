use crate::{
    app_state::ServerState, dao::user_dao, error::AppError, model::user::User,
    service::auth_service,
};
use axum::{extract::State, http::Request, middleware::Next, response::Response};
use std::sync::Arc;

#[derive(Clone)]
pub struct Identity {
    pub user: User,
    pub session_hash: Option<String>,
}
impl Identity {
    pub fn admin(&self) -> Result<(), AppError> {
        if self.user.role == "admin" {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}
pub async fn auth(
    State(state): State<Arc<ServerState>>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    let path = request.uri().path().to_owned();
    if path == "/health"
        || path == "/api/auth/login"
        || (!path.starts_with("/api/") && !path.starts_with("/v1/"))
    {
        return Ok(next.run(request).await);
    }
    let supplied = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            request
                .headers()
                .get("x-aimux-token")
                .and_then(|v| v.to_str().ok())
        })
        .or_else(|| {
            request
                .headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
        });
    let hash = supplied.map(auth_service::token_hash);
    let (user, session_hash) = if state.desktop {
        let user = user_dao::get(&state.pool, "local").await?;
        let runtime = state.user_state(&user).await?;
        let secret_valid = supplied.is_some() && supplied == runtime.desktop_secret.as_deref();
        let token_valid = hash
            .as_deref()
            .is_some_and(|h| user.api_token_hash.as_deref() == Some(h));
        // An empty legacy token keeps desktop compatibility until the owner explicitly sets one.
        if !secret_valid
            && !token_valid
            && (path.starts_with("/api/") || user.api_token_hash.is_some())
        {
            return Err(AppError::Unauthorized);
        }
        (user, None)
    } else {
        let hash = hash.as_deref().ok_or(AppError::Unauthorized)?;
        if path.starts_with("/v1/") {
            (
                user_dao::by_token(&state.pool, hash)
                    .await?
                    .ok_or(AppError::Unauthorized)?,
                None,
            )
        } else {
            (
                user_dao::by_session(&state.pool, hash)
                    .await?
                    .ok_or(AppError::Unauthorized)?,
                Some(hash.to_owned()),
            )
        }
    };
    let scoped = state.user_state(&user).await?;
    request.extensions_mut().insert(scoped);
    request
        .extensions_mut()
        .insert(Identity { user, session_hash });
    Ok(next.run(request).await)
}
