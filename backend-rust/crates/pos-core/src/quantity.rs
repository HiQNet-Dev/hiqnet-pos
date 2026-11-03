use crate::error::{DomainError, DomainResult};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Cantidad exacta con hasta tres decimales (milésimas).
///
/// Se usa tanto para unidades vendidas como para existencias. No admite
/// valores negativos: los ajustes negativos se modelan como movimientos de
/// ledger con signo explícito, no como una cantidad negativa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Quantity(i64);

impl Quantity {
    pub const ZERO: Quantity = Quantity(0);

    pub fn from_milli_units(milli_units: i64) -> DomainResult<Self> {
        if milli_units < 0 {
            return Err(DomainError::NegativeQuantity);
        }
        Ok(Self(milli_units))
    }

    pub fn from_decimal(value: Decimal) -> DomainResult<Self> {
        if value.is_sign_negative() {
            return Err(DomainError::NegativeQuantity);
        }
        if value.scale() > 3 {
            let rounded = value.round_dp(3);
            if rounded != value {
                return Err(DomainError::InvalidQuantityScale);
            }
            return Self::from_decimal(rounded);
        }
        let milli = (value * Decimal::from(1000))
            .to_i64()
            .ok_or(DomainError::QuantityOverflow)?;
        Self::from_milli_units(milli)
    }

    pub fn milli_units(self) -> i64 {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn as_decimal(self) -> Decimal {
        Decimal::new(self.0, 3).normalize()
    }

    pub fn checked_add(self, other: Quantity) -> DomainResult<Quantity> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(DomainError::QuantityOverflow)
    }
}

impl FromStr for Quantity {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let decimal =
            Decimal::from_str(value.trim()).map_err(|_| DomainError::InvalidQuantityScale)?;
        Quantity::from_decimal(decimal)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_decimal())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rechaza_negativas_y_escala_excesiva() {
        assert_eq!(
            Quantity::from_milli_units(-1),
            Err(DomainError::NegativeQuantity)
        );
        assert_eq!(
            Quantity::from_decimal(Decimal::new(1234, 4)),
            Err(DomainError::InvalidQuantityScale)
        );
    }

    #[test]
    fn parsea_y_formatea_sin_ceros_innecesarios() {
        assert_eq!("2.500".parse::<Quantity>().unwrap().milli_units(), 2500);
        assert_eq!("2.5".parse::<Quantity>().unwrap().to_string(), "2.5");
        assert_eq!("3".parse::<Quantity>().unwrap().to_string(), "3");
    }

    #[test]
    fn suma_con_control_de_overflow() {
        let a = Quantity::from_milli_units(i64::MAX).unwrap();
        let b = Quantity::from_milli_units(1).unwrap();
        assert_eq!(a.checked_add(b), Err(DomainError::QuantityOverflow));
    }
}
