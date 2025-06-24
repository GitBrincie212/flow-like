use axum::{
    Router,
    routing::{get, post, put},
};

use crate::state::AppState;

pub mod assign_role;
pub mod delete_role;
pub mod get_roles;
pub mod make_role_default;
pub mod upsert_role;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_roles::get_roles))
        .route(
            "/{role_id}",
            put(upsert_role::upsert_role).delete(delete_role::delete_role),
        )
        .route(
            "/{role_id}/default",
            put(make_role_default::make_role_default),
        )
        .route("/{role_id}/assign/{sub}", post(assign_role::assign_role))
}
