//! Catalogo: productos, categorias y variantes.

use super::*;

pub(super) async fn insert_product(tx: &mut SqliteTx, product: &Product) -> AppResult<()> {
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
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn list_products(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
) -> AppResult<Vec<Product>> {
    let rows = sqlx::query(
            "SELECT id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, category_id, created_at \
             FROM product WHERE organization_id = ? ORDER BY name COLLATE NOCASE",
        )
        .bind(organization_id.to_string())
        .fetch_all(&mut **tx.tx()?)
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

pub(super) async fn list_catalog_configuration(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
) -> AppResult<CatalogConfiguration> {
    let variant_rows = sqlx::query(
            "SELECT id, organization_id, product_id, name, sku, unit_price_minor, active, sort_order, created_at \
             FROM product_variant WHERE organization_id = ? AND active = 1 \
             ORDER BY product_id, sort_order, name COLLATE NOCASE",
        )
        .bind(organization_id.to_string())
        .fetch_all(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;

    let variants = variant_rows
        .into_iter()
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
        .collect::<AppResult<Vec<_>>>()?;

    let modifier_rows = sqlx::query(
        "SELECT DISTINCT product_id FROM modifier_group WHERE organization_id = ? AND active = 1",
    )
    .bind(organization_id.to_string())
    .fetch_all(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;

    let products_with_modifiers = modifier_rows
        .into_iter()
        .map(|row| parse_id::<ProductId>(row.get("product_id")))
        .collect::<AppResult<_>>()?;

    Ok(CatalogConfiguration {
        variants,
        products_with_modifiers,
    })
}

pub(super) async fn insert_category(tx: &mut SqliteTx, category: &Category) -> AppResult<()> {
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
    .execute(&mut **tx.tx()?)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn find_category(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    category_id: CategoryId,
) -> AppResult<Option<Category>> {
    let row = sqlx::query(
        "SELECT id, organization_id, name, sort_order, active, created_at \
             FROM category WHERE id = ? AND organization_id = ?",
    )
    .bind(category_id.to_string())
    .bind(organization_id.to_string())
    .fetch_optional(&mut **tx.tx()?)
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

pub(super) async fn update_product_category(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    product_id: ProductId,
    category_id: Option<CategoryId>,
) -> AppResult<()> {
    let result =
        sqlx::query("UPDATE product SET category_id = ? WHERE id = ? AND organization_id = ?")
            .bind(category_id.map(|id| id.to_string()))
            .bind(product_id.to_string())
            .bind(organization_id.to_string())
            .execute(&mut **tx.tx()?)
            .await
            .map_err(map_sqlx)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub(super) async fn list_categories(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
) -> AppResult<Vec<Category>> {
    let rows = sqlx::query(
        "SELECT id, organization_id, name, sort_order, active, created_at \
             FROM category WHERE organization_id = ? ORDER BY sort_order, name COLLATE NOCASE",
    )
    .bind(organization_id.to_string())
    .fetch_all(&mut **tx.tx()?)
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

pub(super) async fn insert_product_variant(
    tx: &mut SqliteTx,
    variant: &ProductVariant,
) -> AppResult<()> {
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
        .execute(&mut **tx.tx()?)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(super) async fn list_product_variants(
    tx: &mut SqliteTx,
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
        .fetch_all(&mut **tx.tx()?)
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

pub(super) async fn find_product(
    tx: &mut SqliteTx,
    organization_id: OrganizationId,
    product_id: ProductId,
) -> AppResult<Option<Product>> {
    let row = sqlx::query(
            "SELECT id, organization_id, sku, name, unit_price_minor, tax_rate_bp, active, category_id, created_at \
             FROM product WHERE id = ? AND organization_id = ?",
        )
        .bind(product_id.to_string())
        .bind(organization_id.to_string())
        .fetch_optional(&mut **tx.tx()?)
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
