use std::str::FromStr;

use crate::{error::ApiError, middleware::jwt::AppUser, state::AppState};
use axum::{Extension, Json, extract::State};
use flow_like_types::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct BillingSession {
    pub session_id: String,
    pub url: String,
}

#[tracing::instrument(name = "GET /user/billing", skip(state, user))]
pub async fn get_billing_session(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
) -> Result<Json<BillingSession>, ApiError> {
    let stripe = state
        .stripe_client
        .as_ref()
        .ok_or(anyhow!("Premium Feature is not enabled"))?;

    let user = user.get_user(&state).await?;
    let stripe_id = user
        .stripe_id
        .ok_or(anyhow!("User does not have a Stripe customer ID"))?;

    let session = stripe::BillingPortalSession::create(
        stripe,
        stripe::CreateBillingPortalSession::new(
            stripe::CustomerId::from_str(&stripe_id)
                .map_err(|_| anyhow!("Invalid Stripe customer ID".to_string()))?,
        ),
    )
    .await?;

    let billing_session = BillingSession {
        session_id: session.id.to_string(),
        url: session.url,
    };

    Ok(Json(billing_session))
}
