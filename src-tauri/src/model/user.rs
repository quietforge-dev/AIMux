use serde::Serialize;

#[derive(Clone, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub enabled: bool,
    pub api_token_hash: Option<String>,
    pub api_token_prefix: String,
    pub api_token_updated_at: Option<String>,
    pub initialized: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize)]
pub struct UserView {
    pub id: String,
    pub username: String,
    pub role: String,
    pub enabled: bool,
    pub api_token_prefix: String,
    pub api_token_updated_at: Option<String>,
    pub created_at: String,
}
impl From<User> for UserView {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role: user.role,
            enabled: user.enabled,
            api_token_prefix: user.api_token_prefix,
            api_token_updated_at: user.api_token_updated_at,
            created_at: user.created_at,
        }
    }
}
