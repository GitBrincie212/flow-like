use crate::{
    ensure_permission, entity::invite_link, error::ApiError, middleware::jwt::AppUser,
    permission::role_permission::RolePermissions, state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use flow_like_types::create_id;
use sea_orm::ActiveModelTrait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInviteLinkPayload {
    pub name: Option<String>,
    pub max_uses: Option<i64>,
}

#[tracing::instrument(name = "PUT /apps/{app_id}/team/link", skip(state, user))]
pub async fn create_invite_link(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path(app_id): Path<String>,
    Json(payload): Json<CreateInviteLinkPayload>,
) -> Result<Json<()>, ApiError> {
    ensure_permission!(user, &app_id, &state, RolePermissions::Admin);

    let nonce = create_id();

    let new_link = invite_link::Model {
        id: create_id(),
        app_id: app_id.clone(),
        name: payload.name,
        count_joined: 0,
        max_uses: payload.max_uses.unwrap_or(-1), // -1 means unlimited uses
        created_at: chrono::Utc::now().naive_utc(),
        updated_at: chrono::Utc::now().naive_utc(),
        token: nonce.clone(),
    };

    let new_link: invite_link::ActiveModel = new_link.into();
    new_link.insert(&state.db).await?;

    Ok(Json(()))
}
