use crate::{
    ensure_in_project, ensure_permission,
    entity::{app, membership, meta, role},
    error::ApiError,
    middleware::jwt::AppUser,
    permission::role_permission::RolePermissions,
    routes::LanguageParams,
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use flow_like::{app::App, bit::Metadata};
use flow_like_types::{anyhow, bail};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, EntityTrait, JoinType, QueryFilter, QueryOrder, QuerySelect,
    RelationTrait, TransactionTrait, prelude::Expr,
};

#[tracing::instrument(
    name = "POST /apps/{app_id}/roles/{role_id}/assign/{sub}",
    skip(state, user)
)]
pub async fn assign_role(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path((app_id, role_id, sub)): Path<(String, String, String)>,
) -> Result<Json<()>, ApiError> {
    ensure_permission!(user, &app_id, &state, RolePermissions::Admin);
    let called_sub = user.sub()?;

    if called_sub == sub {
        tracing::warn!(
            "User {} is trying to assign a role to themselves in app {}",
            called_sub,
            app_id
        );
        return Err(ApiError::Forbidden);
    }

    let txn = state.db.begin().await?;

    let target_role = role::Entity::find_by_id(role_id.clone())
        .filter(role::Column::AppId.eq(app_id.clone()))
        .one(&txn)
        .await?
        .ok_or_else(|| ApiError::NotFound)?;

    let target_permission =
        RolePermissions::from_bits(target_role.permissions).ok_or_else(|| ApiError::Forbidden)?;

    let target_current_role = role::Entity::find()
        .inner_join(membership::Entity)
        .filter(membership::Column::AppId.eq(app_id.clone()))
        .filter(membership::Column::UserId.eq(sub.clone()))
        .one(&txn)
        .await?
        .ok_or_else(|| ApiError::NotFound)?;

    let target_current_permission =
        RolePermissions::from_bits(target_current_role.permissions).ok_or(ApiError::Forbidden)?;

    // Owners can not remove their own permission. Every project has to have exactly one owner.
    if target_current_permission.contains(RolePermissions::Owner) {
        tracing::warn!(
            "User {} already has owner permissions in app {}",
            sub,
            app_id
        );
        return Err(ApiError::Forbidden);
    }

    if target_permission.contains(RolePermissions::Owner) {
        let caller_sub = user.sub()?;
        let caller_role = role::Entity::find()
            .inner_join(membership::Entity)
            .filter(membership::Column::AppId.eq(app_id.clone()))
            .filter(membership::Column::UserId.eq(caller_sub.clone()))
            .one(&txn)
            .await?
            .ok_or_else(|| ApiError::NotFound)?;

        let caller_permissions = RolePermissions::from_bits(caller_role.permissions)
            .ok_or_else(|| ApiError::Forbidden)?;

        if !caller_permissions.contains(RolePermissions::Owner) {
            tracing::warn!(
                "User {} is trying to assign owner permissions to {} in app {}, but does not have owner permissions themselves",
                caller_sub,
                sub,
                app_id
            );
            return Err(ApiError::Forbidden);
        }

        tracing::warn!(
            "User {} is transferring owner permissions to {} in app {}",
            caller_sub,
            sub,
            app_id
        );

        let app = app::Entity::find_by_id(app_id.clone())
            .one(&txn)
            .await?
            .ok_or_else(|| ApiError::NotFound)?;

        if let Some(default_role) = app.default_role_id {
            let new_role_for_owner = role::Entity::find_by_id(default_role.clone())
                .filter(role::Column::AppId.eq(app_id.clone()))
                .one(&txn)
                .await?
                .ok_or_else(|| ApiError::NotFound)?;

            let new_owner = membership::Entity::update_many()
                .filter(membership::Column::AppId.eq(app_id.clone()))
                .filter(membership::Column::UserId.eq(sub.clone()))
                .col_expr(
                    membership::Column::RoleId,
                    Expr::value(target_role.id.clone()),
                )
                .exec_with_returning(&txn)
                .await?;

            let updated_owner = membership::Entity::update_many()
                .filter(membership::Column::AppId.eq(app_id.clone()))
                .filter(membership::Column::UserId.eq(caller_sub.clone()))
                .col_expr(
                    membership::Column::RoleId,
                    Expr::value(new_role_for_owner.id.clone()),
                )
                .exec_with_returning(&txn)
                .await?;

            if new_owner.len() != 1 || updated_owner.len() != 1 {
                tracing::error!(
                    "Failed to update roles for user {} and new owner {} in app {}",
                    sub,
                    caller_sub,
                    app_id
                );
                return Err(ApiError::App(
                    anyhow!("Failed to update roles for user and new owner".to_string()).into(),
                ));
            }

            txn.commit().await?;
            return Ok(Json(()));
        }

        return Err(ApiError::Forbidden);
    }

    tracing::info!(
        "Assigning role {} to user {} in app {}, by user {}",
        role_id,
        sub,
        app_id,
        user.sub()?
    );

    membership::Entity::update_many()
        .filter(membership::Column::AppId.eq(app_id.clone()))
        .filter(membership::Column::UserId.eq(sub.clone()))
        .col_expr(
            membership::Column::RoleId,
            Expr::value(target_role.id.clone()),
        )
        .exec(&txn)
        .await?;

    txn.commit().await?;
    Ok(Json(()))
}
