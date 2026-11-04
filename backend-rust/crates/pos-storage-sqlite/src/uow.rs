use async_trait::async_trait;
use pos_application::{AppError, AppResult, InstallationState, Store, StoreTx};
use pos_core::audit::{AuditAction, AuditEvent};
use pos_core::cash::{CashMovement, CashRegister, CashShift, CashShiftStatus};
use pos_core::catalog::{Category, Product, ProductVariant};
use pos_core::identity::{Role, User};
use pos_core::ids::{AuditEventId, BranchId, CategoryId, OrganizationId, ProductId, UserId};
use pos_core::inventory::InventoryMovement;
use pos_core::inventory::MovementKind;
use pos_core::modifiers::{ModifierGroup, ModifierOption};
use pos_core::money::Money;
use pos_core::organization::{Branch, Organization};
use pos_core::quantity::Quantity;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::str::FromStr;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Implementación SQLite del puerto `Store`.
#[derive(Debug, Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl Store for SqliteStore {
    async fn begin(&self) -> AppResult<Box<dyn StoreTx>> {
        let tx = self.pool.begin().await.map_err(map_sqlx)?;
        Ok(Box::new(SqliteTx { tx: Some(tx) }))
    }
}

struct SqliteTx {
    tx: Option<Transaction<'static, Sqlite>>,
}

impl SqliteTx {
    fn tx(&mut self) -> AppResult<&mut Transaction<'static, Sqlite>> {
        self.tx.as_mut().ok_or(AppError::Storage)
    }
}

