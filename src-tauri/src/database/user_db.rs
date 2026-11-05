use sqlx::SqlitePool;

/// Business services receive this scope, never a caller-supplied user ID or an unscoped pool.
#[derive(Clone)]
pub struct UserDb {
    pool: SqlitePool,
    user_id: String,
}

impl UserDb {
    pub(crate) fn new(pool: SqlitePool, user_id: impl Into<String>) -> Self {
        Self {
            pool,
            user_id: user_id.into(),
        }
    }
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
    pub(crate) fn raw(&self) -> &SqlitePool {
        &self.pool
    }
}

impl std::ops::Deref for UserDb {
    type Target = SqlitePool;
    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}
