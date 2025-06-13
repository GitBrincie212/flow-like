use crate::app::{App, AppCategory, AppStatus, AppVisibility};
use flow_like_types::{FromProto, Timestamp, ToProto};
use std::time::SystemTime;

impl ToProto<flow_like_types::proto::App> for App {
    fn to_proto(&self) -> flow_like_types::proto::App {
        flow_like_types::proto::App {
            id: self.id.clone(),
            authors: self.authors.clone(),
            bits: self.bits.clone(),
            boards: self.boards.clone(),
            events: self.events.clone(),
            changelog: self.changelog.clone(),
            primary_category: self.primary_category.clone().map(|c| c.to_proto()),
            secondary_category: self.secondary_category.clone().map(|c| c.to_proto()),
            status: self.status.to_proto(),
            version: self.version.clone(),
            visibility: self.visibility.to_proto(),
            avg_rating: self.avg_rating.map(|v| v as f32),
            relevance_score: self.relevance_score.map(|v| v as f32),
            download_count: self.download_count as i64,
            interaction_count: self.interactions_count as i64,
            price: self.price.map(|p| p as i32),
            rating_count: self.rating_count as i64,
            rating_sum: self.rating_sum as i64,
            templates: self.templates.clone(),
            created_at: Some(Timestamp::from(self.created_at)),
            updated_at: Some(Timestamp::from(self.updated_at)),
            ..Default::default()
        }
    }
}

impl FromProto<flow_like_types::proto::App> for App {
    fn from_proto(proto: flow_like_types::proto::App) -> Self {
        App {
            id: proto.id,
            authors: proto.authors,
            bits: proto.bits,
            boards: proto.boards,
            events: proto.events,
            templates: proto.templates,
            changelog: proto.changelog,
            avg_rating: proto.avg_rating.map(|v| v as f64),
            relevance_score: proto.relevance_score.map(|v| v as f64),
            download_count: proto.download_count as u64,
            interactions_count: proto.interaction_count as u64,
            rating_count: proto.rating_count as u64,
            rating_sum: proto.rating_sum as u64,
            primary_category: proto.primary_category.map(AppCategory::from_proto),
            secondary_category: proto.secondary_category.map(AppCategory::from_proto),
            status: AppStatus::from_proto(proto.status),
            version: proto.version,
            visibility: AppVisibility::from_proto(proto.visibility),
            price: proto.price.map(|p| p as u32),
            created_at: proto
                .created_at
                .map(|t| SystemTime::try_from(t).unwrap_or(SystemTime::UNIX_EPOCH))
                .unwrap_or(SystemTime::UNIX_EPOCH),
            updated_at: proto
                .updated_at
                .map(|t| SystemTime::try_from(t).unwrap_or(SystemTime::UNIX_EPOCH))
                .unwrap_or(SystemTime::UNIX_EPOCH),
            app_state: None,
            frontend: None,
        }
    }
}

impl AppCategory {
    fn to_proto(&self) -> i32 {
        match self {
            AppCategory::Other => 0,
            AppCategory::Productivity => 1,
            AppCategory::Social => 2,
            AppCategory::Entertainment => 3,
            AppCategory::Education => 4,
            AppCategory::Health => 5,
            AppCategory::Finance => 6,
            AppCategory::Lifestyle => 7,
            AppCategory::Travel => 8,
            AppCategory::News => 9,
            AppCategory::Sports => 10,
            AppCategory::Shopping => 11,
            AppCategory::FoodAndDrink => 12,
            AppCategory::Music => 13,
            AppCategory::Photography => 14,
            AppCategory::Utilities => 15,
            AppCategory::Weather => 16,
            AppCategory::Games => 17,
            AppCategory::Business => 18,
            AppCategory::Communication => 19,
            AppCategory::Anime => 20,
        }
    }

    fn from_proto(value: i32) -> Self {
        match value {
            0 => AppCategory::Other,
            1 => AppCategory::Productivity,
            2 => AppCategory::Social,
            3 => AppCategory::Entertainment,
            4 => AppCategory::Education,
            5 => AppCategory::Health,
            6 => AppCategory::Finance,
            7 => AppCategory::Lifestyle,
            8 => AppCategory::Travel,
            9 => AppCategory::News,
            10 => AppCategory::Sports,
            11 => AppCategory::Shopping,
            12 => AppCategory::FoodAndDrink,
            13 => AppCategory::Music,
            14 => AppCategory::Photography,
            15 => AppCategory::Utilities,
            16 => AppCategory::Weather,
            17 => AppCategory::Games,
            18 => AppCategory::Business,
            19 => AppCategory::Communication,
            20 => AppCategory::Anime,
            _ => AppCategory::Other, // Default
        }
    }
}

impl AppStatus {
    fn to_proto(&self) -> i32 {
        match self {
            AppStatus::Active => 0,
            AppStatus::Inactive => 1,
            AppStatus::Archived => 2,
        }
    }

    fn from_proto(value: i32) -> Self {
        match value {
            0 => AppStatus::Active,
            1 => AppStatus::Inactive,
            2 => AppStatus::Archived,
            _ => AppStatus::Active, // Default
        }
    }
}

impl AppVisibility {
    fn to_proto(&self) -> i32 {
        match self {
            AppVisibility::Public => 0,
            AppVisibility::PublicRequestAccess => 1,
            AppVisibility::Private => 2,
            AppVisibility::Prototype => 3,
            AppVisibility::Offline => 4,
        }
    }

    fn from_proto(value: i32) -> Self {
        match value {
            0 => AppVisibility::Public,
            1 => AppVisibility::PublicRequestAccess,
            2 => AppVisibility::Private,
            3 => AppVisibility::Prototype,
            4 => AppVisibility::Offline,
            _ => AppVisibility::Public, // Default
        }
    }
}
