//! Adaptador móvil mínimo para Android Storage Access Framework (SAF).
//!
//! El plugin no decide qué datos exportar ni valida respaldos. Solo transporta
//! archivos temporales privados elegidos por Rust a través de `ContentResolver`.

use serde::{Deserialize, Serialize};
#[cfg(desktop)]
use std::marker::PhantomData;
use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("el selector de documentos no está disponible en esta plataforma")]
    Unavailable,
    #[error("no fue posible completar la operación con el proveedor de documentos: {0}")]
    Plugin(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    /// Nombre sugerido; nunca contiene una ruta.
    pub display_name: String,
    /// Ruta privada creada por Rust, que el frontend nunca recibe.
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    /// Ruta privada nueva creada por Rust, que el frontend nunca recibe.
    pub target_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResult {
    /// `completed` o `cancelled`; permite que la UI trate cancelar como acción recuperable.
    pub status: String,
    pub bytes: Option<u64>,
}

#[cfg(mobile)]
mod mobile {
    use super::*;
    use serde::de::DeserializeOwned;
    use tauri::{
        AppHandle,
        plugin::{PluginApi, PluginHandle},
    };

    pub fn init<R: Runtime, C: DeserializeOwned>(
        _app: &AppHandle<R>,
        api: PluginApi<R, C>,
    ) -> Result<SafDocument<R>> {
        let handle = api
            .register_android_plugin("com.hiqnet.pos.safdocument", "SafDocumentPlugin")
            .map_err(|error| Error::Plugin(error.to_string()))?;
        Ok(SafDocument(handle))
    }

    pub struct SafDocument<R: Runtime>(PluginHandle<R>);

    impl<R: Runtime> SafDocument<R> {
        pub fn export_document(&self, request: ExportRequest) -> Result<DocumentResult> {
            self.0
                .run_mobile_plugin("exportDocument", request)
                .map_err(|error| Error::Plugin(error.to_string()))
        }

        pub fn import_document(&self, request: ImportRequest) -> Result<DocumentResult> {
            self.0
                .run_mobile_plugin("importDocument", request)
                .map_err(|error| Error::Plugin(error.to_string()))
        }
    }
}

#[cfg(desktop)]
mod desktop {
    use super::*;

    pub fn init<R: Runtime>(_app: &tauri::AppHandle<R>) -> Result<SafDocument<R>> {
        Ok(SafDocument(PhantomData))
    }

    pub struct SafDocument<R: Runtime>(pub(super) PhantomData<fn() -> R>);

    impl<R: Runtime> SafDocument<R> {
        pub fn export_document(&self, _request: ExportRequest) -> Result<DocumentResult> {
            Err(Error::Unavailable)
        }

        pub fn import_document(&self, _request: ImportRequest) -> Result<DocumentResult> {
            Err(Error::Unavailable)
        }
    }
}

#[cfg(desktop)]
use desktop::SafDocument;
#[cfg(mobile)]
use mobile::SafDocument;

pub trait SafDocumentExt<R: Runtime> {
    fn saf_document(&self) -> &SafDocument<R>;
}

impl<R: Runtime, T: Manager<R>> SafDocumentExt<R> for T {
    fn saf_document(&self) -> &SafDocument<R> {
        self.state::<SafDocument<R>>().inner()
    }
}

/// Registra el puente Android sin modificar `src-tauri/gen/android`.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("saf-document")
        .setup(|app, api| {
            #[cfg(desktop)]
            let _ = api;
            #[cfg(mobile)]
            let plugin = mobile::init(app, api)?;
            #[cfg(desktop)]
            let plugin = desktop::init(app)?;
            app.manage(plugin);
            Ok(())
        })
        .build()
}
