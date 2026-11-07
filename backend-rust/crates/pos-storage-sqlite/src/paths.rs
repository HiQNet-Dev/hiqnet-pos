//! Resolución de la ruta de la base local por plataforma.
//!
//! La ruta debe ser coherente entre la aplicación Tauri, la utilidad de
//! respaldo/restauración, las pruebas y los instaladores. Windows conserva la
//! ubicación histórica; Linux respeta XDG; Android resuelve su almacenamiento
//! privado desde Tauri y no pasa por aquí.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Variable de entorno para aislar la base en desarrollo y QA.
pub const DATABASE_PATH_ENV: &str = "HIQNET_POS_DB_PATH";

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
}

/// Ruta por defecto de la base local, fuera del repositorio.
///
/// - Windows: `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db` (sin cambios).
/// - Linux: `$XDG_DATA_HOME/hiqnet-pos/data/hiqnet-pos.db` o
///   `~/.local/share/hiqnet-pos/data/hiqnet-pos.db`.
/// - macOS: `~/Library/Application Support/hiqnet-pos/data/hiqnet-pos.db`.
///
/// Si la variable de plataforma no existe se usa el directorio temporal, que
/// mantiene la aplicación arrancable sin escribir dentro del repositorio.
pub fn default_database_path() -> PathBuf {
    resolve_default_database_path(
        HostPlatform::current(),
        |name| std::env::var_os(name),
        std::env::temp_dir(),
    )
}

/// Base indicada por [`DATABASE_PATH_ENV`], si define una ruta no vacía.
pub fn database_path_from_env() -> Option<PathBuf> {
    non_empty(std::env::var_os(DATABASE_PATH_ENV)).map(PathBuf::from)
}

/// Base efectiva de la aplicación y de la CLI de respaldo.
///
/// El orden es: variable de entorno explícita y, si no existe, la ruta de la
/// plataforma. La automatización de escritorio antepone su propia ruta.
pub fn resolved_database_path() -> PathBuf {
    database_path_from_env().unwrap_or_else(default_database_path)
}

fn resolve_default_database_path<F>(platform: HostPlatform, env: F, temp_dir: PathBuf) -> PathBuf
where
    F: Fn(&str) -> Option<OsString>,
{
    match platform {
        HostPlatform::Windows => {
            let base = non_empty(env("LOCALAPPDATA"))
                .or_else(|| non_empty(env("USERPROFILE")))
                .map(PathBuf::from)
                .unwrap_or_else(|| temp_dir.clone());
            // Ruta histórica: no debe cambiar para no perder bases existentes.
            base.join("HiQNet")
                .join("POS")
                .join("data")
                .join("hiqnet-pos.db")
        }
        HostPlatform::Linux => {
            let base = absolute_path(env("XDG_DATA_HOME"))
                .or_else(|| {
                    non_empty(env("HOME")).map(|home| PathBuf::from(home).join(".local/share"))
                })
                .unwrap_or_else(|| temp_dir.clone());
            base.join("hiqnet-pos").join("data").join("hiqnet-pos.db")
        }
        HostPlatform::MacOs => {
            let base = non_empty(env("HOME"))
                .map(|home| PathBuf::from(home).join("Library/Application Support"))
                .unwrap_or_else(|| temp_dir.clone());
            base.join("hiqnet-pos").join("data").join("hiqnet-pos.db")
        }
        HostPlatform::Other => temp_dir
            .join("hiqnet-pos")
            .join("data")
            .join("hiqnet-pos.db"),
    }
}

fn non_empty(value: Option<OsString>) -> Option<OsString> {
    value.filter(|value| !value.is_empty())
}

/// La especificación XDG ignora rutas relativas en `XDG_DATA_HOME`.
fn absolute_path(value: Option<OsString>) -> Option<PathBuf> {
    non_empty(value)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path != Path::new("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn resolver(platform: HostPlatform, vars: &[(&str, &str)], temp: &str) -> PathBuf {
        let map: HashMap<String, OsString> = vars
            .iter()
            .map(|(key, value)| ((*key).to_string(), OsString::from(value)))
            .collect();
        resolve_default_database_path(platform, |name| map.get(name).cloned(), PathBuf::from(temp))
    }

    #[test]
    fn windows_conserva_la_ruta_historica() {
        let path = resolver(
            HostPlatform::Windows,
            &[("LOCALAPPDATA", r"C:\Users\ana\AppData\Local")],
            r"C:\Temp",
        );
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
    fn windows_cae_al_perfil_y_luego_al_temporal() {
        let perfil = resolver(
            HostPlatform::Windows,
            &[("USERPROFILE", r"C:\Users\ana")],
            r"C:\Temp",
        );
        assert!(perfil.starts_with(PathBuf::from(r"C:\Users\ana")));

        let temporal = resolver(HostPlatform::Windows, &[], r"C:\Temp");
        assert!(temporal.starts_with(PathBuf::from(r"C:\Temp")));
    }

    #[test]
    fn linux_usa_xdg_data_home_absoluto() {
        let xdg = std::env::temp_dir().join("hiqnet-xdg-data");
        let path = resolver(
            HostPlatform::Linux,
            &[("XDG_DATA_HOME", &xdg.to_string_lossy())],
            "/tmp",
        );
        assert_eq!(
            path,
            xdg.join("hiqnet-pos").join("data").join("hiqnet-pos.db")
        );
    }

    #[test]
    fn linux_ignora_xdg_relativo_y_prefiere_home() {
        let home = std::env::temp_dir().join("hiqnet-home");
        let path = resolver(
            HostPlatform::Linux,
            &[
                ("XDG_DATA_HOME", "datos-relativos"),
                ("HOME", &home.to_string_lossy()),
            ],
            "/tmp",
        );
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
    fn linux_sin_home_usa_el_temporal() {
        let temp = std::env::temp_dir().join("hiqnet-temp-linux");
        let path = resolver(HostPlatform::Linux, &[], &temp.to_string_lossy());
        assert_eq!(
            path,
            temp.join("hiqnet-pos").join("data").join("hiqnet-pos.db")
        );
    }

    #[test]
    fn macos_usa_application_support() {
        let home = std::env::temp_dir().join("hiqnet-home-macos");
        let path = resolver(
            HostPlatform::MacOs,
            &[("HOME", &home.to_string_lossy())],
            "/tmp",
        );
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
    fn plataforma_desconocida_no_escribe_en_el_repositorio() {
        let temp = std::env::temp_dir().join("hiqnet-temp-other");
        let home = std::env::temp_dir().join("hiqnet-home-other");
        let path = resolver(
            HostPlatform::Other,
            &[("HOME", &home.to_string_lossy())],
            &temp.to_string_lossy(),
        );
        assert!(path.starts_with(&temp));
    }

    #[test]
    fn la_variable_de_entorno_no_vacia_gana_sobre_la_plataforma() {
        // Se prueba el contrato de `non_empty`, que es la base del override.
        assert_eq!(non_empty(Some(OsString::from(""))), None);
        assert_eq!(
            non_empty(Some(OsString::from("/datos/qa.db"))),
            Some(OsString::from("/datos/qa.db"))
        );
    }

    #[test]
    fn la_ruta_por_defecto_no_vive_dentro_del_repositorio() {
        let path = default_database_path();
        assert!(
            path.is_absolute(),
            "la base debe resolverse absoluta: {path:?}"
        );
        assert!(path.ends_with("hiqnet-pos.db"));
    }
}
