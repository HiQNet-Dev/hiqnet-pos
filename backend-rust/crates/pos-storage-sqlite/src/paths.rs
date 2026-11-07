//! Resolución de la ruta de la base local por plataforma.
//!
//! La ruta debe ser coherente entre la aplicación Tauri, la utilidad de
//! respaldo/restauración, las pruebas y los instaladores. Windows conserva la
//! ubicación histórica; Linux respeta XDG; Android resuelve su almacenamiento
//! privado desde Tauri y no pasa por aquí.
//!
//! Si no se puede determinar un directorio persistente se devuelve un error en
//! lugar de crear una base definitiva en el directorio temporal.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Variable de entorno para aislar la base en desarrollo y QA.
pub const DATABASE_PATH_ENV: &str = "HIQNET_POS_DB_PATH";

/// No existe un directorio persistente fiable para la base.
#[derive(Debug, thiserror::Error)]
pub enum DatabasePathError {
    #[error(
        "No se pudo determinar un directorio de datos persistente para la base ({platform}). \
         Define HIQNET_POS_DB_PATH o corrige las variables de entorno del usuario."
    )]
    NoPersistentDataDirectory { platform: &'static str },
}

/// Plataforma lógica usada al resolver la ruta.
///
/// Se modela aparte de `cfg!(target_os)` para poder probar todas las ramas en
/// cualquier host, incluida la integración continua sobre Linux.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostPlatform {
    Windows,
    Linux,
    MacOs,
    Other,
}

impl HostPlatform {
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOs
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Self::Other
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::MacOs => "macos",
            Self::Other => "plataforma no soportada",
        }
    }
}

/// Ruta por defecto de la base local, fuera del repositorio.
///
/// - Windows: `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` (sin cambios).
/// - Linux: `$XDG_DATA_HOME/hiqnet-pos/data/hiqnet-pos.db` o
///   `~/.local/share/hiqnet-pos/data/hiqnet-pos.db`.
/// - macOS: `~/Library/Application Support/hiqnet-pos/data/hiqnet-pos.db`.
///
/// Nunca cae al directorio temporal: una instalación operativa debe abortar con
/// un diagnóstico si el sistema no ofrece un directorio persistente.
pub fn default_database_path() -> Result<PathBuf, DatabasePathError> {
    resolve_default_database_path(HostPlatform::current(), |name| std::env::var_os(name))
}

/// Base indicada por [`DATABASE_PATH_ENV`], si define una ruta no vacía.
pub fn database_path_from_env() -> Option<PathBuf> {
    non_empty(std::env::var_os(DATABASE_PATH_ENV)).map(PathBuf::from)
}

/// Base efectiva de la aplicación y de la CLI de respaldo.
///
/// El orden es: variable de entorno explícita y, si no existe, la ruta de la
/// plataforma. La automatización de escritorio antepone su propia ruta.
pub fn resolved_database_path() -> Result<PathBuf, DatabasePathError> {
    match database_path_from_env() {
        Some(path) => Ok(path),
        None => default_database_path(),
    }
}

fn resolve_default_database_path<F>(
    platform: HostPlatform,
    env: F,
) -> Result<PathBuf, DatabasePathError>
where
    F: Fn(&str) -> Option<OsString>,
{
    let missing = || DatabasePathError::NoPersistentDataDirectory {
        platform: platform.as_str(),
    };

    let base = match platform {
        HostPlatform::Windows => {
            let base = absolute_dir(env("LOCALAPPDATA"))
                .or_else(|| absolute_dir(env("USERPROFILE")))
                .ok_or_else(missing)?;
            // Ruta histórica: no debe cambiar para no perder bases existentes.
            return Ok(base
                .join("HiQNet")
                .join("POS")
                .join("data")
                .join("hiqnet-pos.db"));
        }
        HostPlatform::Linux => absolute_dir(env("XDG_DATA_HOME"))
            .or_else(|| absolute_dir(env("HOME")).map(|home| home.join(".local/share")))
            .ok_or_else(missing)?,
        HostPlatform::MacOs => absolute_dir(env("HOME"))
            .map(|home| home.join("Library/Application Support"))
            .ok_or_else(missing)?,
        HostPlatform::Other => return Err(missing()),
    };

    Ok(base.join("hiqnet-pos").join("data").join("hiqnet-pos.db"))
}

fn non_empty(value: Option<OsString>) -> Option<OsString> {
    value.filter(|value| !value.is_empty())
}

