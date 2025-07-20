use std::time::Duration;

use crate::{
    entity::meta,
    error::ApiError,
    middleware::jwt::AppUser,
    routes::app::meta::{MediaItem, MediaQuery, MetaMode},
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use flow_like_storage::Path as FlowPath;
use flow_like_types::{anyhow, create_id};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, TransactionTrait};

#[tracing::instrument(
    name = "DELETE /apps/{app_id}/meta/media/{media_id}",
    skip(state, user)
)]
pub async fn remove_media(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path((app_id, media_id)): Path<(String, String)>,
    Query(query): Query<MediaQuery>,
) -> Result<Json<()>, ApiError> {
    let mode = MetaMode::from_media_query(&query, &app_id);
    mode.ensure_write_permission(&user, &app_id, &state).await?;
    let language = query.language.as_deref().unwrap_or("en");

    let txn = state.db.begin().await?;

    let existing_meta = mode
        .find_existing_meta(language, &txn)
        .await?
        .ok_or_else(|| ApiError::NotFound)?;

    let mut model: meta::ActiveModel = existing_meta.clone().into();
    model.updated_at = Set(chrono::Utc::now().naive_utc());

    match &query.item {
        MediaItem::Icon => {
            if existing_meta.icon.clone() == Some(media_id.clone()) {
                model.icon = Set(None);
            }
        }
        MediaItem::Thumbnail => {
            if existing_meta.thumbnail.clone() == Some(media_id.clone()) {
                model.thumbnail = Set(None);
            }
        }
        MediaItem::Preview => {
            let mut existing_preview = existing_meta.preview_media.clone().unwrap_or_default();
            existing_preview.retain(|id| id != &media_id);
            model.preview_media = Set(Some(existing_preview));
        }
    }

    model.update(&txn).await?;

    let item_name = format!("{}.webp", media_id);
    let master_store = state.master_credentials().await?;
    let master_store = master_store.to_store(false).await?;
    let path = FlowPath::from("media")
        .child(app_id)
        .child(item_name.clone());
    if let Err(e) = master_store.as_generic().delete(&path).await {
        tracing::error!("Failed to delete media file at {}: {:?}", path, e);
        return Err(ApiError::InternalError(
            anyhow!("Failed to delete media file, reference ID: {}", create_id()).into(),
        ));
    }
    txn.commit().await?;

    Ok(Json(()))
}
