use super::*;

#[test]
fn politica_de_contrasena_exige_longitud_minima() {
    assert!(validate_password("123456789").is_err());
    assert!(validate_password("1234567890").is_ok());
}

#[test]
fn vista_comercial_marca_solo_lectura_al_vencer() {
    let start = OffsetDateTime::now_utc();
    let commercial =
        CommercialState::new_trial(InstallationId::new(), OrganizationId::new(), start);
    let state = installation_from_commercial(&commercial);

    // Sin licencia firmada se conserva el trial local.
    let (fresh, resolution) = resolve_commercial(&state, None);
    assert!(matches!(resolution, LicenseResolution::NoGrant));
    assert_eq!(fresh.effective_status(start), LicenseStatus::Trial);

    let expirado = fresh.effective_status(start + time::Duration::days(31));
    assert_eq!(expirado, LicenseStatus::ReadOnly);
    assert!(!fresh.allows(FeatureKey::Sales, start + time::Duration::days(31)));
}

#[test]
fn las_fechas_del_contrato_ipc_son_texto_rfc3339() {
    let movement = CashMovementView {
        id: "m-1".into(),
        kind: "in".into(),
        amount: "1.00".into(),
        reason: "prueba".into(),
        occurred_at: OffsetDateTime::now_utc(),
    };
    let value = serde_json::to_value(movement).unwrap();
    assert!(
        value["occurred_at"].is_string(),
        "el contrato IPC debe serializar fechas como texto: {value}"
    );
}
