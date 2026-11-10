//! DTO del servicio, agrupados por dominio.

use super::*;

mod users;
pub use users::*;
mod purchasing;
pub use purchasing::*;
mod catalog;
pub use catalog::*;
mod cash;
pub use cash::*;
mod inventory;
pub use inventory::*;
mod sales;
pub use sales::*;
mod expenses;
pub use expenses::*;
mod returns;
pub use returns::*;
mod reporting;
pub use reporting::*;
mod audit;
pub use audit::*;
