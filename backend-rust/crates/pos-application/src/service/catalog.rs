//! Catalogo: productos, categorias, variantes y modificadores.

use super::*;

impl PosService {
    pub fn preview_product_price(
        &self,
        actor: &Actor,
        price: &str,
        tax_rate_bp: u16,
    ) -> AppResult<ProductPricePreviewView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let unit_price: Money = price.trim().parse()?;
        let tax = pos_core::pricing::unit_tax(unit_price, tax_rate_bp)?;
        let public_unit_total = pos_core::pricing::public_unit_total(unit_price, tax_rate_bp)?;
        Ok(ProductPricePreviewView {
            unit_price: unit_price.to_string(),
            tax: tax.to_string(),
            public_unit_total: public_unit_total.to_string(),
            tax_rate_bp,
        })
    }

    pub async fn create_product(
        &self,
        actor: &Actor,
        draft: ProductDraft,
    ) -> AppResult<ProductView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let price: Money = draft.price.parse()?;
        let category_id = draft
            .category_id
            .as_deref()
            .map(str::parse::<pos_core::ids::CategoryId>)
            .transpose()
            .map_err(|_| pos_core::DomainError::CategoryNotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;

        if let Some(category_id) = category_id {
            let category = tx
                .find_category(actor.organization_id, category_id)
                .await?
                .ok_or(pos_core::DomainError::CategoryNotFound)?;
            if !category.active {
                return Err(pos_core::DomainError::InactiveCategory.into());
            }
        }

        let mut product = Product::new(
            ProductId::new(),
            actor.organization_id,
            draft.sku,
            draft.name,
            price,
            draft.tax_rate_bp,
        )?;
        product.category_id = category_id;

        tx.insert_product(&product).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ProductCreated,
            "product",
            product.id.to_string(),
            serde_json::json!({ "sku": product.sku, "price": product.unit_price.to_string() }),
        ))
        .await?;
        tx.commit().await?;

        ProductView::of(&product)
    }

    pub async fn assign_product_category(
        &self,
        actor: &Actor,
        product_id: ProductId,
        category_id: Option<String>,
    ) -> AppResult<ProductView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let category_id = category_id
            .as_deref()
            .map(str::parse::<pos_core::ids::CategoryId>)
            .transpose()
            .map_err(|_| pos_core::DomainError::CategoryNotFound)?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;

        tx.find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        if let Some(category_id) = category_id {
            let category = tx
                .find_category(actor.organization_id, category_id)
                .await?
                .ok_or(pos_core::DomainError::CategoryNotFound)?;
            if !category.active {
                return Err(pos_core::DomainError::InactiveCategory.into());
            }
        }
        tx.update_product_category(actor.organization_id, product_id, category_id)
            .await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ProductCategoryChanged,
            "product",
            product_id.to_string(),
            serde_json::json!({ "category_id": category_id.map(|id| id.to_string()) }),
        ))
        .await?;
        let product = tx
            .find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        tx.commit().await?;
        ProductView::of(&product)
    }

    pub async fn list_products(&self, actor: &Actor) -> AppResult<Vec<ProductView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let products = tx.list_products(actor.organization_id).await?;
        let configuration = tx.list_catalog_configuration(actor.organization_id).await?;
        tx.rollback().await?;

        // Las variantes se agrupan una vez: el catálogo completo se presenta sin
        // una consulta ni una llamada IPC por producto.
        let mut variants_by_product: std::collections::HashMap<ProductId, Vec<&ProductVariant>> =
            std::collections::HashMap::new();
        for variant in &configuration.variants {
            variants_by_product
                .entry(variant.product_id)
                .or_default()
                .push(variant);
        }

        products
            .iter()
            .map(|product| {
                let variants = variants_by_product
                    .get(&product.id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                ProductView::with_configuration(
                    product,
                    variants,
                    configuration.products_with_modifiers.contains(&product.id),
                )
            })
            .collect()
    }

    pub async fn create_category(
        &self,
        actor: &Actor,
        draft: CategoryDraft,
    ) -> AppResult<CategoryView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;

        let category = Category::new(actor.organization_id, draft.name, draft.sort_order)?;
        tx.insert_category(&category).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::CategoryCreated,
            "category",
            category.id.to_string(),
            serde_json::json!({ "name": category.name, "sort_order": category.sort_order }),
        ))
        .await?;
        tx.commit().await?;
        Ok(CategoryView::from(&category))
    }

    pub async fn list_categories(&self, actor: &Actor) -> AppResult<Vec<CategoryView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let categories = tx.list_categories(actor.organization_id).await?;
        tx.rollback().await?;
        Ok(categories.iter().map(CategoryView::from).collect())
    }

    pub async fn create_product_variant(
        &self,
        actor: &Actor,
        draft: ProductVariantDraft,
    ) -> AppResult<ProductVariantView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let product_id: ProductId = draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let price: Money = draft.price.parse()?;

        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;

        tx.find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;

        let variant = ProductVariant::new(
            actor.organization_id,
            product_id,
            draft.name,
            draft.sku,
            price,
            draft.sort_order,
        )?;
        tx.insert_product_variant(&variant).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ProductVariantCreated,
            "product_variant",
            variant.id.to_string(),
            serde_json::json!({ "product_id": product_id.to_string(), "name": variant.name, "price": variant.unit_price.to_string() }),
        ))
        .await?;
        tx.commit().await?;
        Ok(ProductVariantView::from(&variant))
    }

    pub async fn list_product_variants(
        &self,
        actor: &Actor,
        product_id: ProductId,
    ) -> AppResult<Vec<ProductVariantView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let variants = tx
            .list_product_variants(actor.organization_id, product_id)
            .await?;
        tx.rollback().await?;
        Ok(variants.iter().map(ProductVariantView::from).collect())
    }

    pub async fn create_modifier_group(
        &self,
        actor: &Actor,
        draft: ModifierGroupDraft,
    ) -> AppResult<ModifierGroupView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let product_id: pos_core::ids::ProductId =
            draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;
        tx.find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        let group = ModifierGroup::new(
            actor.organization_id,
            product_id,
            draft.name,
            draft.min,
            draft.max,
            draft.sort_order,
        )?;
        tx.insert_modifier_group(&group).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ModifierGroupCreated,
            "modifier_group",
            group.id.to_string(),
            serde_json::json!({ "product_id": product_id.to_string(), "name": group.name }),
        ))
        .await?;
        tx.commit().await?;
        Ok(ModifierGroupView::from(&group))
    }

    pub async fn list_modifier_groups(
        &self,
        actor: &Actor,
        product_id: pos_core::ids::ProductId,
    ) -> AppResult<Vec<ModifierGroupView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let groups = tx
            .list_modifier_groups(actor.organization_id, product_id)
            .await?;
        tx.rollback().await?;
        Ok(groups.iter().map(ModifierGroupView::from).collect())
    }

    pub async fn create_modifier_option(
        &self,
        actor: &Actor,
        draft: ModifierOptionDraft,
    ) -> AppResult<ModifierOptionView> {
        actor.require(pos_core::identity::Permission::ManageCatalog)?;
        let group_id: pos_core::ids::ModifierGroupId =
            draft.group_id.parse().map_err(|_| AppError::NotFound)?;
        let kind = OptionKind::from_key(&draft.kind).ok_or(AppError::NotFound)?;
        let price_delta: Money = draft.price_delta.parse()?;
        let mut tx = self.store.begin().await?;
        let commercial = require_commercial(tx.as_mut()).await?;
        commercial.require_writes_allowed(self.clock.now())?;
        commercial.require_feature(FeatureKey::Catalog, self.clock.now())?;
        let group = tx
            .find_modifier_group(actor.organization_id, group_id)
            .await?
            .ok_or(pos_core::DomainError::CategoryNotFound)?;
        // product validity ensured when the group was created; re-check organization only.
        let _ = group.product_id;
        let option = ModifierOption::new(
            actor.organization_id,
            group_id,
            draft.name,
            kind,
            price_delta,
            draft.sort_order,
        )?;
        tx.insert_modifier_option(&option).await?;
        tx.append_audit(&AuditEvent::new(
            actor.organization_id,
            actor.branch_id,
            Some(actor.user_id),
            AuditAction::ModifierOptionCreated,
            "modifier_option",
            option.id.to_string(),
            serde_json::json!({ "group_id": group_id.to_string(), "name": option.name }),
        ))
        .await?;
        tx.commit().await?;
        Ok(ModifierOptionView::from(&option))
    }

    pub async fn list_modifier_options(
        &self,
        actor: &Actor,
        group_id: pos_core::ids::ModifierGroupId,
    ) -> AppResult<Vec<ModifierOptionView>> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let mut tx = self.store.begin().await?;
        let options = tx
            .list_modifier_options(actor.organization_id, group_id)
            .await?;
        tx.rollback().await?;
        Ok(options.iter().map(ModifierOptionView::from).collect())
    }

    pub async fn configure_product(
        &self,
        actor: &Actor,
        draft: ModifierConfigurationDraft,
    ) -> AppResult<ModifierConfigurationView> {
        actor.require(pos_core::identity::Permission::Sell)?;
        let product_id: pos_core::ids::ProductId =
            draft.product_id.parse().map_err(|_| AppError::NotFound)?;
        let variant_id = draft
            .variant_id
            .as_deref()
            .map(str::parse::<pos_core::ids::ProductVariantId>)
            .transpose()
            .map_err(|_| AppError::NotFound)?;
        let selected_ids: Vec<pos_core::ids::ModifierOptionId> = draft
            .selected_option_ids
            .iter()
            .map(|value| value.parse())
            .collect::<Result<_, _>>()
            .map_err(|_| AppError::NotFound)?;

        let mut tx = self.store.begin().await?;
        let product = tx
            .find_product(actor.organization_id, product_id)
            .await?
            .ok_or(pos_core::DomainError::ProductNotFound)?;
        let variant = if let Some(variant_id) = variant_id {
            Some(
                tx.list_product_variants(actor.organization_id, product_id)
                    .await?
                    .into_iter()
                    .find(|value| value.id == variant_id)
                    .ok_or(AppError::NotFound)?,
            )
        } else {
            None
        };
        let groups = tx
            .list_modifier_groups(actor.organization_id, product_id)
            .await?;
        let mut options = Vec::new();
        for group in &groups {
            options.extend(
                tx.list_modifier_options(actor.organization_id, group.id)
                    .await?,
            );
        }
        tx.rollback().await?;

        let configuration = pos_core::modifiers::configure(
            &product,
            variant.as_ref(),
            &groups,
            &options,
            &selected_ids,
        )?;
        Ok(ModifierConfigurationView::from(configuration))
    }
}
