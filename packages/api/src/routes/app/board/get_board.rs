use crate::{
    ensure_permission, error::ApiError, middleware::jwt::AppUser,
    permission::role_permission::RolePermissions,
    routes::app::template::get_template::VersionQuery, state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use flow_like::flow::board::Board;
use flow_like_types::anyhow;

#[tracing::instrument(name = "GET /apps/{app_id}/board/{board_id}", skip(state, user))]
pub async fn get_board(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path((app_id, board_id)): Path<(String, String)>,
    Query(params): Query<VersionQuery>,
) -> Result<Json<Board>, ApiError> {
    let permission = ensure_permission!(user, &app_id, &state, RolePermissions::ReadBoards);
    let sub = permission.sub()?;

    let version_opt = if let Some(ver_str) = params.version {
        let parts = ver_str
            .split('_')
            .map(str::parse::<u32>)
            .collect::<Result<Vec<u32>, _>>()?;
        match parts.as_slice() {
            [maj, min, pat] => Some((*maj, *min, *pat)),
            _ => {
                return Err(ApiError::Internal(
                    anyhow!("version must be in MAJOR_MINOR_PATCH format").into(),
                ));
            }
        }
    } else {
        None
    };

    let board = state
        .scoped_board(&sub, &app_id, &board_id, &state, version_opt)
        .await?;

    Ok(Json(board))
}
