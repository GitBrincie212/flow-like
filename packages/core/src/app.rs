use crate::{
    bit::Metadata,
    flow::board::{Board, VersionType},
    state::FlowLikeState,
    utils::compression::{compress_to_file, from_compressed},
};
use flow_like_storage::Path;
use flow_like_types::{FromProto, ToProto, create_id, proto, sync::Mutex};
use futures::{StreamExt, TryStreamExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::SystemTime, vec};

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub enum StandardInterfaces {
    Chat,
    Search,
    Form,
    List,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct FrontendConfiguration {
    pub landing_page: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum AppCategory {
    Other = 0,
    Productivity = 1,
    Social = 2,
    Entertainment = 3,
    Education = 4,
    Health = 5,
    Finance = 6,
    Lifestyle = 7,
    Travel = 8,
    News = 9,
    Sports = 10,
    Shopping = 11,
    FoodAndDrink = 12,
    Music = 13,
    Photography = 14,
    Utilities = 15,
    Weather = 16,
    Games = 17,
    Business = 18,
    Communication = 19,
    Anime = 20,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub enum AppStatus {
    Active = 0,
    Inactive = 1,
    Archived = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum AppVisibility {
    Public = 0,
    PublicRequestAccess = 1,
    Private = 2,
    Prototype = 3,
    Offline = 4,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub enum AppSearchSort {
    BestRated,
    WorstRated,
    MostPopular,
    LeastPopular,
    MostRelevant,
    LeastRelevant,
    NewestCreated,
    OldestCreated,
    NewestUpdated,
    OldestUpdated,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct AppSearchQuery {
    pub search: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub categories: Option<Vec<AppCategory>>,
    pub authors: Option<Vec<String>>,
    pub sort: Option<AppSearchSort>,
    pub tag: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct App {
    pub id: String,

    pub status: AppStatus,
    pub visibility: AppVisibility,

    pub authors: Vec<String>,
    pub bits: Vec<String>,
    pub boards: Vec<String>,
    pub releases: Vec<String>,
    pub templates: Vec<String>,

    pub changelog: Option<String>,

    pub primary_category: Option<AppCategory>,
    pub secondary_category: Option<AppCategory>,

    pub rating_sum: u64,
    pub rating_count: u64,
    pub download_count: u64,
    pub interactions_count: u64,

    pub avg_rating: Option<f64>,
    pub relevance_score: Option<f64>,

    pub updated_at: SystemTime,
    pub created_at: SystemTime,

    pub version: Option<String>,

    pub frontend: Option<FrontendConfiguration>,

    pub price: Option<u32>,

    #[serde(skip)]
    pub app_state: Option<Arc<Mutex<FlowLikeState>>>,
}

impl Clone for App {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            status: self.status.clone(),
            visibility: self.visibility.clone(),
            authors: self.authors.clone(),
            boards: self.boards.clone(),
            templates: self.templates.clone(),
            bits: self.bits.clone(),
            releases: self.releases.clone(),
            changelog: self.changelog.clone(),
            avg_rating: self.avg_rating,
            download_count: self.download_count,
            interactions_count: self.interactions_count,
            rating_count: self.rating_count,
            rating_sum: self.rating_sum,
            relevance_score: self.relevance_score,
            primary_category: self.primary_category.clone(),
            secondary_category: self.secondary_category.clone(),
            updated_at: self.updated_at,
            created_at: self.created_at,
            version: self.version.clone(),
            price: self.price,
            app_state: self.app_state.clone(),
            frontend: self.frontend.clone(),
        }
    }
}

impl App {
    pub async fn new(
        id: Option<String>,
        meta: Metadata,
        bits: Vec<String>,
        app_state: Arc<Mutex<FlowLikeState>>,
    ) -> flow_like_types::Result<Self> {
        let id = id.unwrap_or(create_id());

        App::push_meta(id.clone(), meta, app_state.clone(), None, None).await?;

        let item = Self {
            id,
            authors: vec![],
            bits,
            boards: vec![],
            releases: vec![],
            templates: vec![],
            updated_at: SystemTime::now(),
            created_at: SystemTime::now(),
            version: None,
            status: AppStatus::Active,
            visibility: AppVisibility::Offline,
            changelog: None,
            avg_rating: None,
            download_count: 0,
            interactions_count: 0,
            rating_count: 0,
            rating_sum: 0,
            relevance_score: None,

            primary_category: None,
            secondary_category: None,

            price: None,

            frontend: None,
            app_state: Some(app_state.clone()),
        };

        Ok(item)
    }

    pub async fn load(
        id: String,
        app_state: Arc<Mutex<FlowLikeState>>,
    ) -> flow_like_types::Result<Self> {
        let storage_root = Path::from("apps").child(id.clone());

        let store = FlowLikeState::project_meta_store(&app_state)
            .await?
            .as_generic();

        let vault: flow_like_types::proto::App =
            from_compressed(store, storage_root.child("manifest.app")).await?;
        let mut vault = App::from_proto(vault);
        vault.app_state = Some(app_state.clone());

        Ok(vault)
    }

    pub fn calculate_relevance_score(&mut self) -> f64 {
        let downloads = self.download_count as f64;
        let sum_ratings = self.rating_sum as f64;
        let rating_count = self.rating_count as f64;
        let interactions = self.interactions_count as f64;
        let avg_rating = sum_ratings / rating_count;
        self.avg_rating = Some(avg_rating);
        let relevance =
            (downloads * 2.0 + interactions) * (1.0 + avg_rating / 5.0) * (rating_count.ln() + 1.0);
        self.relevance_score = Some(relevance);
        relevance
    }

    pub async fn get_meta(
        id: String,
        app_state: Arc<Mutex<FlowLikeState>>,
        language: Option<String>,
        template_id: Option<String>,
    ) -> flow_like_types::Result<Metadata> {
        let store = FlowLikeState::project_storage_store(&app_state)
            .await?
            .as_generic();

        let mut metadata_path = Path::from("apps").child(id).child("metadata");
        if let Some(template_id) = template_id {
            metadata_path = metadata_path.child("templates").child(template_id);
        }
        let languages = [
            language.unwrap_or_else(|| "en".to_string()),
            "en".to_string(),
        ];

        // Try requested language first, then fallback to English
        for lang in languages
            .iter()
            .take_while(|&l| l != &languages[1] || l == &languages[0])
        {
            let meta_path = metadata_path.child(format!("{}.meta", lang));

            if let Ok(metadata) = from_compressed::<proto::Metadata>(store.clone(), meta_path).await
            {
                return Ok(Metadata::from_proto(metadata));
            }
        }

        Err(flow_like_types::anyhow!(
            "No metadata found for app {}",
            metadata_path
        ))
    }

    pub async fn push_meta(
        id: String,
        metadata: Metadata,
        app_state: Arc<Mutex<FlowLikeState>>,
        language: Option<String>,
        template_id: Option<String>,
    ) -> flow_like_types::Result<()> {
        let store = FlowLikeState::project_storage_store(&app_state)
            .await?
            .as_generic();

        let language = language.unwrap_or_else(|| "en".to_string());
        let mut meta_path = Path::from("apps").child(id).child("metadata");

        if let Some(template_id) = template_id {
            meta_path = meta_path.child("templates").child(template_id);
        }

        let meta_path = meta_path.child(format!("{}.meta", language));

        let proto_metadata = metadata.to_proto();
        compress_to_file(store, meta_path, &proto_metadata).await?;

        Ok(())
    }

    pub async fn create_board(&mut self, id: Option<String>) -> flow_like_types::Result<String> {
        let storage_root = Path::from("apps").child(self.id.clone());
        let state = self
            .app_state
            .clone()
            .ok_or(flow_like_types::anyhow!("App state not found"))?;
        let board = Board::new(id, storage_root, state);
        board.save(None).await?;
        self.boards.push(board.id.clone());
        self.updated_at = SystemTime::now();
        Ok(board.id)
    }

    pub async fn create_template_version(
        &mut self,
        template_id: Option<String>,
        version_type: VersionType,
        board_id: String,
        board_version: Option<(u32, u32, u32)>,
    ) -> flow_like_types::Result<(String, (u32, u32, u32))> {
        let template_id = template_id.unwrap_or(create_id());
        let new_template: Arc<Mutex<Board>> = self
            .open_board(board_id, Some(false), board_version)
            .await?;
        let old_template = self.open_template(template_id.clone(), None).await.ok();

        let template: (u32, u32, u32) = new_template
            .lock()
            .await
            .create_template(template_id.clone(), version_type, old_template, None)
            .await?;

        if !self.templates.contains(&template_id) {
            self.templates.push(template_id.clone());
        }

        self.updated_at = SystemTime::now();
        self.save().await?;

        Ok((template_id, template))
    }

    pub async fn boards_configured(&self) -> bool {
        for board_id in &self.boards {
            let board = self.open_board(board_id.clone(), Some(false), None).await;
            if let Ok(board) = board {
                let vars = board
                    .lock()
                    .await
                    .variables
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                for var in vars {
                    if var.default_value.is_none() {
                        return false;
                    }
                }
            }
        }

        true
    }

    pub async fn open_board(
        &self,
        board_id: String,
        register: Option<bool>,
        version: Option<(u32, u32, u32)>,
    ) -> flow_like_types::Result<Arc<Mutex<Board>>> {
        let storage_root = Path::from("apps").child(self.id.clone());
        if let Some(app_state) = &self.app_state {
            let board = app_state.lock().await.get_board(&board_id, version);

            if let Ok(board) = board {
                return Ok(board);
            }
        }

        let state = self
            .app_state
            .clone()
            .ok_or(flow_like_types::anyhow!("App state not found"))?;

        let board = Board::load(storage_root, &board_id, state, version).await?;
        let board_ref = Arc::new(Mutex::new(board));
        let register = register.unwrap_or(false);
        if register {
            if let Some(app_state) = &self.app_state {
                app_state
                    .lock()
                    .await
                    .register_board(&board_id, board_ref.clone(), version)?;
            }
        }

        Ok(board_ref)
    }

    pub async fn open_template(
        &self,
        template_id: String,
        version: Option<(u32, u32, u32)>,
    ) -> flow_like_types::Result<Board> {
        let storage_root = Path::from("apps").child(self.id.clone());

        let state = self
            .app_state
            .clone()
            .ok_or(flow_like_types::anyhow!("App state not found"))?;

        let template = Board::load_template(storage_root, &template_id, state, version).await?;

        Ok(template)
    }

    pub async fn delete_board(&mut self, board_id: &str) -> flow_like_types::Result<()> {
        self.boards.retain(|b| b != board_id);
        let board_dir = Path::from("apps")
            .child(self.id.clone())
            .child(format!("{}.board", board_id));

        let state = self
            .app_state
            .clone()
            .ok_or(flow_like_types::anyhow!("App state not found"))?;
        let store = FlowLikeState::project_meta_store(&state)
            .await?
            .as_generic();
        store.delete(&board_dir).await?;

        if let Some(app_state) = &self.app_state {
            app_state.lock().await.remove_board(board_id)?;
        }

        // Remove all versions of the board
        let versions_path = Path::from("apps")
            .child(self.id.clone())
            .child("versions")
            .child(board_id);
        let locations = store
            .list(Some(&versions_path))
            .map_ok(|m| m.location)
            .boxed();

        store
            .delete_stream(locations)
            .try_collect::<Vec<Path>>()
            .await?;
        self.updated_at = SystemTime::now();
        self.save().await?;
        Ok(())
    }

    pub async fn delete_template(&mut self, template_id: &str) -> flow_like_types::Result<()> {
        self.templates.retain(|b| b != template_id);
        let template_dir = Path::from("apps")
            .child(self.id.clone())
            .child(format!("{}.template", template_id));

        let state = self
            .app_state
            .clone()
            .ok_or(flow_like_types::anyhow!("App state not found"))?;
        let store = FlowLikeState::project_meta_store(&state)
            .await?
            .as_generic();
        store.delete(&template_dir).await?;

        // Remove all versions of the board
        let versions_path = Path::from("apps")
            .child(self.id.clone())
            .child("templates")
            .child("versions")
            .child(template_id);
        let locations = store
            .list(Some(&versions_path))
            .map_ok(|m| m.location)
            .boxed();

        store
            .delete_stream(locations)
            .try_collect::<Vec<Path>>()
            .await?;
        self.updated_at = SystemTime::now();
        self.save().await?;
        Ok(())
    }

    pub async fn save(&self) -> flow_like_types::Result<()> {
        if let Some(app_state) = &self.app_state {
            let store = FlowLikeState::project_meta_store(app_state)
                .await?
                .as_generic();

            let board_refs = {
                let guard = app_state.lock().await;
                let mut refs = Vec::with_capacity(self.boards.len());

                for board_id in &self.boards {
                    if let Ok(board) = guard.get_board(board_id, None) {
                        refs.push(board.clone());
                    }
                }
                refs
            };

            for board in board_refs {
                let tmp = board.lock().await.clone();
                tmp.save(Some(store.clone())).await?;
            }
        }

        let store = self
            .app_state
            .clone()
            .ok_or(flow_like_types::anyhow!("App state not found"))?;
        let store = FlowLikeState::project_meta_store(&store)
            .await?
            .as_generic();

        let manifest_path = Path::from("apps")
            .child(self.id.clone())
            .child("manifest.app");

        let proto_app = self.to_proto();
        compress_to_file(store, manifest_path, &proto_app).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{state::FlowLikeConfig, utils::http::HTTPClient};
    use flow_like_storage::files::store::FlowLikeStore;
    use flow_like_types::sync::Mutex;
    use flow_like_types::{FromProto, ToProto};
    use flow_like_types::{Message, tokio};
    use std::sync::Arc;

    async fn flow_state() -> Arc<Mutex<crate::state::FlowLikeState>> {
        let mut config: FlowLikeConfig = FlowLikeConfig::new();
        config.register_app_meta_store(FlowLikeStore::Other(Arc::new(
            flow_like_storage::object_store::memory::InMemory::new(),
        )));
        let (http_client, _refetch_rx) = HTTPClient::new();
        let flow_like_state = crate::state::FlowLikeState::new(config, http_client);
        Arc::new(Mutex::new(flow_like_state))
    }

    #[tokio::test]
    async fn serialize_app() {
        let app = crate::app::App {
            id: "id".to_string(),
            authors: vec!["author1".to_string(), "author2".to_string()],
            boards: vec!["board1".to_string(), "board2".to_string()],
            bits: vec!["bit1".to_string(), "bit2".to_string()],
            releases: vec!["release1".to_string(), "release2".to_string()],
            templates: vec!["template1".to_string(), "template2".to_string()],
            updated_at: std::time::SystemTime::now(),
            created_at: std::time::SystemTime::now(),
            status: crate::app::AppStatus::Active,
            visibility: crate::app::AppVisibility::Public,
            changelog: Some("Changelog text".to_string()),
            primary_category: Some(crate::app::AppCategory::Productivity),
            secondary_category: Some(crate::app::AppCategory::Education),
            app_state: Some(flow_state().await),
            version: Some("1.0.0".to_string()),
            avg_rating: Some(4.5),
            download_count: 1000,
            interactions_count: 500,
            price: Some(9),
            rating_count: 200,
            rating_sum: 800,
            relevance_score: Some(0.9),
            frontend: None,
        };

        let mut buf = Vec::new();
        app.to_proto().encode(&mut buf).unwrap();
        let deser = super::App::from_proto(flow_like_types::proto::App::decode(&buf[..]).unwrap());

        assert_eq!(app.id, deser.id);
    }
}
