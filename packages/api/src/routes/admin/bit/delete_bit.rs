use crate::{
    entity::bit, error::ApiError, middleware::jwt::AppUser,
    permission::global_permission::GlobalPermission, state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use flow_like::bit::Bit;
use sea_orm::EntityTrait;

#[tracing::instrument(name = "DELETE admin//bit/{bit_id}", skip(state, user))]
pub async fn delete_bit(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Path(bit_id): Path<String>,
) -> Result<Json<Vec<Bit>>, ApiError> {
    user.check_global_permission(&state, GlobalPermission::WriteBits)
        .await?;

    let cdn_bucket = state.cdn_bucket.clone();
    let deleted_bits = bit::Entity::delete_by_id(&bit_id)
        .exec_with_returning(&state.db)
        .await?;

    let mut bits = Vec::with_capacity(deleted_bits.len());
    for bit in deleted_bits {
        let bit: Bit = bit.into();
        if !bit.hash.is_empty() {
            let path =
                flow_like_storage::object_store::path::Path::from("bits").child(bit.hash.clone());
            cdn_bucket.as_generic().delete(&path).await?;
        }
        bits.push(bit);
    }

    Ok(Json(bits))
}
