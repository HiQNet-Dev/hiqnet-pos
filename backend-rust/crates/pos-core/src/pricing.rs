//! Política única de impuesto y precio al público.
//!
//! Los precios del catálogo y de las variantes son **netos**. El impuesto se
//! calcula por unidad con redondeo half-up a centavos y el precio al público es
//! neto + impuesto (ADR-003 y ADR-006). Ninguna otra capa reproduce esta
//! aritmética: la interfaz solo presenta lo que devuelve este módulo.

use crate::error::{DomainError, DomainResult};
use crate::money::Money;

/// Tasa máxima admitida: 100 % en puntos base.
pub const MAX_TAX_RATE_BP: u16 = 10_000;

/// Impuesto de una unidad neta a la tasa indicada, redondeado half-up.
pub fn unit_tax(price: Money, tax_rate_bp: u16) -> DomainResult<Money> {
    if tax_rate_bp > MAX_TAX_RATE_BP {
        return Err(DomainError::InvalidTaxRate);
    }
    let base = i128::from(price.minor_units()) * i128::from(tax_rate_bp);
    let tax = div_round_half_up(base, i128::from(MAX_TAX_RATE_BP))?;
    Money::from_minor_units(i64::try_from(tax).map_err(|_| DomainError::AmountOverflow)?)
}

/// Precio final al público de una unidad: neto + impuesto de esa misma unidad.
///
/// El redondeo se aplica sobre el impuesto, no sobre el total: 100.00 netos al
/// 16 % dan 116.00, y 0.05 netos al 16 % dan 0.06 (impuesto 0.01).
pub fn public_unit_total(price: Money, tax_rate_bp: u16) -> DomainResult<Money> {
    price.checked_add(unit_tax(price, tax_rate_bp)?)
}

/// Redondeo half-up de una división entera con signo.
pub(crate) fn div_round_half_up(numerator: i128, denominator: i128) -> DomainResult<i128> {
    if denominator == 0 {
        return Err(DomainError::AmountOverflow);
    }
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder.abs() * 2 >= denominator {
        Ok(quotient + numerator.signum())
    } else {
        Ok(quotient)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn money(minor: i64) -> Money {
        Money::from_minor_units(minor).unwrap()
    }

    #[test]
    fn impuesto_de_una_unidad_al_16_por_ciento() {
        assert_eq!(unit_tax(money(10_000), 1_600).unwrap().minor_units(), 1_600);
    }

    #[test]
    fn precio_al_publico_suma_el_impuesto_una_sola_vez() {
        assert_eq!(
            public_unit_total(money(10_000), 1_600)
                .unwrap()
                .minor_units(),
            11_600
        );
        assert_eq!(
            public_unit_total(money(12_500), 1_600)
                .unwrap()
                .minor_units(),
            14_500
        );
    }

    #[test]
    fn tasa_cero_conserva_el_precio_neto() {
        assert_eq!(unit_tax(money(1_234), 0).unwrap().minor_units(), 0);
        assert_eq!(
            public_unit_total(money(1_234), 0).unwrap().minor_units(),
            1_234
        );
    }

    #[test]
    fn redondea_half_up_las_fracciones_de_centavo() {
        // 0.05 * 16 % = 0.008 → 0.01
        assert_eq!(unit_tax(money(5), 1_600).unwrap().minor_units(), 1);
        // 0.03 * 16 % = 0.0048 → 0.00
        assert_eq!(unit_tax(money(3), 1_600).unwrap().minor_units(), 0);
        // 0.15 * 16 % = 0.024 → 0.02
        assert_eq!(unit_tax(money(15), 1_600).unwrap().minor_units(), 2);
        // justo la mitad hacia arriba: 0.25 * 16 % = 0.04 exacto
        assert_eq!(unit_tax(money(25), 1_600).unwrap().minor_units(), 4);
    }

    #[test]
    fn un_centavo_al_diez_por_ciento_redondea_hacia_arriba() {
        // 0.01 * 10 % = 0.001 → 0.00 por half-up (0.001 < 0.005)
        assert_eq!(unit_tax(money(1), 1_000).unwrap().minor_units(), 0);
        // 0.09 * 10 % = 0.009 → 0.01
        assert_eq!(unit_tax(money(9), 1_000).unwrap().minor_units(), 1);
    }

    #[test]
    fn rechaza_tasas_mayores_al_cien_por_ciento() {
        assert!(unit_tax(money(100), 10_001).is_err());
        assert!(public_unit_total(money(100), 20_000).is_err());
    }

    #[test]
    fn el_precio_al_publico_nunca_es_menor_que_el_neto() {
        for minor in [1_i64, 7, 99, 100, 11_599, 999_999] {
            for rate in [0_u16, 800, 1_600, 2_100] {
                let net = money(minor);
                let public = public_unit_total(net, rate).unwrap();
                assert!(public >= net);
            }
        }
    }
}
