use crate::{
    ensure_permission, error::ApiError, middleware::jwt::AppUser,
    permission::role_permission::RolePermissions, state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use flow_like::flow::board::Board;

#[tracing::instrument(name = "GET /apps/{app_id}/board", skip(state, user))]
pub async fn get_boards(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path(app_id): Path<String>,
) -> Result<Json<Vec<Board>>, ApiError> {
    let permission = ensure_permission!(user, &app_id, &state, RolePermissions::ReadBoards);
    let sub = permission.sub()?;

    let mut boards = vec![];

    let app = state.scoped_app(&sub, &app_id, &state).await?;
    for board_id in app.boards.iter() {
        let board = app.open_board(board_id.clone(), Some(false), None).await;
        if let Ok(board) = board {
            boards.push(board.lock().await.clone());
        }
    }

    Ok(Json(boards))
}
