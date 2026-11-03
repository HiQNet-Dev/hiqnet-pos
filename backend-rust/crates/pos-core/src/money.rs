use crate::error::{DomainError, DomainResult};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::quantity::Quantity;

/// Importe monetario exacto almacenado en unidades menores (centavos).
///
/// El MVP opera en una sola moneda (MXN); la moneda no forma parte del value
/// object todavía. Ver `docs/01-architecture/decisions/ADR-003-money.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub fn from_minor_units(minor_units: i64) -> DomainResult<Self> {
        if minor_units < 0 {
            return Err(DomainError::NegativeAmount);
        }
        Ok(Self(minor_units))
    }

    pub fn from_decimal(value: Decimal) -> DomainResult<Self> {
        if value.is_sign_negative() {
            return Err(DomainError::NegativeAmount);
        }
        if value.scale() > 2 {
            let rounded = value.round_dp(2);
            if rounded != value {
                return Err(DomainError::InvalidMoneyScale);
            }
            return Self::from_decimal(rounded);
        }
        let minor = (value * Decimal::from(100))
            .to_i64()
            .ok_or(DomainError::AmountOverflow)?;
        Self::from_minor_units(minor)
    }

    pub fn minor_units(self) -> i64 {
        self.0
    }

    pub fn as_decimal(self) -> Decimal {
        Decimal::new(self.0, 2)
    }

    pub fn checked_add(self, other: Money) -> DomainResult<Money> {
        self.0
            .checked_add(other.0)
            .map(Money)
            .ok_or(DomainError::AmountOverflow)
    }

    pub fn checked_sub(self, other: Money) -> DomainResult<Money> {
        self.0
            .checked_sub(other.0)
            .map(Money)
            .ok_or(DomainError::NegativeAmount)
    }

    /// Multiplica por una cantidad exacta y redondea a centavos (half-up).
    pub fn multiply_by(self, quantity: Quantity) -> DomainResult<Money> {
        let milli = i128::from(self.0) * i128::from(quantity.milli_units());
        let rounded = round_half_up_div(milli, 1_000)?;
        let minor = i64::try_from(rounded).map_err(|_| DomainError::AmountOverflow)?;
        Money::from_minor_units(minor)
    }
}

fn round_half_up_div(numerator: i128, denominator: i128) -> DomainResult<i128> {
    debug_assert!(denominator > 0);
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder.abs() * 2 >= denominator {
        Ok(quotient + numerator.signum())
    } else {
        Ok(quotient)
    }
}

impl FromStr for Money {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let decimal =
            Decimal::from_str(value.trim()).map_err(|_| DomainError::InvalidMoneyScale)?;
        Money::from_decimal(decimal)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.as_decimal())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_acepta_negativos() {
        assert_eq!(
            Money::from_minor_units(-1),
            Err(DomainError::NegativeAmount)
        );
    }

    #[test]
    fn rechaza_mas_de_dos_decimales_no_exactos() {
        assert_eq!(
            Money::from_decimal(Decimal::new(105, 3)),
            Err(DomainError::InvalidMoneyScale)
        );
    }

    #[test]
    fn suma_y_resta_exactas() {
        let a = Money::from_minor_units(1999).unwrap();
        let b = Money::from_minor_units(1).unwrap();
        assert_eq!(a.checked_add(b).unwrap().minor_units(), 2000);
        assert_eq!(a.checked_sub(b).unwrap().minor_units(), 1998);
    }

    #[test]
    fn multiplica_por_cantidad_con_redondeo_half_up() {
        let price = Money::from_minor_units(1050).unwrap(); // 10.50
        let quantity = Quantity::from_milli_units(2500).unwrap(); // 2.5
        assert_eq!(price.multiply_by(quantity).unwrap().minor_units(), 2625);

        let cent = Money::from_minor_units(1).unwrap();
        let half = Quantity::from_milli_units(500).unwrap();
        assert_eq!(cent.multiply_by(half).unwrap().minor_units(), 1);
    }

    #[test]
    fn parsea_y_formatea() {
        assert_eq!("1234.56".parse::<Money>().unwrap().minor_units(), 123456);
        assert_eq!(
            Money::from_minor_units(123456).unwrap().to_string(),
            "1234.56"
        );
    }
}
