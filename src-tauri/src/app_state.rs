use crate::{
    config::{Settings, SharedSettings},
    dao::{settings_dao, user_dao},
    database::{self, UserDb},
    error::AppError,
    model::user::User,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};

/// An immutable tenant identity with its own mutable settings and policy counters.
pub struct AppState {
    pub pool: UserDb,
    pub settings: SharedSettings,
    pub cache_priority: crate::service::cache_priority_service::SharedCachePriority,
    pub desktop: bool,
    pub desktop_secret: Option<String>,
}
struct UserRuntime {
    state: Arc<AppState>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for UserRuntime {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
pub struct ServerState {
    pub pool: sqlx::SqlitePool,
    pub desktop: bool,
    pub runtime_settings: Settings,
    users: Mutex<HashMap<String, UserRuntime>>,
    pub login_attempts: Mutex<HashMap<String, (u32, std::time::Instant)>>,
    pub login_slots: tokio::sync::Semaphore,
}
impl ServerState {
    pub async fn initialize(settings: Settings, desktop: bool) -> Result<Arc<Self>, AppError> {
        let pool = database::connect(&settings.database_path()).await?;
        let owner = user_dao::get(&pool, "local").await?;
        if !owner.initialized {
            crate::service::settings_service::validate(&settings).map_err(AppError::BadRequest)?;
            let (username, password_hash) = if desktop {
                ("admin".to_string(), String::new())
            } else {
                let username = std::env::var("AIMUX_ADMIN_USERNAME").map_err(|_| {
                    AppError::Internal("首次启动 Web 版必须设置 AIMUX_ADMIN_USERNAME".into())
                })?;
                crate::service::auth_service::validate_username(&username)?;
                let password = std::env::var("AIMUX_ADMIN_PASSWORD").map_err(|_| {
                    AppError::Internal("首次启动 Web 版必须设置 AIMUX_ADMIN_PASSWORD".into())
                })?;
                (
                    username,
                    crate::service::auth_service::hash_password(&password).await?,
                )
            };
            let hash = (!settings.local_token.is_empty())
                .then(|| crate::service::auth_service::token_hash(&settings.local_token));
            let prefix = crate::service::auth_service::prefix(&settings.local_token);
            user_dao::initialize_owner(
                &pool,
                &username,
                &password_hash,
                hash.as_deref().map(|h| (h, prefix.as_str())),
                &settings,
            )
            .await?;
        } else if !desktop && owner.password_hash.is_empty() {
            let username = std::env::var("AIMUX_ADMIN_USERNAME").map_err(|_| {
                AppError::Internal("首次启动 Web 版必须设置 AIMUX_ADMIN_USERNAME".into())
            })?;
            crate::service::auth_service::validate_username(&username)?;
            let password = std::env::var("AIMUX_ADMIN_PASSWORD").map_err(|_| {
                AppError::Internal("首次启动 Web 版必须设置 AIMUX_ADMIN_PASSWORD".into())
            })?;
            let hash = crate::service::auth_service::hash_password(&password).await?;
            user_dao::initialize_web_credentials(&pool, &username, &hash).await?;
        }
        let state = Arc::new(Self {
            pool,
            desktop,
            runtime_settings: settings,
            users: Mutex::new(HashMap::new()),
            login_attempts: Mutex::new(HashMap::new()),
            login_slots: tokio::sync::Semaphore::new(2),
        });
        for id in user_dao::ids(&state.pool).await? {
            if desktop && id != "local" {
                continue;
            }
            let db = UserDb::new(state.pool.clone(), id);
            crate::dao::usage_dao::fail_unfinished_streams(
                &db,
                &crate::utils::time::utc_now_string(),
            )
            .await?;
        }
        Ok(state)
    }
    pub async fn user_state(&self, user: &User) -> Result<Arc<AppState>, AppError> {
        if !user.enabled || (self.desktop && user.id != "local") {
            return Err(AppError::Unauthorized);
        }
        let mut users = self.users.lock().await;
        let current = user_dao::get(&self.pool, &user.id).await?;
        if !current.enabled {
            return Err(AppError::Unauthorized);
        }
        if let Some(runtime) = users.get(&user.id) {
            return Ok(runtime.state.clone());
        }
        let db = UserDb::new(self.pool.clone(), user.id.clone());
        let settings = settings_dao::load(&db, &self.runtime_settings).await?;
        crate::service::model_service::seed(&db).await?;
        let state = Arc::new(AppState {
            pool: db,
            cache_priority: Arc::new(Mutex::new(
                crate::service::cache_priority_service::CachePriorityTracker::new(&settings),
            )),
            settings: Arc::new(RwLock::new(settings)),
            desktop: self.desktop,
            desktop_secret: self.desktop.then(crate::service::auth_service::new_token),
        });
        users.insert(
            user.id.clone(),
            UserRuntime {
                state: state.clone(),
                tasks: Vec::new(),
            },
        );
        Ok(state)
    }
    #[cfg(test)]
    pub(crate) fn testing(pool: sqlx::SqlitePool, settings: Settings, desktop: bool) -> Arc<Self> {
        Arc::new(Self {
            pool,
            desktop,
            runtime_settings: settings,
            users: Mutex::new(HashMap::new()),
            login_attempts: Mutex::new(HashMap::new()),
            login_slots: tokio::sync::Semaphore::new(2),
        })
    }
    pub async fn stop_user(&self, id: &str) {
        self.users.lock().await.remove(id);
    }
    pub async fn supervise(self: Arc<Self>) {
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(5));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            timer.tick().await;
            if let Err(e) = self.sync_tasks().await {
                tracing::error!(error=%e,"同步用户后台任务失败");
            }
        }
    }
    async fn sync_tasks(&self) -> Result<(), AppError> {
        let users = user_dao::enabled(&self.pool).await?;
        let ids: Vec<_> = users
            .iter()
            .filter(|u| !self.desktop || u.id == "local")
            .map(|u| u.id.clone())
            .collect();
        self.users.lock().await.retain(|id, _| ids.contains(id));
        for user in users {
            if self.desktop && user.id != "local" {
                continue;
            }
            let state = match self.user_state(&user).await {
                Ok(state) => state,
                Err(AppError::Unauthorized) => continue,
                Err(error) => {
                    tracing::error!(user_id=%user.id,%error,"加载用户后台配置失败");
                    continue;
                }
            };
            let mut runtimes = self.users.lock().await;
            let Some(runtime) = runtimes.get_mut(&user.id) else {
                continue;
            };
            if runtime.tasks.is_empty() {
                runtime.tasks = vec![
                    tokio::spawn(crate::background::account_status_task::run(state.clone())),
                    tokio::spawn(crate::background::daily_statistics_task::run(state.clone())),
                    tokio::spawn(crate::background::monitor_task::run(state.clone())),
                    tokio::spawn(crate::background::multiplier_monitor_task::run(state)),
                ];
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn disabled_users_stop_tasks_and_stale_identities_cannot_restart_them() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let settings = Settings {
            monitoring_enabled: false,
            ..Settings::default()
        };
        user_dao::initialize_owner(&pool, "owner", "fixture", None, &settings)
            .await
            .unwrap();
        let (user, _) = crate::service::auth_service::create_user(&pool, "colleague", "password")
            .await
            .unwrap();
        let state = ServerState::testing(pool, settings, false);
        state.sync_tasks().await.unwrap();
        assert_eq!(state.users.lock().await.len(), 2);
        assert_eq!(state.users.lock().await[&user.id].tasks.len(), 4);
        let runtime = state.user_state(&user).await.unwrap();
        let owner = state
            .user_state(&user_dao::get(&state.pool, "local").await.unwrap())
            .await
            .unwrap();
        assert!(!Arc::ptr_eq(&owner.settings, &runtime.settings));
        assert!(!Arc::ptr_eq(&owner.cache_priority, &runtime.cache_priority));
        user_dao::set_enabled(&state.pool, &user.id, false)
            .await
            .unwrap();
        state.stop_user(&user.id).await;
        assert!(state.user_state(&user).await.is_err());
        state.sync_tasks().await.unwrap();
        assert!(!state.users.lock().await.contains_key(&user.id));
        user_dao::set_enabled(&state.pool, &user.id, true)
            .await
            .unwrap();
        state.sync_tasks().await.unwrap();
        assert_eq!(state.users.lock().await[&user.id].tasks.len(), 4);
    }
}
