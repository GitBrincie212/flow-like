pub mod imap;
pub mod smtp;

use flow_like::flow::node::NodeLogic;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

static AVAILABLE_FOOTER_PLAIN: [&str; 3] = [
    "sent via Flow-Like.com - Stop Doing. Start Flowing.",
    "sent via Flow-Like.com - Your Work. On Beast Mode.",
    "sent via Flow-Like.com - No Mercy for Manual Tasks.",
];

static AVAILABLE_FOOTER_HTML: [&str; 3] = [
    "<br><br>&mdash;<br><small>sent via <a href=\"https://flow-like.com\">Flow-Like.com</a - Stop Doing. Start Flowing.</small>",
    "<br><br>&mdash;<br><small>sent via <a href=\"https://flow-like.com\">Flow-Like.com</a - Your Work. On Beast Mode.</small>",
    "<br><br>&mdash;<br><small>sent via <a href=\"https://flow-like.com\">Flow-Like.com</a - No Mercy for Manual Tasks.</small>",
];

pub fn generate_mail_footer_html() -> String {
    let index = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        % AVAILABLE_FOOTER_HTML.len() as u64) as usize;
    AVAILABLE_FOOTER_HTML[index].to_string()
}

pub fn generate_mail_footer_plain() -> String {
    let index = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        % AVAILABLE_FOOTER_PLAIN.len() as u64) as usize;
    AVAILABLE_FOOTER_PLAIN[index].to_string()
}

pub async fn register_functions() -> Vec<Arc<dyn NodeLogic>> {
    let mut output = vec![
        Arc::new(imap::ImapConnectNode::default()) as Arc<dyn NodeLogic>,
        Arc::new(imap::inbox::list::ListMailsNode::default()) as Arc<dyn NodeLogic>,
    ];

    output.extend(imap::inbox::mail::register_functions().await);
    output.extend(imap::inbox::register_functions().await);

    output.push(Arc::new(smtp::SmtpConnectNode::default()));
    output.push(Arc::new(smtp::send_mail::SmtpSendMailNode::default()));

    output
}
