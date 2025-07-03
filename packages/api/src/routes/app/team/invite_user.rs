use crate::{
    ensure_in_project, ensure_permission,
    entity::{
        app, invitation, invite_link, membership, meta, role, sea_orm_active_enums::Visibility,
    },
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
use flow_like_types::{anyhow, bail, create_id};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, TransactionTrait, prelude::Expr,
};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct InviteUserParams {
    pub message: Option<String>,
    pub sub: String,
}

#[tracing::instrument(name = "PUT /apps/{app_id}/team/invite", skip(state, user))]
pub async fn invite_user(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path(app_id): Path<String>,
    Json(params): Json<InviteUserParams>,
) -> Result<Json<()>, ApiError> {
    let permission = ensure_permission!(user, &app_id, &state, RolePermissions::Admin);

    if params.sub == permission.sub()? {
        tracing::warn!(
            "User {} is trying to invite themself to app {}",
            user.sub()?,
            app_id
        );
        return Err(ApiError::Forbidden);
    }

    let txn = state.db.begin().await?;

    let max_prototype = state.platform_config.max_users_prototype.unwrap_or(-1);

    let (app, meta) = app::Entity::find_by_id(app_id.clone())
        .find_also_related(meta::Entity)
        .filter(meta::Column::Lang.eq("en"))
        .one(&txn)
        .await?
        .ok_or_else(|| ApiError::NotFound)?;

    if app.default_role_id.is_none() {
        tracing::warn!(
            "App {} does not have a default role set, cannot invite user",
            app_id
        );
        return Err(ApiError::Internal(
            anyhow!("App does not have a default role set").into(),
        ));
    }

    if matches!(app.visibility, Visibility::Private | Visibility::Offline) {
        tracing::warn!(
            "User {} is trying to invite a user to app {} but the app is not public",
            user.sub()?,
            app_id
        );
        return Err(ApiError::Forbidden);
    }

    let user_already_member = membership::Entity::find()
        .filter(membership::Column::AppId.eq(app_id.clone()))
        .filter(membership::Column::UserId.eq(params.sub.clone()))
        .one(&txn)
        .await?;

    if user_already_member.is_some() {
        tracing::warn!(
            "User {} is trying to invite {} to app {} but the user is already a member",
            user.sub()?,
            params.sub,
            app_id
        );
        return Err(ApiError::Forbidden);
    }

    if max_prototype > 0
        && !matches!(
            app.visibility,
            Visibility::Public | Visibility::PublicRequestAccess
        )
    {
        let count = membership::Entity::find()
            .filter(membership::Column::AppId.eq(app_id.clone()))
            .count(&txn)
            .await?;

        if count >= max_prototype as u64 {
            tracing::warn!(
                "User {} is trying to invite a user to app {} but the app has reached its user limit",
                user.sub()?,
                app_id
            );
            return Err(ApiError::Forbidden);
        }
    }

    let existing_invite = invitation::Entity::find()
        .filter(invitation::Column::AppId.eq(app_id.clone()))
        .filter(invitation::Column::UserId.eq(params.sub.clone()))
        .one(&txn)
        .await?;

    if existing_invite.is_some() {
        tracing::warn!(
            "User {} is trying to invite {} to app {} but the user already has an invite",
            user.sub()?,
            params.sub,
            app_id
        );
        return Err(ApiError::Forbidden);
    }

    let invitation = invitation::ActiveModel {
        id: Set(create_id()),
        app_id: Set(app_id.clone()),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
        by_member_id: Set(user.sub()?),
        message: Set(params.message),
        user_id: Set(params.sub),
        name: Set(meta
            .as_ref()
            .map_or("Unknown App".to_string(), |m| m.name.clone())),
        description: Set(meta.as_ref().and_then(|m| m.description.clone())),
    };

    invitation.insert(&txn).await?;
    txn.commit().await?;

    Ok(Json(()))
}
