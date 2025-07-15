use crate::{
    ensure_permission,
    entity::{app, membership, role},
    error::ApiError,
    middleware::jwt::AppUser,
    permission::role_permission::RolePermissions,
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait, prelude::Expr,
};

#[tracing::instrument(name = "DELETE /apps/{app_id}/roles/{role_id}", skip(state, user))]
pub async fn delete_role(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path((app_id, role_id)): Path<(String, String)>,
) -> Result<Json<()>, ApiError> {
    ensure_permission!(user, &app_id, &state, RolePermissions::Admin);

    let txn = state.db.begin().await?;

    let (role, app) = role::Entity::find_by_id(role_id.clone())
        .filter(role::Column::AppId.eq(app_id.clone()))
        .find_also_related(app::Entity)
        .one(&txn)
        .await?
        .ok_or(ApiError::NotFound)?;

    let app = app.ok_or(ApiError::NotFound)?;
    let default_role_id = app.default_role_id.ok_or(ApiError::NotFound)?;

    if role_id == default_role_id {
        tracing::warn!(
            "User {} is trying to delete the default role {} in app {}",
            user.sub()?,
            role_id,
            app_id
        );
        return Err(ApiError::Forbidden);
    }

    let Some(permission) = RolePermissions::from_bits(role.permissions) else {
        return Err(ApiError::Forbidden);
    };

    if permission.contains(RolePermissions::Owner) {
        return Err(ApiError::Forbidden);
    }

    membership::Entity::update_many()
        .filter(membership::Column::AppId.eq(app_id))
        .filter(membership::Column::RoleId.eq(role_id))
        .col_expr(membership::Column::RoleId, Expr::value(default_role_id))
        .exec(&txn)
        .await?;

    let role: role::ActiveModel = role.into();
    role.delete(&txn).await?;

    txn.commit().await?;

    Ok(Json(()))
}
