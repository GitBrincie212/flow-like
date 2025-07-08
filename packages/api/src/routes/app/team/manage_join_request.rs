use crate::{
    ensure_permission,
    entity::{app, join_queue, membership, sea_orm_active_enums::Visibility},
    error::ApiError,
    middleware::jwt::AppUser,
    permission::role_permission::RolePermissions,
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use flow_like_types::create_id;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    TransactionTrait,
};

#[tracing::instrument(
    name = "POST /apps/{app_id}/team/queue/{request_id}",
    skip(state, user)
)]
pub async fn accept_join_request(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path((app_id, request_id)): Path<(String, String)>,
) -> Result<Json<()>, ApiError> {
    ensure_permission!(user, &app_id, &state, RolePermissions::Admin);

    let max_prototypes = state.platform_config.max_users_prototype.unwrap_or(-1);

    let txn = state.db.begin().await?;

    let request = join_queue::Entity::find()
        .filter(join_queue::Column::AppId.eq(app_id.clone()))
        .filter(join_queue::Column::Id.eq(request_id.clone()))
        .one(&txn)
        .await?
        .ok_or_else(|| ApiError::NotFound)?;

    let already_member = membership::Entity::find()
        .filter(membership::Column::AppId.eq(app_id.clone()))
        .filter(membership::Column::UserId.eq(request.user_id.clone()))
        .one(&txn)
        .await?;

    if already_member.is_some() {
        tracing::warn!(
            "User {} is trying to join app {} but is already a member",
            request.user_id,
            app_id
        );
        return Err(ApiError::Forbidden);
    }

    let app = app::Entity::find_by_id(app_id.clone())
        .one(&txn)
        .await?
        .ok_or_else(|| ApiError::NotFound)?;

    if matches!(app.visibility, Visibility::Private | Visibility::Offline) {
        tracing::warn!(
            "User {} is trying let a user join to app {} but the app is not public",
            user.sub()?,
            app_id
        );
        return Err(ApiError::Forbidden);
    }

    if max_prototypes > 0
        && !matches!(
            app.visibility,
            Visibility::Public | Visibility::PublicRequestAccess
        )
    {
        let current_members_count = membership::Entity::find()
            .filter(membership::Column::AppId.eq(app_id.clone()))
            .count(&txn)
            .await?;

        if current_members_count >= max_prototypes as u64 {
            tracing::warn!(
                "User {} is trying to join app {} but the app has reached its maximum members",
                request.user_id,
                app_id
            );
            return Err(ApiError::Forbidden);
        }
    }

    let default_role_id = app
        .default_role_id
        .clone()
        .ok_or_else(|| ApiError::NotFound)?;

    let membership = membership::ActiveModel {
        id: Set(create_id()),
        user_id: Set(request.user_id.clone()),
        app_id: Set(app_id.clone()),
        role_id: Set(default_role_id),
        created_at: Set(chrono::Utc::now().naive_utc()),
        joined_via: Set(Some("accept_join_request".to_string())),
        updated_at: Set(chrono::Utc::now().naive_utc()),
    };

    let request: join_queue::ActiveModel = request.into();

    request.delete(&txn).await?;
    membership.insert(&txn).await?;

    txn.commit().await?;

    tracing::info!(
        "Join request {} for app {} has been accepted by user {}",
        request_id,
        app_id,
        user.sub()?
    );

    Ok(Json(()))
}

#[tracing::instrument(
    name = "DELETE /apps/{app_id}/team/queue/{request_id}",
    skip(state, user)
)]
pub async fn reject_join_request(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path((app_id, request_id)): Path<(String, String)>,
) -> Result<Json<()>, ApiError> {
    ensure_permission!(user, &app_id, &state, RolePermissions::Admin);

    let txn = state.db.begin().await?;
    let request = join_queue::Entity::find()
        .filter(join_queue::Column::AppId.eq(app_id.clone()))
        .filter(join_queue::Column::Id.eq(request_id.clone()))
        .one(&txn)
        .await?
        .ok_or_else(|| ApiError::NotFound)?;

    let request: join_queue::ActiveModel = request.into();
    request.delete(&txn).await?;
    txn.commit().await?;
    tracing::info!(
        "Join request {} for app {} has been declined by user {}",
        request_id,
        app_id,
        user.sub()?
    );

    Ok(Json(()))
}
