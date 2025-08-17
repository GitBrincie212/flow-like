use std::time::Duration;

use crate::{
    entity::user, error::ApiError, middleware::jwt::AppUser, state::AppState,
    user_management::UserManagement,
};
use axum::{Extension, Json, extract::State};
use flow_like_types::create_id;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UpsertInfoBody {
    pub name: Option<String>,
    pub description: Option<String>,
    pub avatar_extension: Option<String>,
    pub accepted_terms_version: Option<String>,
    pub tutorial_completed: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UpsertInfoResponse {
    pub signed_url: Option<String>,
}

#[tracing::instrument(name = "PUT /user/info", skip(state, user))]
pub async fn upsert_info(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Json(payload): Json<UpsertInfoBody>,
) -> Result<Json<UpsertInfoResponse>, ApiError> {
    let sub = user.sub()?;
    let username = user.username().clone();
    let mut response = UpsertInfoResponse { signed_url: None };
    let user_manager = UserManagement::new(&state).await;

    let email = user_manager.get_attribute(&sub, &username, "email").await?;
    let preferred_username = user_manager
        .get_attribute(&sub, &username, "preferred_username")
        .await?;

    let current_user = user::Entity::find_by_id(&sub)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    let mut updated_user: user::ActiveModel = current_user.clone().into();

    if let Some(email) = email
        && current_user.email != Some(email.clone())
    {
        updated_user.email = Set(Some(email));
    }

    if let Some(preferred_username) = preferred_username
        && current_user.preferred_username != Some(preferred_username.clone())
    {
        updated_user.preferred_username = Set(Some(preferred_username));
    }

    if let Some(name) = payload.name {
        updated_user.name = Set(Some(name));
    }
    if let Some(description) = payload.description {
        updated_user.description = Set(Some(description));
    }
    if let Some(avatar_extension) = payload.avatar_extension {
        let master_store = state.master_credentials().await?;
        let master_store = master_store.to_store(false).await?;

        if let Some(avatar) = &current_user.avatar {
            let file_name = format!("{}.webp", avatar);
            let path = flow_like_storage::Path::from("media")
                .child("users")
                .child(sub.clone())
                .child(file_name);
            if let Err(err) = master_store.as_generic().delete(&path).await {
                tracing::error!("Failed to delete existing avatar at {}: {:?}", path, err);
            }
        }

        let id = create_id();
        updated_user.avatar = Set(Some(id.clone()));

        let path = flow_like_storage::Path::from("media")
            .child("users")
            .child(sub.clone())
            .child(format!("{}.{}", id, avatar_extension));
        let signed_url = master_store
            .sign("PUT", &path, Duration::from_secs(60 * 5))
            .await?;
        response.signed_url = Some(signed_url.to_string());
    }

    if let Some(accepted_terms_version) = payload.accepted_terms_version {
        updated_user.accepted_terms_version = Set(Some(accepted_terms_version));
    }

    if let Some(tutorial_completed) = payload.tutorial_completed {
        updated_user.tutorial_completed = Set(tutorial_completed);
    }
    updated_user.updated_at = Set(chrono::Utc::now().naive_utc());
    updated_user.update(&state.db).await?;

    Ok(Json(response))
}
