// La serialización es intencional: el inyector de fallos es global y las
// pruebas de este módulo comparten el mismo proceso.
#![allow(clippy::await_holding_lock)]

use super::*;
use crate::open;
use std::sync::Mutex;

/// Las pruebas comparten el inyector global: se serializan entre sí.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|poison| poison.into_inner())
}

async fn pending_count(path: &Path) -> i64 {
    let pool = open(path).await.unwrap();
    let value: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;
    value
}

async fn scenario() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("hiqnet-pos.db");
    let backup = dir.path().join("respaldo.db");

    let pool = open(&database).await.unwrap();
    sqlx::query("INSERT INTO organization (id, name, created_at) VALUES ('org-1', 'Org', '2026-01-01T00:00:00Z')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO local_installation_state \
         (id, installation_id, organization_id, plan, license_status, features_json, \
          trial_started_at, valid_until, license_grant_json, license_signature_b64, updated_at) \
         VALUES (1, 'install-1', 'org-1', 'trial', 'trial', '[\"catalog\"]', \
                 '2026-01-01T00:00:00Z', '2026-12-01T00:00:00Z', NULL, NULL, '2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    for id in ["p-1", "p-2"] {
        sqlx::query(
            "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) \
             VALUES (?, 'org-1', ?, ?, 100, 0, 1, '2026-01-01T00:00:00Z')",
        )
        .bind(id)
        .bind(id)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    }
    backup_to(&pool, &backup).await.unwrap();

    // Cambio posterior al respaldo: la restauración debe deshacerlo.
    sqlx::query(
        "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at) \
         VALUES ('p-3', 'org-1', 'P-3', 'P-3', 100, 0, 1, '2026-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    (dir, database, backup)
}

fn staged(database: &Path) -> PathBuf {
    staged_path(database)
}

#[tokio::test]
async fn un_fallo_de_promocion_tras_confirmar_conserva_el_marcador_y_permite_reintentar() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    schedule_pending_restore(&database, &backup).await.unwrap();
    fault_injector::inject(&[RestoreStep::FinalVerification]);
    let error = consume_pending_restore(&database).await.unwrap_err();
    assert!(
        matches!(error, StorageError::InvalidBackup | StorageError::Restore),
        "obtenido {error:?}"
    );
    assert!(
        pending_restore_path(&database).is_file(),
        "el marcador debe sobrevivir para permitir el reintento"
    );
    assert_eq!(
        pending_count(&database).await,
        3,
        "la base previa se conserva"
    );

    // Reintento sin fallo: la restauración completa y libera el marcador.
    assert_eq!(
        consume_pending_restore(&database).await.unwrap(),
        PendingRestoreOutcome::Restored
    );
    assert_eq!(pending_count(&database).await, 2);
    assert!(!pending_restore_path(&database).is_file());
    assert!(
        !backup.exists(),
        "la copia importada se elimina tras restaurar"
    );
}

#[tokio::test]
async fn fallo_al_crear_la_copia_de_recuperacion_no_toca_la_base() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    // Fallo simulado: el snapshot no queda disponible para verificarse.
    fault_injector::inject(&[RestoreStep::RecoveryCopy]);
    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    assert!(matches!(error, StorageError::Restore), "obtenido {error:?}");
    assert_eq!(pending_count(&database).await, 3);
}

#[tokio::test]
async fn fallo_al_copiar_el_temporal_deja_la_base_intacta() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    // El temporal existe como directorio: la copia no puede completarse.
    std::fs::create_dir_all(staged(&database)).unwrap();
    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    assert!(matches!(error, StorageError::Restore), "obtenido {error:?}");
    assert_eq!(pending_count(&database).await, 3);
}

#[tokio::test]
async fn fallo_durante_la_copia_del_temporal_limpia_el_archivo() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    fault_injector::inject(&[RestoreStep::StagedCopy]);
    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    assert!(matches!(error, StorageError::Restore), "obtenido {error:?}");
    assert_eq!(pending_count(&database).await, 3);
    assert!(!staged(&database).exists(), "sin archivos temporales");
}

#[tokio::test]
async fn fallo_al_validar_el_temporal_deja_la_base_intacta() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    fault_injector::inject(&[RestoreStep::StagedValidation]);
    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    assert!(
        matches!(error, StorageError::InvalidBackup | StorageError::Restore),
        "obtenido {error:?}"
    );
    assert_eq!(pending_count(&database).await, 3);
    assert!(!staged(&database).exists(), "el temporal se elimina");
}

