use crate::{
    ensure_permission, error::ApiError, middleware::jwt::AppUser,
    permission::role_permission::RolePermissions, state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};

#[tracing::instrument(
    name = "GET /apps/{app_id}/board/{board_id}/version",
    skip(state, user)
)]
pub async fn get_board_versions(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path((app_id, board_id)): Path<(String, String)>,
) -> Result<Json<Vec<(u32, u32, u32)>>, ApiError> {
    let permission = ensure_permission!(user, &app_id, &state, RolePermissions::ReadBoards);
    let sub = permission.sub()?;

    let board = state
        .master_board(&sub, &app_id, &board_id, &state, None)
        .await?;
    let versions = board.get_versions(None).await?;

    Ok(Json(versions))
}
