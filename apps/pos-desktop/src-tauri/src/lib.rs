//! Punto de entrada Tauri 2 y comandos IPC tipados.
//!
//! Toda regla de negocio vive en Rust. El frontend solo presenta estado y envía
//! comandos; la autorización se resuelve en el backend con el actor de sesión.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use pos_application::{
    AppError, AppStatus, AuditEventView, BootstrapRequest, BootstrapResponse,
    CommercialStateView, LoginRequest, PosService, ProductDraft, ProductView, SharedClock,
    SharedHasher, SharedStore, SystemClock,
};
use pos_core::identity::{Actor, Permission};
use pos_security::Argon2idHasher;
use pos_storage_sqlite::SqliteStore;
use serde::Serialize;
use tauri::{Manager, State};

struct AppState {
    db_path: PathBuf,
    pool: sqlx::SqlitePool,
    service: PosService,
    session: RwLock<Option<Actor>>,
}

#[derive(Debug, Serialize)]
struct IpcError {
    code: &'static str,
    message: String,
}

impl From<pos_core::DomainError> for IpcError {
    fn from(error: pos_core::DomainError) -> Self {
        AppError::from(error).into()
    }
}

impl From<AppError> for IpcError {
    fn from(error: AppError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

type IpcResult<T> = Result<T, IpcError>;

fn session_actor(state: &AppState) -> IpcResult<Actor> {
    state
        .session
        .read()
        .map_err(|_| AppError::Storage)?
        .clone()
        .ok_or(AppError::Unauthenticated.into())
}

#[tauri::command]
async fn app_status(state: State<'_, AppState>) -> IpcResult<AppStatus> {
    Ok(state.service.status().await?)
}

#[tauri::command]
async fn bootstrap(
    state: State<'_, AppState>,
    request: BootstrapRequest,
) -> IpcResult<BootstrapResponse> {
    Ok(state.service.bootstrap(request).await?)
}

#[tauri::command]
async fn login(state: State<'_, AppState>, request: LoginRequest) -> IpcResult<Actor> {
    let actor = state.service.login(request).await?;
    *state.session.write().map_err(|_| IpcError {
        code: "storage_error",
        message: "no fue posible conservar la sesión".into(),
    })? = Some(actor.clone());
    Ok(actor)
}

#[tauri::command]
fn logout(state: State<'_, AppState>) -> IpcResult<()> {
    *state.session.write().map_err(|_| IpcError {
        code: "storage_error",
        message: "no fue posible cerrar la sesión".into(),
    })? = None;
    Ok(())
}

#[tauri::command]
fn current_session(state: State<'_, AppState>) -> IpcResult<Option<Actor>> {
    Ok(state
        .session
        .read()
        .map_err(|_| AppError::Storage)?
        .clone())
}

#[tauri::command]
async fn create_product(
    state: State<'_, AppState>,
    draft: ProductDraft,
) -> IpcResult<ProductView> {
    let actor = session_actor(&state)?;
    Ok(state.service.create_product(&actor, draft).await?)
}

#[tauri::command]
async fn list_products(state: State<'_, AppState>) -> IpcResult<Vec<ProductView>> {
    let actor = session_actor(&state)?;
    Ok(state.service.list_products(&actor).await?)
}

#[tauri::command]
async fn list_audit_events(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> IpcResult<Vec<AuditEventView>> {
    let actor = session_actor(&state)?;
    Ok(state.service.list_audit(&actor, limit.unwrap_or(50)).await?)
}

#[tauri::command]
async fn commercial_state(state: State<'_, AppState>) -> IpcResult<CommercialStateView> {
    Ok(state.service.commercial_state().await?)
}

/// Crea un respaldo consistente de la base local y lo registra en auditoría.
#[tauri::command]
async fn backup_database(state: State<'_, AppState>) -> IpcResult<String> {
    let actor = session_actor(&state)?;
    actor.require(Permission::ManageBackups)?;

    let stamp = time::OffsetDateTime::now_utc().unix_timestamp();
    let target = state
        .db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("backups")
        .join(format!("hiqnet-pos-{stamp}.db"));

    let store = SqliteStore::new(state.pool.clone());
    pos_storage_sqlite::backup_to(store.pool(), &target)
        .await
        .map_err(|_| AppError::Storage)?;
    state
        .service
        .record_backup(&actor, &target.to_string_lossy())
        .await?;
    Ok(target.to_string_lossy().to_string())
}

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let db_path = std::env::var_os("HIQNET_POS_DB_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(pos_storage_sqlite::default_database_path);

            let pool = tauri::async_runtime::block_on(pos_storage_sqlite::open(&db_path))
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;

            let store: SharedStore = Arc::new(SqliteStore::new(pool.clone()));
            let hasher: SharedHasher = Arc::new(Argon2idHasher::new());
            let clock: SharedClock = Arc::new(SystemClock);
            let service = PosService::new(store, hasher, clock);

            app.manage(AppState {
                db_path,
                pool,
                service,
                session: RwLock::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            bootstrap,
            login,
            logout,
            current_session,
            create_product,
            list_products,
            list_audit_events,
            commercial_state,
            backup_database
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar HiQNet POS");
}