#[tokio::test]
async fn fallo_al_promover_deja_la_base_intacta() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    fault_injector::inject(&[RestoreStep::Promotion]);
    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    assert!(matches!(error, StorageError::Restore), "obtenido {error:?}");
    assert_eq!(pending_count(&database).await, 3);
    assert!(!staged(&database).exists(), "el temporal se elimina");
}

#[tokio::test]
async fn fallo_en_la_verificacion_final_revierte_a_la_copia_previa() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    fault_injector::inject(&[RestoreStep::FinalVerification]);
    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    assert!(
        matches!(error, StorageError::InvalidBackup | StorageError::Restore),
        "obtenido {error:?}"
    );
    assert_eq!(
        pending_count(&database).await,
        3,
        "la reversión devuelve el estado previo completo"
    );
}

#[tokio::test]
async fn fallo_de_reversion_conserva_artefactos_y_avisa() {
    let _serial = serial();
    let (dir, database, backup) = scenario().await;

    fault_injector::inject(&[RestoreStep::FinalVerification, RestoreStep::Rollback]);
    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    let StorageError::RestoreCritical {
        database: base,
        recovery,
        staged,
    } = error
    else {
        panic!("se esperaba RestoreCritical, obtenido {error:?}");
    };

    // Todo lo necesario para recuperar a mano sigue presente.
    assert!(Path::new(&base).is_file(), "la base se conserva");
    assert!(
        Path::new(&recovery).is_file(),
        "la copia de recuperación se conserva"
    );
    assert!(
        Path::new(&staged).is_file(),
        "el temporal validado se conserva"
    );
    assert_eq!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                // Los sidecars apartados (`.wal.pre-restore-`, `.shm.pre-restore-`)
                // no son copias de recuperación.
                name.contains(".pre-restore-")
                    && !name.contains(".wal.pre-restore-")
                    && !name.contains(".shm.pre-restore-")
            })
            .count(),
        1,
        "una sola copia de recuperación"
    );
    // La copia de recuperación sí es utilizable.
    verify_snapshot(Path::new(&recovery)).await.unwrap();
}

#[tokio::test]
async fn restauracion_correcta_deja_el_estado_respaldado() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    let report = restore_from_backup(&database, &backup).await.unwrap();
    assert!(
        report
            .recovery_copy
            .as_ref()
            .is_some_and(|path| path.exists())
    );
    assert_eq!(pending_count(&database).await, 2);
}

