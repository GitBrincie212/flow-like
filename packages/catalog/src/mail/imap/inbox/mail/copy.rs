use flow_like::{
    flow::{
        execution::{LogLevel, context::ExecutionContext},
        node::{Node, NodeLogic},
        pin::PinOptions,
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{async_trait, json::json};

use crate::mail::imap::inbox::list::EmailRef;

#[derive(Default)]
pub struct ImapCopyMailNode;

impl ImapCopyMailNode {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NodeLogic for ImapCopyMailNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "email_imap_copy_message",
            "Copy Mail to Mailbox",
            "Copies a mail (by UID) to another IMAP mailbox",
            "Email/IMAP",
        );
        node.add_icon("/flow/icons/mail.svg");

        node.add_input_pin("exec_in", "In", "Trigger", VariableType::Execution);

        node.add_input_pin(
            "email",
            "Email",
            "EmailRef containing connection, inbox, uid",
            VariableType::Struct,
        )
        .set_schema::<EmailRef>()
        .set_options(PinOptions::new().set_enforce_schema(true).build());

        node.add_input_pin(
            "destination",
            "Destination Mailbox",
            "Target mailbox (e.g. Archive)",
            VariableType::String,
        )
        .set_default_value(Some(json!("Archive")));

        node.add_input_pin(
            "create_if_missing",
            "Create If Missing",
            "Create the destination mailbox if it doesn't exist",
            VariableType::Boolean,
        )
        .set_default_value(Some(json!(false)));

        node.add_output_pin(
            "new_message_ref",
            "New Message Ref",
            "Reference to the copied message",
            VariableType::Struct,
        )
        .set_schema::<EmailRef>();
        node.add_output_pin(
            "exec_out",
            "Out",
            "Execution output",
            VariableType::Execution,
        );

        node
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        context.deactivate_exec_pin("exec_out").await?;

        let mut email: EmailRef = context.evaluate_pin("email").await?;
        let destination: String = context.evaluate_pin("destination").await?;
        let create_if_missing: bool = context.evaluate_pin("create_if_missing").await?;

        let mut cached_session = email.connection.to_session_cache(context).await?;

        let source_mailbox = email.inbox.name.clone();
        cached_session
            .session
            .lock()
            .await
            .select(&source_mailbox)
            .await
            .map_err(|e| {
                flow_like_types::anyhow!(
                    "Failed to select source mailbox '{}': {}",
                    source_mailbox,
                    e
                )
            })?;

        if create_if_missing {
            if !cached_session.mailbox_exists(&destination).await? {
                context.log_message(
                    &format!("Destination '{}' missing → creating", destination),
                    LogLevel::Debug,
                );
                cached_session
                    .create_mailbox(&destination)
                    .await
                    .map_err(|e| {
                        flow_like_types::anyhow!(
                            "Failed to create mailbox '{}': {}",
                            destination,
                            e
                        )
                    })?;
            }
        }

        let uid = email.uid.clone().to_string();

        cached_session
            .session
            .lock()
            .await
            .uid_copy(&uid, &destination)
            .await
            .map_err(|e| flow_like_types::anyhow!("UID COPY to '{}' failed: {}", destination, e))?;

        context.log_message(
            &format!(
                "Copied UID {} from '{}' to '{}'",
                email.uid, source_mailbox, destination
            ),
            LogLevel::Debug,
        );

        email.inbox.name = destination.clone();
        context
            .set_pin_value("new_message_ref", json!(email))
            .await?;
        context.activate_exec_pin("exec_out").await?;
        Ok(())
    }
}