#[async_trait]
impl StoreTx for SqliteTx {
    async fn installation_state(&mut self) -> AppResult<Option<InstallationState>> {
        let tx = self.tx()?;
        let row = sqlx::query(
            "SELECT installation_id, organization_id, plan, license_status, features_json, \
             trial_started_at, valid_until, license_grant_json, license_signature_b64 \
             FROM local_installation_state WHERE id = 1",
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_sqlx)?;

        row.map(|row| -> AppResult<InstallationState> {
            let features: Vec<String> =
                serde_json::from_str(&row.get::<String, _>("features_json"))
                    .map_err(|_| AppError::Storage)?;
            Ok(InstallationState {
                installation_id: parse_id(row.get("installation_id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                trial_started_at: parse_ts(row.get("trial_started_at"))?,
                plan: row.get("plan"),
                license_status: row.get("license_status"),
                features,
                valid_until: parse_ts(row.get("valid_until"))?,
                license_grant_json: row.get("license_grant_json"),
                license_signature_b64: row.get("license_signature_b64"),
            })
        })
        .transpose()
    }

    async fn save_installation_state(&mut self, state: &InstallationState) -> AppResult<()> {
        let features = serde_json::to_string(&state.features).map_err(|_| AppError::Storage)?;
        sqlx::query(
            "INSERT INTO local_installation_state \
             (id, installation_id, organization_id, plan, license_status, features_json, \
              trial_started_at, valid_until, license_grant_json, license_signature_b64, updated_at) \
             VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (id) DO UPDATE SET \
              installation_id = excluded.installation_id, \
              organization_id = excluded.organization_id, \
              plan = excluded.plan, \
              license_status = excluded.license_status, \
              features_json = excluded.features_json, \
              trial_started_at = excluded.trial_started_at, \
              valid_until = excluded.valid_until, \
              license_grant_json = excluded.license_grant_json, \
              license_signature_b64 = excluded.license_signature_b64, \
              updated_at = excluded.updated_at",
        )
        .bind(state.installation_id.to_string())
        .bind(state.organization_id.to_string())
        .bind(&state.plan)
        .bind(&state.license_status)
        .bind(features)
        .bind(format_ts(state.trial_started_at))
        .bind(format_ts(state.valid_until))
        .bind(&state.license_grant_json)
        .bind(&state.license_signature_b64)
        .bind(format_ts(OffsetDateTime::now_utc()))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn insert_organization(&mut self, organization: &Organization) -> AppResult<()> {
        sqlx::query("INSERT INTO organization (id, name, created_at) VALUES (?, ?, ?)")
            .bind(organization.id.to_string())
            .bind(&organization.name)
            .bind(format_ts(organization.created_at))
            .execute(&mut **self.tx()?)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn insert_branch(&mut self, branch: &Branch) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO branch (id, organization_id, name, code, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(branch.id.to_string())
        .bind(branch.organization_id.to_string())
        .bind(&branch.name)
        .bind(&branch.code)
        .bind(format_ts(branch.created_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn insert_user(&mut self, user: &User) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO app_user \
             (id, organization_id, branch_id, username, display_name, role, password_hash, active, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user.id.to_string())
        .bind(user.organization_id.to_string())
        .bind(user.branch_id.map(|id| id.to_string()))
        .bind(&user.username)
        .bind(&user.display_name)
        .bind(user.role.to_string())
        .bind(&user.password_hash)
        .bind(i64::from(user.active))
        .bind(format_ts(user.created_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn find_user_by_username(&mut self, username: &str) -> AppResult<Option<User>> {
        let row = sqlx::query(
            "SELECT id, organization_id, branch_id, username, display_name, role, password_hash, \
             active, created_at FROM app_user WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;

        row.map(|row| -> AppResult<User> {
            let role = match row.get::<String, _>("role").as_str() {
                "owner" => Role::Owner,
                "cashier" => Role::Cashier,
                _ => return Err(AppError::Storage),
            };
            Ok(User {
                id: parse_id(row.get("id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                branch_id: parse_optional_id(row.get("branch_id"))?,
                username: row.get("username"),
                display_name: row.get("display_name"),
                role,
                password_hash: row.get("password_hash"),
                active: row.get::<i64, _>("active") != 0,
                created_at: parse_ts(row.get("created_at"))?,
            })
        })
        .transpose()
    }

    async fn count_users(&mut self, organization_id: OrganizationId) -> AppResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS total FROM app_user WHERE organization_id = ?")
            .bind(organization_id.to_string())
            .fetch_one(&mut **self.tx()?)
            .await
            .map_err(map_sqlx)?;
        Ok(row.get("total"))
    }

    async fn insert_product(&mut self, product: &Product) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO product \
             (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, category_id, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(product.id.to_string())
        .bind(product.organization_id.to_string())
        .bind(&product.sku)
        .bind(&product.name)
        .bind(product.unit_price.minor_units())
        .bind(i64::from(product.tax_rate_bp))
        .bind(i64::from(product.active))
        .bind(product.category_id.map(|id| id.to_string()))
        .bind(format_ts(product.created_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn list_products(&mut self, organization_id: OrganizationId) -> AppResult<Vec<Product>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, category_id, created_at \
             FROM product WHERE organization_id = ? ORDER BY name COLLATE NOCASE",
        )
        .bind(organization_id.to_string())
        .fetch_all(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;

        rows.into_iter()
            .map(|row| -> AppResult<Product> {
                Ok(Product {
                    id: parse_id(row.get("id"))?,
                    organization_id: parse_id(row.get("organization_id"))?,
                    sku: row.get("sku"),
                    name: row.get("name"),
                    unit_price: Money::from_minor_units(row.get("unit_price_minor"))
                        .map_err(AppError::Domain)?,
                    tax_rate_bp: u16::try_from(row.get::<i64, _>("tax_rate_bp"))
                        .map_err(|_| AppError::Storage)?,
                    active: row.get::<i64, _>("active") != 0,
                    category_id: parse_optional_id(row.get("category_id"))?,
                    created_at: parse_ts(row.get("created_at"))?,
                })
            })
            .collect()
    }

    async fn insert_category(&mut self, category: &Category) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO category (id, organization_id, name, sort_order, active, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(category.id.to_string())
        .bind(category.organization_id.to_string())
        .bind(&category.name)
        .bind(category.sort_order)
        .bind(i64::from(category.active))
        .bind(format_ts(category.created_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn find_category(
        &mut self,
        organization_id: OrganizationId,
        category_id: CategoryId,
    ) -> AppResult<Option<Category>> {
        let row = sqlx::query(
            "SELECT id, organization_id, name, sort_order, active, created_at \
             FROM category WHERE id = ? AND organization_id = ?",
        )
        .bind(category_id.to_string())
        .bind(organization_id.to_string())
        .fetch_optional(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;

        row.map(|row| -> AppResult<Category> {
            Ok(Category {
                id: parse_id(row.get("id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                name: row.get("name"),
                sort_order: row.get("sort_order"),
                active: row.get::<i64, _>("active") != 0,
                created_at: parse_ts(row.get("created_at"))?,
            })
        })
        .transpose()
    }

    async fn update_product_category(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
        category_id: Option<CategoryId>,
    ) -> AppResult<()> {
        let result =
            sqlx::query("UPDATE product SET category_id = ? WHERE id = ? AND organization_id = ?")
                .bind(category_id.map(|id| id.to_string()))
                .bind(product_id.to_string())
                .bind(organization_id.to_string())
                .execute(&mut **self.tx()?)
                .await
                .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    async fn list_categories(
        &mut self,
        organization_id: OrganizationId,
    ) -> AppResult<Vec<Category>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, name, sort_order, active, created_at \
             FROM category WHERE organization_id = ? ORDER BY sort_order, name COLLATE NOCASE",
        )
        .bind(organization_id.to_string())
        .fetch_all(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;

        rows.into_iter()
            .map(|row| -> AppResult<Category> {
                Ok(Category {
                    id: parse_id(row.get("id"))?,
                    organization_id: parse_id(row.get("organization_id"))?,
                    name: row.get("name"),
                    sort_order: row.get("sort_order"),
                    active: row.get::<i64, _>("active") != 0,
                    created_at: parse_ts(row.get("created_at"))?,
                })
            })
            .collect()
    }

    async fn insert_product_variant(&mut self, variant: &ProductVariant) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO product_variant \
             (id, organization_id, product_id, name, sku, unit_price_minor, active, sort_order, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(variant.id.to_string())
        .bind(variant.organization_id.to_string())
        .bind(variant.product_id.to_string())
        .bind(&variant.name)
        .bind(&variant.sku)
        .bind(variant.unit_price.minor_units())
        .bind(i64::from(variant.active))
        .bind(variant.sort_order)
        .bind(format_ts(variant.created_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn list_product_variants(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Vec<ProductVariant>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, product_id, name, sku, unit_price_minor, active, sort_order, created_at \
             FROM product_variant WHERE organization_id = ? AND product_id = ? \
             ORDER BY sort_order, name COLLATE NOCASE",
        )
        .bind(organization_id.to_string())
        .bind(product_id.to_string())
        .fetch_all(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;

        rows.into_iter()
            .map(|row| -> AppResult<ProductVariant> {
                Ok(ProductVariant {
                    id: parse_id(row.get("id"))?,
                    organization_id: parse_id(row.get("organization_id"))?,
                    product_id: parse_id(row.get("product_id"))?,
                    name: row.get("name"),
                    sku: row.get("sku"),
                    unit_price: Money::from_minor_units(row.get("unit_price_minor"))
                        .map_err(AppError::Domain)?,
                    active: row.get::<i64, _>("active") != 0,
                    sort_order: row.get("sort_order"),
                    created_at: parse_ts(row.get("created_at"))?,
                })
            })
            .collect()
    }

    async fn find_product(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Option<Product>> {
        let row = sqlx::query(
            "SELECT id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, category_id, created_at \
             FROM product WHERE id = ? AND organization_id = ?",
        )
        .bind(product_id.to_string())
        .bind(organization_id.to_string())
        .fetch_optional(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;

        row.map(|row| -> AppResult<Product> {
            Ok(Product {
                id: parse_id(row.get("id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                sku: row.get("sku"),
                name: row.get("name"),
                unit_price: Money::from_minor_units(row.get("unit_price_minor"))
                    .map_err(AppError::Domain)?,
                tax_rate_bp: u16::try_from(row.get::<i64, _>("tax_rate_bp"))
                    .map_err(|_| AppError::Storage)?,
                active: row.get::<i64, _>("active") != 0,
                category_id: parse_optional_id(row.get("category_id"))?,
                created_at: parse_ts(row.get("created_at"))?,
            })
        })
        .transpose()
    }

    async fn insert_inventory_movement(&mut self, movement: &InventoryMovement) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO inventory_movement \
             (id, organization_id, branch_id, product_id, kind, quantity_milli, reason, \
              actor_user_id, occurred_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(movement.id.to_string())
        .bind(movement.organization_id.to_string())
        .bind(movement.branch_id.to_string())
        .bind(movement.product_id.to_string())
        .bind(movement.kind.as_str())
        .bind(movement.quantity.milli_units())
        .bind(&movement.reason)
        .bind(movement.actor_user_id.map(|id| id.to_string()))
        .bind(format_ts(movement.occurred_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn current_stock(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        product_id: ProductId,
    ) -> AppResult<Quantity> {
        let row = sqlx::query(
            "SELECT COALESCE(SUM(CASE kind WHEN 'outbound' THEN -quantity_milli WHEN 'adjustment_out' THEN -quantity_milli ELSE quantity_milli END), 0) AS stock \
             FROM inventory_movement WHERE organization_id = ? AND branch_id = ? AND product_id = ?",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .bind(product_id.to_string())
        .fetch_one(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        let stock: i64 = row.get("stock");
        Quantity::from_milli_units(stock).map_err(AppError::Domain)
    }

    async fn list_inventory_movements(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
        product_id: Option<ProductId>,
        limit: i64,
    ) -> AppResult<Vec<InventoryMovement>> {
        let rows = if let Some(product_id) = product_id {
            sqlx::query(
                "SELECT id, organization_id, branch_id, product_id, kind, quantity_milli, reason, \
                 actor_user_id, occurred_at FROM inventory_movement \
                 WHERE organization_id = ? AND branch_id = ? AND product_id = ? \
                 ORDER BY occurred_at DESC LIMIT ?",
            )
            .bind(organization_id.to_string())
            .bind(branch_id.to_string())
            .bind(product_id.to_string())
            .bind(limit)
            .fetch_all(&mut **self.tx()?)
            .await
            .map_err(map_sqlx)?
        } else {
            sqlx::query(
                "SELECT id, organization_id, branch_id, product_id, kind, quantity_milli, reason, \
                 actor_user_id, occurred_at FROM inventory_movement \
                 WHERE organization_id = ? AND branch_id = ? \
                 ORDER BY occurred_at DESC LIMIT ?",
            )
            .bind(organization_id.to_string())
            .bind(branch_id.to_string())
            .bind(limit)
            .fetch_all(&mut **self.tx()?)
            .await
            .map_err(map_sqlx)?
        };

        rows.into_iter()
            .map(|row| -> AppResult<InventoryMovement> {
                let kind = MovementKind::from_key(&row.get::<String, _>("kind"))
                    .ok_or(AppError::Storage)?;
                Ok(InventoryMovement {
                    id: row
                        .get::<String, _>("id")
                        .parse()
                        .map_err(|_| AppError::Storage)?,
                    organization_id: parse_id(row.get("organization_id"))?,
                    branch_id: parse_id(row.get("branch_id"))?,
                    product_id: row
                        .get::<String, _>("product_id")
                        .parse()
                        .map_err(|_| AppError::Storage)?,
                    kind,
                    quantity: Quantity::from_milli_units(row.get("quantity_milli"))
                        .map_err(AppError::Domain)?,
                    reason: row.get("reason"),
                    actor_user_id: parse_optional_id(row.get("actor_user_id"))?,
                    occurred_at: parse_ts(row.get("occurred_at"))?,
                })
            })
            .collect()
    }

    async fn insert_modifier_group(&mut self, group: &ModifierGroup) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO modifier_group \
             (id, organization_id, product_id, name, min_selectable, max_selectable, sort_order, active, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(group.id.to_string())
        .bind(group.organization_id.to_string())
        .bind(group.product_id.to_string())
        .bind(&group.name)
        .bind(group.min as i64)
        .bind(group.max as i64)
        .bind(group.sort_order)
        .bind(i64::from(group.active))
        .bind(format_ts(group.created_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn find_modifier_group(
        &mut self,
        organization_id: OrganizationId,
        group_id: pos_core::ids::ModifierGroupId,
    ) -> AppResult<Option<ModifierGroup>> {
        let row = sqlx::query(
            "SELECT id, organization_id, product_id, name, min_selectable, max_selectable, sort_order, active, created_at \
             FROM modifier_group WHERE id = ? AND organization_id = ?",
        )
        .bind(group_id.to_string())
        .bind(organization_id.to_string())
        .fetch_optional(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        row.map(|r| -> AppResult<ModifierGroup> {
            Ok(ModifierGroup {
                id: parse_id(r.get("id"))?,
                organization_id: parse_id(r.get("organization_id"))?,
                product_id: parse_id(r.get("product_id"))?,
                name: r.get("name"),
                min: u32::try_from(r.get::<i64, _>("min_selectable"))
                    .map_err(|_| AppError::Storage)?,
                max: u32::try_from(r.get::<i64, _>("max_selectable"))
                    .map_err(|_| AppError::Storage)?,
                sort_order: r.get("sort_order"),
                active: r.get::<i64, _>("active") != 0,
                created_at: parse_ts(r.get("created_at"))?,
            })
        })
        .transpose()
    }

    async fn list_modifier_groups(
        &mut self,
        organization_id: OrganizationId,
        product_id: ProductId,
    ) -> AppResult<Vec<ModifierGroup>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, product_id, name, min_selectable, max_selectable, sort_order, active, created_at \
             FROM modifier_group WHERE organization_id = ? AND product_id = ? ORDER BY sort_order, name",
        )
        .bind(organization_id.to_string())
        .bind(product_id.to_string())
        .fetch_all(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(|r| -> AppResult<ModifierGroup> {
                Ok(ModifierGroup {
                    id: parse_id(r.get("id"))?,
                    organization_id: parse_id(r.get("organization_id"))?,
                    product_id: parse_id(r.get("product_id"))?,
                    name: r.get("name"),
                    min: u32::try_from(r.get::<i64, _>("min_selectable"))
                        .map_err(|_| AppError::Storage)?,
                    max: u32::try_from(r.get::<i64, _>("max_selectable"))
                        .map_err(|_| AppError::Storage)?,
                    sort_order: r.get("sort_order"),
                    active: r.get::<i64, _>("active") != 0,
                    created_at: parse_ts(r.get("created_at"))?,
                })
            })
            .collect()
    }

    async fn insert_modifier_option(&mut self, option: &ModifierOption) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO modifier_option \
             (id, organization_id, group_id, name, kind, price_delta_minor, sort_order, active, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(option.id.to_string())
        .bind(option.organization_id.to_string())
        .bind(option.group_id.to_string())
        .bind(&option.name)
        .bind(option.kind.as_str())
        .bind(option.price_delta.minor_units())
        .bind(option.sort_order)
        .bind(i64::from(option.active))
        .bind(format_ts(option.created_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn find_modifier_option(
        &mut self,
        organization_id: OrganizationId,
        option_id: pos_core::ids::ModifierOptionId,
    ) -> AppResult<Option<ModifierOption>> {
        let row = sqlx::query(
            "SELECT id, organization_id, group_id, name, kind, price_delta_minor, sort_order, active, created_at \
             FROM modifier_option WHERE id = ? AND organization_id = ?",
        )
        .bind(option_id.to_string())
        .bind(organization_id.to_string())
        .fetch_optional(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        row.map(|r| -> AppResult<ModifierOption> {
            Ok(ModifierOption {
                id: parse_id(r.get("id"))?,
                organization_id: parse_id(r.get("organization_id"))?,
                group_id: parse_id(r.get("group_id"))?,
                name: r.get("name"),
                kind: pos_core::modifiers::OptionKind::from_key(&r.get::<String, _>("kind"))
                    .ok_or(AppError::Storage)?,
                price_delta: Money::from_minor_units(r.get("price_delta_minor"))
                    .map_err(AppError::Domain)?,
                sort_order: r.get("sort_order"),
                active: r.get::<i64, _>("active") != 0,
                created_at: parse_ts(r.get("created_at"))?,
            })
        })
        .transpose()
    }

    async fn list_modifier_options(
        &mut self,
        organization_id: OrganizationId,
        group_id: pos_core::ids::ModifierGroupId,
    ) -> AppResult<Vec<ModifierOption>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, group_id, name, kind, price_delta_minor, sort_order, active, created_at \
             FROM modifier_option WHERE organization_id = ? AND group_id = ? ORDER BY sort_order, name",
        )
        .bind(organization_id.to_string())
        .bind(group_id.to_string())
        .fetch_all(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(|r| -> AppResult<ModifierOption> {
                Ok(ModifierOption {
                    id: parse_id(r.get("id"))?,
                    organization_id: parse_id(r.get("organization_id"))?,
                    group_id: parse_id(r.get("group_id"))?,
                    name: r.get("name"),
                    kind: pos_core::modifiers::OptionKind::from_key(&r.get::<String, _>("kind"))
                        .ok_or(AppError::Storage)?,
                    price_delta: Money::from_minor_units(r.get("price_delta_minor"))
                        .map_err(AppError::Domain)?,
                    sort_order: r.get("sort_order"),
                    active: r.get::<i64, _>("active") != 0,
                    created_at: parse_ts(r.get("created_at"))?,
                })
            })
            .collect()
    }

    async fn insert_cash_register(&mut self, register: &CashRegister) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO cash_register (id, organization_id, branch_id, name, code, active, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(register.id.to_string())
        .bind(register.organization_id.to_string())
        .bind(register.branch_id.to_string())
        .bind(&register.name)
        .bind(&register.code)
        .bind(i64::from(register.active))
        .bind(format_ts(register.created_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn list_cash_registers(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<CashRegister>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, branch_id, name, code, active, created_at FROM cash_register WHERE organization_id = ? AND branch_id = ? ORDER BY name",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .fetch_all(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(|row| -> AppResult<CashRegister> {
                Ok(CashRegister {
                    id: parse_id(row.get("id"))?,
                    organization_id: parse_id(row.get("organization_id"))?,
                    branch_id: parse_id(row.get("branch_id"))?,
                    name: row.get("name"),
                    code: row.get("code"),
                    active: row.get::<i64, _>("active") != 0,
                    created_at: parse_ts(row.get("created_at"))?,
                })
            })
            .collect()
    }

    async fn insert_cash_shift(&mut self, shift: &CashShift) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO cash_shift (id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(shift.id.to_string())
        .bind(shift.organization_id.to_string())
        .bind(shift.branch_id.to_string())
        .bind(shift.register_id.to_string())
        .bind(shift.opened_by.to_string())
        .bind(format_ts(shift.opened_at))
        .bind(shift.opening_amount.minor_units())
        .bind(shift.status.as_str())
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn find_open_cash_shift(
        &mut self,
        organization_id: OrganizationId,
        register_id: pos_core::ids::CashRegisterId,
    ) -> AppResult<Option<CashShift>> {
        let row = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status, closed_by, closed_at, expected_amount_minor, counted_amount_minor, difference_minor FROM cash_shift WHERE organization_id = ? AND register_id = ? AND status = 'open'",
        )
        .bind(organization_id.to_string())
        .bind(register_id.to_string())
        .fetch_optional(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| -> AppResult<CashShift> {
            let status = CashShiftStatus::from_key(&row.get::<String, _>("status"))
                .ok_or(AppError::Storage)?;
            Ok(CashShift {
                id: parse_id(row.get("id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                branch_id: parse_id(row.get("branch_id"))?,
                register_id: parse_id(row.get("register_id"))?,
                opened_by: parse_id(row.get("opened_by"))?,
                opened_at: parse_ts(row.get("opened_at"))?,
                opening_amount: Money::from_minor_units(row.get("opening_amount_minor"))
                    .map_err(AppError::Domain)?,
                status,
                closed_by: parse_optional_id(row.get("closed_by"))?,
                closed_at: row
                    .get::<Option<String>, _>("closed_at")
                    .map(parse_ts)
                    .transpose()?,
                expected_amount: row
                    .get::<Option<i64>, _>("expected_amount_minor")
                    .map(Money::from_minor_units)
                    .transpose()
                    .map_err(AppError::Domain)?,
                counted_amount: row
                    .get::<Option<i64>, _>("counted_amount_minor")
                    .map(Money::from_minor_units)
                    .transpose()
                    .map_err(AppError::Domain)?,
                difference_minor: row.get("difference_minor"),
            })
        })
        .transpose()
    }

    async fn find_cash_shift(
        &mut self,
        organization_id: OrganizationId,
        shift_id: pos_core::ids::CashShiftId,
    ) -> AppResult<Option<CashShift>> {
        let row = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status, closed_by, closed_at, expected_amount_minor, counted_amount_minor, difference_minor FROM cash_shift WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.to_string())
        .bind(shift_id.to_string())
        .fetch_optional(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| -> AppResult<CashShift> {
            let status = CashShiftStatus::from_key(&row.get::<String, _>("status"))
                .ok_or(AppError::Storage)?;
            Ok(CashShift {
                id: parse_id(row.get("id"))?,
                organization_id: parse_id(row.get("organization_id"))?,
                branch_id: parse_id(row.get("branch_id"))?,
                register_id: parse_id(row.get("register_id"))?,
                opened_by: parse_id(row.get("opened_by"))?,
                opened_at: parse_ts(row.get("opened_at"))?,
                opening_amount: Money::from_minor_units(row.get("opening_amount_minor"))
                    .map_err(AppError::Domain)?,
                status,
                closed_by: parse_optional_id(row.get("closed_by"))?,
                closed_at: row
                    .get::<Option<String>, _>("closed_at")
                    .map(parse_ts)
                    .transpose()?,
                expected_amount: row
                    .get::<Option<i64>, _>("expected_amount_minor")
                    .map(Money::from_minor_units)
                    .transpose()
                    .map_err(AppError::Domain)?,
                counted_amount: row
                    .get::<Option<i64>, _>("counted_amount_minor")
                    .map(Money::from_minor_units)
                    .transpose()
                    .map_err(AppError::Domain)?,
                difference_minor: row.get("difference_minor"),
            })
        })
        .transpose()
    }

    async fn close_cash_shift(
        &mut self,
        organization_id: OrganizationId,
        shift_id: pos_core::ids::CashShiftId,
        closed_by: UserId,
        closed_at: time::OffsetDateTime,
        expected_amount: Money,
        counted_amount: Money,
        difference_minor: i64,
    ) -> AppResult<()> {
        let result = sqlx::query(
            "UPDATE cash_shift SET status = 'closed', closed_by = ?, closed_at = ?, expected_amount_minor = ?, counted_amount_minor = ?, difference_minor = ? WHERE id = ? AND organization_id = ? AND status = 'open'",
        )
        .bind(closed_by.to_string())
        .bind(format_ts(closed_at))
        .bind(expected_amount.minor_units())
        .bind(counted_amount.minor_units())
        .bind(difference_minor)
        .bind(shift_id.to_string())
        .bind(organization_id.to_string())
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    async fn insert_cash_movement(&mut self, movement: &CashMovement) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO cash_movement (id, organization_id, branch_id, register_id, shift_id, kind, amount_minor, reason, actor_user_id, occurred_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(movement.id.to_string())
        .bind(movement.organization_id.to_string())
        .bind(movement.branch_id.to_string())
        .bind(movement.register_id.to_string())
        .bind(movement.shift_id.to_string())
        .bind(movement.kind.as_str())
        .bind(movement.amount.minor_units())
        .bind(&movement.reason)
        .bind(movement.actor_user_id.map(|id| id.to_string()))
        .bind(format_ts(movement.occurred_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn list_cash_movements(
        &mut self,
        organization_id: OrganizationId,
        shift_id: pos_core::ids::CashShiftId,
    ) -> AppResult<Vec<CashMovement>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, shift_id, kind, amount_minor, reason, actor_user_id, occurred_at FROM cash_movement WHERE organization_id = ? AND shift_id = ? ORDER BY occurred_at",
        )
        .bind(organization_id.to_string())
        .bind(shift_id.to_string())
        .fetch_all(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(|row| -> AppResult<CashMovement> {
                let kind =
                    pos_core::cash::CashMovementKind::from_key(&row.get::<String, _>("kind"))
                        .ok_or(AppError::Storage)?;
                Ok(CashMovement {
                    id: parse_id(row.get("id"))?,
                    organization_id: parse_id(row.get("organization_id"))?,
                    branch_id: parse_id(row.get("branch_id"))?,
                    register_id: parse_id(row.get("register_id"))?,
                    shift_id: parse_id(row.get("shift_id"))?,
                    kind,
                    amount: Money::from_minor_units(row.get("amount_minor"))
                        .map_err(AppError::Domain)?,
                    reason: row.get("reason"),
                    actor_user_id: parse_optional_id(row.get("actor_user_id"))?,
                    occurred_at: parse_ts(row.get("occurred_at"))?,
                })
            })
            .collect()
    }

    async fn list_closed_cash_shifts(
        &mut self,
        organization_id: OrganizationId,
        branch_id: BranchId,
    ) -> AppResult<Vec<CashShift>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, branch_id, register_id, opened_by, opened_at, opening_amount_minor, status, closed_by, closed_at, expected_amount_minor, counted_amount_minor, difference_minor FROM cash_shift WHERE organization_id = ? AND branch_id = ? AND status = 'closed' ORDER BY closed_at DESC",
        )
        .bind(organization_id.to_string())
        .bind(branch_id.to_string())
        .fetch_all(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(|row| -> AppResult<CashShift> {
                Ok(CashShift {
                    id: parse_id(row.get("id"))?,
                    organization_id: parse_id(row.get("organization_id"))?,
                    branch_id: parse_id(row.get("branch_id"))?,
                    register_id: parse_id(row.get("register_id"))?,
                    opened_by: parse_id(row.get("opened_by"))?,
                    opened_at: parse_ts(row.get("opened_at"))?,
                    opening_amount: Money::from_minor_units(row.get("opening_amount_minor"))
                        .map_err(AppError::Domain)?,
                    status: CashShiftStatus::Closed,
                    closed_by: parse_optional_id(row.get("closed_by"))?,
                    closed_at: row
                        .get::<Option<String>, _>("closed_at")
                        .map(parse_ts)
                        .transpose()?,
                    expected_amount: row
                        .get::<Option<i64>, _>("expected_amount_minor")
                        .map(Money::from_minor_units)
                        .transpose()
                        .map_err(AppError::Domain)?,
                    counted_amount: row
                        .get::<Option<i64>, _>("counted_amount_minor")
                        .map(Money::from_minor_units)
                        .transpose()
                        .map_err(AppError::Domain)?,
                    difference_minor: row.get("difference_minor"),
                })
            })
            .collect()
    }

    async fn append_audit(&mut self, event: &AuditEvent) -> AppResult<()> {
        let payload = serde_json::to_string(&event.payload).map_err(|_| AppError::Storage)?;
        sqlx::query(
            "INSERT INTO audit_event \
             (id, organization_id, branch_id, actor_user_id, action, entity_type, entity_id, \
              payload_json, occurred_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.id.to_string())
        .bind(event.organization_id.to_string())
        .bind(event.branch_id.map(|id| id.to_string()))
        .bind(event.actor_user_id.map(|id| id.to_string()))
        .bind(event.action.as_str())
        .bind(&event.entity_type)
        .bind(&event.entity_id)
        .bind(payload)
        .bind(format_ts(event.occurred_at))
        .execute(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn list_audit(
        &mut self,
        organization_id: OrganizationId,
        limit: i64,
    ) -> AppResult<Vec<AuditEvent>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, branch_id, actor_user_id, action, entity_type, entity_id, \
             payload_json, occurred_at FROM audit_event \
             WHERE organization_id = ? ORDER BY occurred_at DESC LIMIT ?",
        )
        .bind(organization_id.to_string())
        .bind(limit)
        .fetch_all(&mut **self.tx()?)
        .await
        .map_err(map_sqlx)?;

        rows.into_iter()
            .map(|row| -> AppResult<AuditEvent> {
                let action = AuditAction::from_key(&row.get::<String, _>("action"))
                    .ok_or(AppError::Storage)?;
                let payload = serde_json::from_str(&row.get::<String, _>("payload_json"))
                    .map_err(|_| AppError::Storage)?;
                Ok(AuditEvent {
                    id: row
                        .get::<String, _>("id")
                        .parse::<AuditEventId>()
                        .map_err(|_| AppError::Storage)?,
                    organization_id: parse_id(row.get("organization_id"))?,
                    branch_id: parse_optional_id(row.get("branch_id"))?,
                    actor_user_id: parse_optional_id(row.get("actor_user_id"))?,
                    action,
                    entity_type: row.get("entity_type"),
                    entity_id: row.get("entity_id"),
                    payload,
                    occurred_at: parse_ts(row.get("occurred_at"))?,
                })
            })
            .collect()
    }

    async fn commit(self: Box<Self>) -> AppResult<()> {
        let mut this = self;
        let tx = this.tx.take().ok_or(AppError::Storage)?;
        tx.commit().await.map_err(map_sqlx)
    }

    async fn rollback(self: Box<Self>) -> AppResult<()> {
        let mut this = self;
        let tx = this.tx.take().ok_or(AppError::Storage)?;
        tx.rollback().await.map_err(map_sqlx)
    }
}

fn format_ts(value: OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap_or_default()
}

fn parse_ts(value: String) -> AppResult<OffsetDateTime> {
    OffsetDateTime::parse(&value, &Rfc3339).map_err(|_| AppError::Storage)
}

fn parse_id<T: FromStr>(value: String) -> AppResult<T> {
    value.parse::<T>().map_err(|_| AppError::Storage)
}

fn parse_optional_id<T: FromStr>(value: Option<String>) -> AppResult<Option<T>> {
    value
        .map(|raw| raw.parse::<T>().map_err(|_| AppError::Storage))
        .transpose()
}

fn map_sqlx(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_error) = &error
        && db_error.is_unique_violation()
    {
        return AppError::Conflict;
    }
    tracing::error!(error = %error, "fallo de almacenamiento SQLite");
    AppError::Storage
}