#[tokio::test]
async fn restaura_aunque_un_lector_previo_deje_sidecars() {
    let _serial = serial();
    let (dir, database, backup) = scenario().await;

    // Un lector de solo lectura puede dejar `-wal`/`-shm` sin mantener la
    // base en uso: su presencia no debe interpretarse como conexión activa.
    let mut reader = SqliteConnectOptions::new()
        .filename(&database)
        .read_only(true)
        .connect()
        .await
        .unwrap();
    sqlx::query("SELECT COUNT(*) FROM product")
        .fetch_one(&mut reader)
        .await
        .unwrap();
    reader.close().await.unwrap();

    restore_from_backup(&database, &backup).await.unwrap();
    assert_eq!(pending_count(&database).await, 2);

    // Los residuos se conservan apartados, no se destruyen.
    let quarantined: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.contains(".wal.pre-restore-") || name.contains(".shm.pre-restore-"))
        .collect();
    for name in quarantined {
        let path = dir.path().join(&name);
        if name.contains(".wal.pre-restore-") {
            assert_eq!(
                std::fs::metadata(&path).unwrap().len(),
                0,
                "solo se aparta un WAL ya vaciado"
            );
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn otra_conexion_no_puede_escribir_entre_checkpoint_y_promocion() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    let (arrived_tx, arrived_rx) = std::sync::mpsc::channel();
    let (go_tx, go_rx) = std::sync::mpsc::channel();
    race::arm(
        race::Point::BeforePromotion,
        Box::new(move || {
            let _ = arrived_tx.send(());
            let _ = go_rx.recv_timeout(Duration::from_secs(10));
        }),
    );

    let writer_database = database.clone();
    let writer = std::thread::spawn(move || {
        // Espera a que la utilidad esté a punto de promover.
        let _ = arrived_rx.recv_timeout(Duration::from_secs(10));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let outcome = runtime.block_on(async move {
            let mut connection = SqliteConnectOptions::new()
                .filename(&writer_database)
                .busy_timeout(Duration::from_millis(500))
                .connect()
                .await
                .map_err(|_| "conexion rechazada")?;
            let result = sqlx::query(
                "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at)                      VALUES ('intruso', 'org-1', 'INT', 'Intruso', 100, 0, 1, '2026-01-01T00:00:00Z')",
            )
            .execute(&mut connection)
            .await
            .map_err(|_| "escritura rechazada");
            let _ = connection.close().await;
            result.map(|_| ())
        });
        let _ = go_tx.send(());
        outcome
    });

    let report = restore_from_backup(&database, &backup).await;
    let intento = writer.join().expect("hilo del intruso");

    // La conexión externa no puede escribir mientras se sostiene el bloqueo
    // exclusivo durante la copia y la validación.
    assert!(
        intento.is_err(),
        "una conexión externa escribió entre el checkpoint y la promoción"
    );
    match report {
        Ok(_) => {}
        Err(error) => assert!(
            matches!(error, StorageError::DatabaseInUse | StorageError::Restore),
            "error inesperado: {error:?}"
        ),
    }

    verify_backup(&database).await.unwrap();
    assert_eq!(
        pending_count(&database).await,
        2,
        "estado del respaldo íntegro"
    );
    assert!(
        !wal_has_data(&database),
        "quedó un WAL con datos junto a la base promovida"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn escritura_externa_en_la_ventana_aborta_sin_perder_datos() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    // En la ventana la guardia ya se soltó: una conexión externa sí puede
    // entrar. La promoción debe abortar y sus datos confirmados sobrevivir.
    let (written_tx, written_rx) = std::sync::mpsc::channel();
    race::arm(
        race::Point::BeforePromotionUnlocked,
        Box::new(move || {
            let _ = written_rx.recv_timeout(Duration::from_secs(10));
        }),
    );

    let writer_database = database.clone();
    let (hold_tx, hold_rx) = std::sync::mpsc::channel::<()>();
    let writer = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let mut connection = SqliteConnectOptions::new()
                .filename(&writer_database)
                .busy_timeout(Duration::from_secs(2))
                .connect()
                .await
                .expect("conexión externa");
            sqlx::query(
                "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, created_at)                      VALUES ('intruso', 'org-1', 'INT', 'Intruso', 100, 0, 1, '2026-01-01T00:00:00Z')",
            )
            .execute(&mut connection)
            .await
            .expect("escritura externa confirmada");
            let _ = written_tx.send(());
            // Mantiene el WAL presente mientras la utilidad intenta promover.
            let _ = hold_rx.recv_timeout(Duration::from_secs(10));
            let _ = connection.close().await;
        });
    });

    let report = restore_from_backup(&database, &backup).await;
    let _ = hold_tx.send(());
    writer.join().expect("hilo del escritor externo");

    // El WAL con datos externos impide la promoción: la base original queda
    // intacta y la escritura confirmada no se pierde.
    assert!(
        matches!(report, Err(StorageError::DatabaseInUse)),
        "se esperaba rechazo por base en uso, obtenido {report:?}"
    );
    verify_backup(&database).await.unwrap();
    assert_eq!(
        pending_count(&database).await,
        4,
        "la escritura externa confirmada se conserva"
    );
    assert!(!wal_has_data(&database), "el WAL se integró al cerrar");
}

#[tokio::test]
async fn rechaza_restaurar_con_lector_externo_activo() {
    let _serial = serial();
    let (_dir, database, backup) = scenario().await;

    // Conexión externa con lectura abierta: en WAL no bloquea escritores,
    // por lo que solo el checkpoint TRUNCATE la detecta.
    let reader = SqliteConnectOptions::new()
        .filename(&database)
        .connect()
        .await
        .unwrap();
    let mut reader = reader;
    sqlx::query("BEGIN").execute(&mut reader).await.unwrap();
    sqlx::query("SELECT COUNT(*) FROM product")
        .fetch_one(&mut reader)
        .await
        .unwrap();

    let error = restore_from_backup(&database, &backup).await.unwrap_err();
    assert!(
        matches!(error, StorageError::DatabaseInUse),
        "obtenido {error:?}"
    );
    assert_eq!(pending_count(&database).await, 3, "la base no se modifica");

    sqlx::query("COMMIT").execute(&mut reader).await.unwrap();
    reader.close().await.unwrap();
    restore_from_backup(&database, &backup).await.unwrap();
    assert_eq!(pending_count(&database).await, 2);
}
