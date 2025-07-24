use std::collections::HashMap;

use crate::{
    entity::user, error::ApiError, middleware::jwt::AppUser, routes::user::sign_avatar,
    state::AppState, user_management::UserManagement,
};
use axum::{Extension, Json, extract::State};
use flow_like_types::anyhow;
use sea_orm::{ActiveModelTrait, EntityTrait, sqlx::types::chrono};

/// Sometimes when the user still has an old jwt, the user info is not updated correctly.
/// In these cases, we want to update the value correctly.
#[tracing::instrument(
    name = "Should update user attribute",
    skip(state, sub, attribute, value)
)]
async fn should_update(
    state: &AppState,
    sub: &str,
    username: &Option<String>,
    attribute: &str,
    value: &Option<String>,
) -> bool {
    let user_manager = UserManagement::new(&state).await;
    let actual_value = user_manager
        .get_attribute(&sub, &username, &attribute)
        .await;

    let mut should_update = true;

    if let Ok(Some(actual_value)) = actual_value {
        if Some(actual_value) == *value {
            should_update = false;
        }
    }
    should_update
}

#[tracing::instrument(name = "GET /user/info", skip(state, user))]
pub async fn user_info(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
) -> Result<Json<user::Model>, ApiError> {
    let sub = user.sub()?;
    let email = user.email().clone();
    let username = user.username().clone();
    let preferred_username = user.preferred_username().clone();
    let user_info = user::Entity::find_by_id(&sub).one(&state.db).await?;
    if let Some(mut user_info) = user_info {
        let mut updated_user: Option<user::ActiveModel> = None;
        if let Some(email) = &email {
            if user_info.email != Some(email.clone()) {
                if should_update(&state, &sub, &username, "email", &user_info.email).await {
                    let mut tmp_updated_user: user::ActiveModel = user_info.clone().into();
                    tmp_updated_user.email = sea_orm::ActiveValue::Set(Some(email.clone()));
                    updated_user = Some(tmp_updated_user);
                }
            }
        }

        if let Some(username) = &username {
            if user_info.username != Some(username.clone()) {
                let mut tmp_updated_user: user::ActiveModel =
                    updated_user.unwrap_or(user_info.clone().into());
                tmp_updated_user.username = sea_orm::ActiveValue::Set(Some(username.clone()));
                updated_user = Some(tmp_updated_user);
            }
        }

        if let Some(preferred_username) = &preferred_username {
            if user_info.preferred_username != Some(preferred_username.clone()) {
                if should_update(
                    &state,
                    &sub,
                    &username,
                    "preferred_username",
                    &user_info.preferred_username,
                )
                .await
                {
                    let mut tmp_updated_user: user::ActiveModel =
                        updated_user.unwrap_or(user_info.clone().into());
                    tmp_updated_user.preferred_username =
                        sea_orm::ActiveValue::Set(Some(preferred_username.clone()));
                    updated_user = Some(tmp_updated_user);
                }
            }
        }

        if user_info.stripe_id.is_none() && state.platform_config.features.premium {
            let stripe_customer = generate_stripe_user(&state, &sub, email.clone()).await?;
            let mut tmp_updated_user: user::ActiveModel =
                updated_user.unwrap_or(user_info.clone().into());
            tmp_updated_user.stripe_id =
                sea_orm::ActiveValue::Set(Some(stripe_customer.id.to_string()));
            updated_user = Some(tmp_updated_user);
        }

        if let Some(mut updated_user) = updated_user {
            updated_user.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().naive_utc());
            let new_user = updated_user.update(&state.db).await?;
            user_info = new_user;
        }

        if let Some(avatar) = &user_info.avatar {
            let signed_avatar_url = sign_avatar(&user_info.id, avatar, &state).await?;
            user_info.avatar = Some(signed_avatar_url);
        }

        return Ok(Json(user_info));
    }

    let stripe_customer = if state.platform_config.features.premium {
        Some(
            generate_stripe_user(&state, &sub, email.clone())
                .await?
                .id
                .to_string(),
        )
    } else {
        None
    };

    let user = user::ActiveModel {
        id: sea_orm::ActiveValue::Set(sub.clone()),
        email: sea_orm::ActiveValue::Set(email),
        stripe_id: sea_orm::ActiveValue::Set(stripe_customer),
        username: sea_orm::ActiveValue::Set(username),
        preferred_username: sea_orm::ActiveValue::Set(preferred_username),
        created_at: sea_orm::ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: sea_orm::ActiveValue::Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    };

    let new_user = user::Entity::insert(user)
        .exec_with_returning(&state.db)
        .await?;

    Ok(Json(new_user))
}

async fn generate_stripe_user(
    state: &AppState,
    sub: &str,
    email: Option<String>,
) -> flow_like_types::Result<stripe::Customer> {
    let stripe_client = state
        .stripe_client
        .as_ref()
        .ok_or(anyhow!("Premium Feature disabled"))?;
    let customer = stripe::Customer::create(
        stripe_client,
        stripe::CreateCustomer {
            metadata: Some(HashMap::from([
                ("sub".to_string(), sub.to_string()),
                ("platform".to_string(), "FlowLike".to_string()),
            ])),
            email: email.as_deref(),
            ..Default::default()
        },
    )
    .await?;

    Ok(customer)
}
