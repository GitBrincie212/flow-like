use crate::{
    ensure_permission, entity::role, error::ApiError, middleware::jwt::AppUser,
    permission::role_permission::RolePermissions, state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use flow_like_types::create_id;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};

#[tracing::instrument(name = "PUT /apps/{app_id}/roles/{role_id}", skip(state, user))]
pub async fn upsert_role(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path((app_id, role_id)): Path<(String, String)>,
    Json(mut payload): Json<role::Model>,
) -> Result<Json<()>, ApiError> {
    ensure_permission!(user, &app_id, &state, RolePermissions::Admin);
    let permission = RolePermissions::from_bits(payload.permissions).ok_or(ApiError::Forbidden)?;

    let txn = state.db.begin().await?;

    let is_owner = permission.contains(RolePermissions::Owner);

    let role = role::Entity::find_by_id(role_id.clone())
        .filter(role::Column::AppId.eq(app_id.clone()))
        .one(&txn)
        .await?;

    if let Some(role) = role {
        println!("Updating role: {:?}", role);
        let permission = RolePermissions::from_bits(role.permissions).ok_or(ApiError::Forbidden)?;

        payload.id = role.id;
        payload.created_at = role.created_at;
        payload.updated_at = chrono::Utc::now().naive_utc();
        payload.app_id = role.app_id;

        if permission.contains(RolePermissions::Owner) {
            payload.permissions = role.permissions;
        }

        if is_owner && !permission.contains(RolePermissions::Owner) {
            tracing::warn!("Attempt to update a role with Owner permission");
            return Err(ApiError::Forbidden);
        }

        let payload: role::ActiveModel = payload.into();
        let payload = payload.reset_all();
        payload.update(&txn).await?;
        txn.commit().await?;

        return Ok(Json(()));
    }

    if is_owner {
        tracing::warn!("Attempt to create a role with Owner permission");
        return Err(ApiError::Forbidden);
    }

    payload.id = create_id();
    payload.created_at = chrono::Utc::now().naive_utc();
    payload.updated_at = chrono::Utc::now().naive_utc();
    payload.app_id = Some(app_id.clone());

    let role: role::ActiveModel = payload.into();
    let role = role.reset_all();
    role.insert(&txn).await?;
    txn.commit().await?;

    Ok(Json(()))
}
