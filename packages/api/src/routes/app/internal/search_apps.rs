use crate::{
    entity::{
        app, meta,
        sea_orm_active_enums::{Category, Visibility},
    },
    error::ApiError,
    middleware::jwt::AppUser,
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Query, State},
};
use flow_like::{
    app::{App, AppSearchQuery, AppSearchSort},
    bit::Metadata,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
#[tracing::instrument(name = "GET /apps/search", skip(state, user))]

pub async fn search_apps(
    State(state): State<AppState>,
    Extension(user): Extension<AppUser>,
    Query(query): Query<AppSearchQuery>,
) -> Result<Json<Vec<(App, Option<Metadata>)>>, ApiError> {
    if !state.platform_config.features.unauthorized_read {
        user.sub()?;
    }
    let language = query.language.clone().unwrap_or_else(|| "en".to_string());

    let cache_key = format!("search_apps:{:?}:{}", query, language);

    if let Some(cached) = state.get_cache(&cache_key) {
        return Ok(Json(cached));
    }

    let sort = query.sort.unwrap_or(AppSearchSort::MostRelevant);

    let limit = std::cmp::min(query.limit.unwrap_or(50), 100);
    let mut qb = app::Entity::find()
        .filter(
            app::Column::Visibility
                .eq(Visibility::Public)
                .or(app::Column::Visibility.eq(Visibility::PublicRequestAccess)),
        )
        .limit(Some(limit))
        .offset(query.offset);

    match sort {
        AppSearchSort::BestRated => qb = qb.order_by_desc(app::Column::AvgRating),
        AppSearchSort::WorstRated => qb = qb.order_by_asc(app::Column::AvgRating),
        AppSearchSort::MostPopular => qb = qb.order_by_desc(app::Column::RatingSum),
        AppSearchSort::LeastPopular => qb = qb.order_by_asc(app::Column::RatingSum),
        AppSearchSort::MostRelevant => qb = qb.order_by_desc(app::Column::RelevanceScore),
        AppSearchSort::LeastRelevant => qb = qb.order_by_asc(app::Column::RelevanceScore),
        AppSearchSort::NewestCreated => qb = qb.order_by_desc(app::Column::CreatedAt),
        AppSearchSort::OldestCreated => qb = qb.order_by_asc(app::Column::CreatedAt),
        AppSearchSort::NewestUpdated => qb = qb.order_by_desc(app::Column::UpdatedAt),
        AppSearchSort::OldestUpdated => qb = qb.order_by_asc(app::Column::UpdatedAt),
    }

    if let Some(category) = query.category {
        let category: Category = category.into();
        qb = qb.filter(
            app::Column::PrimaryCategory
                .eq(category.clone())
                .or(app::Column::SecondaryCategory.eq(category)),
        );
    }

    if let Some(search_str) = query.query {
        qb = qb.filter(
            meta::Column::Description
                .contains(&search_str)
                .or(meta::Column::Name.contains(&search_str)),
        )
    }

    if let Some(id) = query.id {
        qb = qb.filter(app::Column::Id.eq(&id));
    }

    if let Some(tag) = query.tag {
        qb = qb.filter(meta::Column::Tags.contains(&tag));
    }

    qb = qb.filter(
        meta::Column::Lang
            .is_null()
            .or(meta::Column::Lang.eq(&language))
            .or(meta::Column::Lang.eq("en")),
    );

    let models = qb
        .find_with_related(meta::Entity)
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let master_store = state.master_credentials().await?;
    let store = master_store.to_store(false).await?;

    let mut apps = Vec::new();

    for (app_model, meta_models) in models {
        let metadata = if let Some(meta) = meta_models
            .iter()
            .find(|meta| meta.lang == language)
            .or_else(|| meta_models.first())
        {
            let mut metadata = Metadata::from(meta.clone());
            let prefix = flow_like_storage::Path::from("media")
                .child("apps")
                .child(app_model.id.clone());
            metadata.presign(prefix, &store).await;
            Some(metadata)
        } else {
            None
        };

        apps.push((App::from(app_model), metadata));
    }

    state.set_cache(cache_key, &apps);

    Ok(Json(apps))
}
