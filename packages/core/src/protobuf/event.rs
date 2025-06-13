use std::time::SystemTime;

use flow_like_types::{FromProto, Timestamp, ToProto};

use crate::flow::{
    event::{CanaryEvent, Event, ReleaseNotes},
    variable::Variable,
};

impl ToProto<flow_like_types::proto::Event> for Event {
    fn to_proto(&self) -> flow_like_types::proto::Event {
        flow_like_types::proto::Event {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            board_id: self.board_id.clone(),
            board_version: self.board_version.map(|v| flow_like_types::proto::Version {
                major: v.0,
                minor: v.1,
                patch: v.2,
            }),
            node_id: self.node_id.clone(),
            variables: self
                .variables
                .iter()
                .map(|(k, v)| (k.clone(), v.to_proto()))
                .collect(),
            config: self.config.clone(),
            active: self.active,
            canary: self.canary.as_ref().map(|c| c.to_proto()),
            notes: self.notes.as_ref().map(|n| match n {
                ReleaseNotes::NOTES(s) => {
                    flow_like_types::proto::event::Notes::ReleaseNotes(s.clone())
                }
                ReleaseNotes::URL(s) => {
                    flow_like_types::proto::event::Notes::ReleaseNotesUrl(s.clone())
                }
            }),
            event_version: Some(flow_like_types::proto::Version {
                major: self.event_version.0,
                minor: self.event_version.1,
                patch: self.event_version.2,
            }),
            created_at: Some(Timestamp::from(self.created_at)),
            updated_at: Some(Timestamp::from(self.updated_at)),
        }
    }
}

impl ToProto<flow_like_types::proto::Canary> for CanaryEvent {
    fn to_proto(&self) -> flow_like_types::proto::Canary {
        flow_like_types::proto::Canary {
            board_id: self.board_id.clone(),
            board_version: self.board_version.map(|v| flow_like_types::proto::Version {
                major: v.0,
                minor: v.1,
                patch: v.2,
            }),
            node_id: self.node_id.clone(),
            weight: self.weight,
            variables: self
                .variables
                .iter()
                .map(|(k, v)| (k.clone(), v.to_proto()))
                .collect(),
            created_at: Some(Timestamp::from(self.created_at)),
            updated_at: Some(Timestamp::from(self.updated_at)),
        }
    }
}

impl FromProto<flow_like_types::proto::Event> for Event {
    fn from_proto(proto: flow_like_types::proto::Event) -> Self {
        Event {
            id: proto.id,
            name: proto.name,
            description: proto.description,
            board_id: proto.board_id,
            board_version: proto.board_version.map(|v| (v.major, v.minor, v.patch)),
            node_id: proto.node_id,
            variables: proto
                .variables
                .into_iter()
                .map(|(k, v)| (k, Variable::from_proto(v)))
                .collect(),
            active: proto.active,
            config: proto.config,
            canary: proto.canary.map(CanaryEvent::from_proto),
            notes: proto.notes.map(|n| match n {
                flow_like_types::proto::event::Notes::ReleaseNotes(s) => ReleaseNotes::NOTES(s),
                flow_like_types::proto::event::Notes::ReleaseNotesUrl(s) => ReleaseNotes::URL(s),
            }),
            event_version: (
                proto.event_version.unwrap().major,
                proto.event_version.unwrap().minor,
                proto.event_version.unwrap().patch,
            ),
            created_at: proto
                .created_at
                .map(|t| SystemTime::try_from(t).unwrap_or(SystemTime::UNIX_EPOCH))
                .unwrap_or(SystemTime::UNIX_EPOCH),
            updated_at: proto
                .updated_at
                .map(|t| SystemTime::try_from(t).unwrap_or(SystemTime::UNIX_EPOCH))
                .unwrap_or(SystemTime::UNIX_EPOCH),
        }
    }
}

impl FromProto<flow_like_types::proto::Canary> for CanaryEvent {
    fn from_proto(proto: flow_like_types::proto::Canary) -> Self {
        CanaryEvent {
            board_id: proto.board_id,
            board_version: proto.board_version.map(|v| (v.major, v.minor, v.patch)),
            node_id: proto.node_id,
            weight: proto.weight,
            variables: proto
                .variables
                .into_iter()
                .map(|(k, v)| (k, Variable::from_proto(v)))
                .collect(),
            created_at: proto
                .created_at
                .map(|t| SystemTime::try_from(t).unwrap_or(SystemTime::UNIX_EPOCH))
                .unwrap_or(SystemTime::UNIX_EPOCH),
            updated_at: proto
                .updated_at
                .map(|t| SystemTime::try_from(t).unwrap_or(SystemTime::UNIX_EPOCH))
                .unwrap_or(SystemTime::UNIX_EPOCH),
        }
    }
}
