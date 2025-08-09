use flow_like::{
    flow::{
        execution::context::ExecutionContext,
        node::{Node, NodeLogic},
        pin::PinOptions,
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{Cacheable, anyhow, async_trait, json::json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    any::Any,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
};
use tokio::{self, net::TcpStream, sync::Mutex};
pub mod inbox;

#[derive(Serialize, Deserialize, JsonSchema, Clone)]
pub struct ImapConnection {
    pub id: String,
}

impl ImapConnection {
    pub fn new(id: String) -> Self {
        ImapConnection { id }
    }

    pub async fn from_context(
        &self,
        context: &mut ExecutionContext,
    ) -> flow_like_types::Result<ImapSession> {
        let cache_key = format!("imap_session_{}", self.id);
        if let Some(session) = context.get_cache(&cache_key).await {
            let session = session
                .as_any()
                .downcast_ref::<ImapSessionCache>()
                .ok_or_else(|| flow_like_types::anyhow!("Failed to downcast ImapSessionCache"))?
                .session
                .clone();
            Ok(session)
        } else {
            Err(flow_like_types::anyhow!("IMAP session not found"))
        }
    }
}

pub type ImapSession = Arc<Mutex<async_imap::Session<async_native_tls::TlsStream<TcpStream>>>>;

pub struct ImapSessionCache {
    pub session: ImapSession,
}

impl Cacheable for ImapSessionCache {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Default)]
pub struct ImapConnectNode;

impl ImapConnectNode {
    pub fn new() -> Self {
        ImapConnectNode
    }
}

fn tls() -> async_native_tls::TlsConnector {
    async_native_tls::TlsConnector::new()
        .danger_accept_invalid_hostnames(true)
        .danger_accept_invalid_certs(true)
}

#[async_trait]
impl NodeLogic for ImapConnectNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "email_imap_connect",
            "IMAP Connect",
            "Connects to an IMAP server and caches the session",
            "Email/IMAP",
        );
        node.add_icon("/flow/icons/mail.svg");

        node.add_input_pin("exec_in", "In", "Execution input", VariableType::Execution);
        node.add_input_pin("host", "Host", "IMAP server hostname", VariableType::String)
            .set_default_value(Some(json!("imap.example.com")));
        node.add_input_pin("port", "Port", "IMAP server port", VariableType::Integer)
            .set_default_value(Some(json!(993)));
        node.add_input_pin(
            "username",
            "Username",
            "Email account username",
            VariableType::String,
        );
        node.add_input_pin(
            "password",
            "Password",
            "Email account password",
            VariableType::String,
        )
        .set_options(PinOptions::new().set_sensitive(true).build());
        node.add_input_pin(
            "encryption",
            "Encryption",
            "Connection encryption: Tls, StartTls, or Plain",
            VariableType::String,
        )
        .set_default_value(Some(json!("Tls")))
        .set_options(
            PinOptions::new()
                .set_valid_values(vec!["Tls".into(), "StartTls".into(), "Plain".into()])
                .build(),
        );

        node.add_output_pin(
            "exec_out",
            "Out",
            "Execution output",
            VariableType::Execution,
        );
        node.add_output_pin(
            "connection",
            "Connection",
            "Cached IMAP connection reference",
            VariableType::Struct,
        )
        .set_schema::<ImapConnection>();

        node
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        context.deactivate_exec_pin("exec_out").await?;

        let host: String = context.evaluate_pin("host").await?;
        let port_f: f64 = context.evaluate_pin("port").await?;
        let port: u16 = port_f as u16;
        let username: String = context.evaluate_pin("username").await?;
        let password: String = context.evaluate_pin("password").await?;
        let encryption: String = context.evaluate_pin("encryption").await?;

        let mut hasher = DefaultHasher::new();
        host.hash(&mut hasher);
        port.hash(&mut hasher);
        username.hash(&mut hasher);
        password.hash(&mut hasher);
        let id = hasher.finish().to_string();
        let cache_key = format!("imap_session_{}", id);

        {
            let cache = context.cache.read().await;
            if cache.contains_key(&cache_key) {
                context
                    .set_pin_value("connection", json!(ImapConnection { id: id.clone() }))
                    .await?;
                context.activate_exec_pin("exec_out").await?;
                return Ok(());
            }
        }

        let imap_addr: (&str, u16) = (&host, port);

        let client = match encryption.as_str() {
            "Tls" => {
                // Implicit SSL/TLS from the start
                let tcp: TcpStream = TcpStream::connect(imap_addr).await?;
                let tls = async_native_tls::TlsConnector::new();
                let stream = tls.connect(&host, tcp).await?;
                async_imap::Client::new(stream)
            }
            "StartTls" => {
                // Plain TCP first, then upgrade via STARTTLS
                let tcp = TcpStream::connect(imap_addr).await?;
                let mut client = async_imap::Client::new(tcp);
                let tls = tls();
                client.run_command_and_check_ok("STARTTLS", None).await?;
                let stream = client.into_inner();
                let tls_stream = tls.connect(&host, stream).await?;
                async_imap::Client::new(tls_stream)
            }
            "Plain" => {
                return Err(flow_like_types::anyhow!(
                    "Plain Connection is not supported. Use Tls or StartTls instead.",
                ));
            }
            other => {
                return Err(flow_like_types::anyhow!(
                    "Unsupported encryption mode: {} (valid: Tls, StartTls, Plain)",
                    other
                ));
            }
        };
        context.log_message(
            &format!("-- connected to {}:{}", imap_addr.0, &imap_addr.1),
            flow_like::flow::execution::LogLevel::Debug,
        );

        let imap_session = client
            .login(&username, password)
            .await
            .map_err(|(e, _)| anyhow!(e))?;
        let imap_session = Arc::new(Mutex::new(imap_session));

        context.log_message(
            &format!("-- logged in as {}", &username),
            flow_like::flow::execution::LogLevel::Debug,
        );

        let cache_obj = ImapSessionCache {
            session: imap_session,
        };
        context
            .cache
            .write()
            .await
            .insert(cache_key, Arc::new(cache_obj));

        context
            .set_pin_value("connection", json!(ImapConnection { id: id.clone() }))
            .await?;
        context.activate_exec_pin("exec_out").await?;
        Ok(())
    }
}