/// Directorio absoluto utilizable como raíz de datos.
///
/// La especificación XDG ignora rutas relativas; se aplica el mismo criterio a
/// las demás plataformas para no escribir la base dentro del directorio actual.
fn absolute_dir(value: Option<OsString>) -> Option<PathBuf> {
    non_empty(value)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path != Path::new("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn resolver(
        platform: HostPlatform,
        vars: &[(&str, &str)],
    ) -> Result<PathBuf, DatabasePathError> {
        let map: HashMap<String, OsString> = vars
            .iter()
            .map(|(key, value)| ((*key).to_string(), OsString::from(value)))
            .collect();
        resolve_default_database_path(platform, |name| map.get(name).cloned())
    }

    fn dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn expect_error(result: Result<PathBuf, DatabasePathError>) -> DatabasePathError {
        match result {
            Ok(path) => panic!("se esperaba error de ruta, se obtuvo {path:?}"),
            Err(error) => error,
        }
    }

    #[test]
    fn windows_conserva_la_estructura_historica() {
        let base = dir("hiqnet-localappdata");
        let path = resolver(
            HostPlatform::Windows,
            &[("LOCALAPPDATA", &base.to_string_lossy())],
        )
        .expect("LOCALAPPDATA absoluto");
        assert_eq!(
            path,
            base.join("HiQNet")
                .join("POS")
                .join("data")
                .join("hiqnet-pos.db")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_ruta_literal_historica() {
        let path = resolver(
            HostPlatform::Windows,
            &[("LOCALAPPDATA", r"C:\Users\ana\AppData\Local")],
        )
        .expect("ruta Windows válida");
        assert_eq!(
            path,
            PathBuf::from(r"C:\Users\ana\AppData\Local")
                .join("HiQNet")
                .join("POS")
                .join("data")
                .join("hiqnet-pos.db")
        );
    }

    #[test]
    fn windows_sin_directorio_persistente_falla() {
        let error = expect_error(resolver(HostPlatform::Windows, &[]));
        assert!(error.to_string().contains("HIQNET_POS_DB_PATH"));
    }

    #[test]
    fn windows_ignora_rutas_relativas() {
        let error = expect_error(resolver(
            HostPlatform::Windows,
            &[("LOCALAPPDATA", "datos-relativos")],
        ));
        let _ = error;
    }

    #[test]
    fn linux_usa_xdg_data_home_absoluto() {
        let xdg = dir("hiqnet-xdg-data");
        let path = resolver(
            HostPlatform::Linux,
            &[("XDG_DATA_HOME", &xdg.to_string_lossy())],
        )
        .expect("XDG absoluto");
        assert_eq!(
            path,
            xdg.join("hiqnet-pos").join("data").join("hiqnet-pos.db")
        );
    }

    #[test]
    fn linux_ignora_xdg_relativo_y_prefiere_home() {
        let home = dir("hiqnet-home");
        let path = resolver(
            HostPlatform::Linux,
            &[
                ("XDG_DATA_HOME", "datos-relativos"),
                ("HOME", &home.to_string_lossy()),
            ],
        )
        .expect("HOME absoluto");
        assert_eq!(
            path,
            home.join(".local")
                .join("share")
                .join("hiqnet-pos")
                .join("data")
                .join("hiqnet-pos.db")
        );
    }

    #[test]
    fn linux_sin_directorio_persistente_falla_sin_usar_temporal() {
        let error = expect_error(resolver(HostPlatform::Linux, &[]));
        assert!(matches!(
            error,
            DatabasePathError::NoPersistentDataDirectory { platform: "linux" }
        ));
    }

    #[test]
    fn macos_usa_application_support() {
        let home = dir("hiqnet-home-macos");
        let path = resolver(HostPlatform::MacOs, &[("HOME", &home.to_string_lossy())])
            .expect("HOME absoluto");
        assert_eq!(
            path,
            home.join("Library")
                .join("Application Support")
                .join("hiqnet-pos")
                .join("data")
                .join("hiqnet-pos.db")
        );
    }

    #[test]
    fn macos_sin_home_falla() {
        let _ = expect_error(resolver(HostPlatform::MacOs, &[("HOME", "")]));
    }

    #[test]
    fn plataforma_desconocida_falla() {
        let error = expect_error(resolver(
            HostPlatform::Other,
            &[("HOME", &dir("hiqnet-home-other").to_string_lossy())],
        ));
        assert!(matches!(
            error,
            DatabasePathError::NoPersistentDataDirectory { .. }
        ));
    }

    #[test]
    fn la_variable_de_entorno_no_vacia_gana_sobre_la_plataforma() {
        assert_eq!(non_empty(Some(OsString::from(""))), None);
        assert_eq!(
            non_empty(Some(OsString::from("/datos/qa.db"))),
            Some(OsString::from("/datos/qa.db"))
        );
    }

    #[test]
    fn la_ruta_por_defecto_no_usa_el_directorio_temporal() {
        let path = default_database_path().expect("este host tiene directorio persistente");
        assert!(
            path.is_absolute(),
            "la base debe resolverse absoluta: {path:?}"
        );
        assert!(path.ends_with("hiqnet-pos.db"));
        assert!(
            !path.starts_with(std::env::temp_dir()),
            "la base no debe resolverse en el temporal: {path:?}"
        );
    }
}
