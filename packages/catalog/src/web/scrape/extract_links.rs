use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

use flow_like::{
    flow::{
        execution::context::ExecutionContext,
        node::{Node, NodeLogic},
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{
    async_trait,
    json::json,
    reqwest::{self, Url},
    tokio,
};
use scraper::{Html, Selector};

#[derive(Default)]
pub struct ExtractLinksNode {}

impl ExtractLinksNode {
    pub fn new() -> Self {
        ExtractLinksNode {}
    }
}

#[async_trait]
impl NodeLogic for ExtractLinksNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "web_scrape_extract_links",
            "Extract Links",
            "Extracts links from the input text",
            "Web/Scraping",
        );
        node.add_icon("/flow/icons/spider-web.svg");

        node.add_input_pin("exec_in", "Input", "Trigger Pin", VariableType::Execution);

        node.add_input_pin(
            "starting_page",
            "Starting Page",
            "The page to start extracting links from",
            VariableType::String,
        );

        node.add_input_pin(
            "same_domain",
            "Same Domain",
            "Stay on the same domain or subdomains",
            VariableType::Boolean,
        )
        .set_default_value(Some(flow_like_types::json::json!(false)));

        node.add_input_pin(
            "offset_ms",
            "Delay (ms)",
            "Delay between requests",
            VariableType::Integer,
        )
        .set_default_value(Some(json!(1000)));

        node.add_input_pin(
            "depth",
            "Depth",
            "The depth to extract links from",
            VariableType::Integer,
        )
        .set_default_value(Some(flow_like_types::json::json!(1)));

        node.add_output_pin("exec_out", "", "", VariableType::Execution);

        node.add_output_pin(
            "links",
            "Links",
            "The extracted links",
            VariableType::String,
        )
        .set_value_type(flow_like::flow::pin::ValueType::HashSet);

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        context.deactivate_exec_pin("exec_out").await?;

        let starting_page = context.evaluate_pin::<String>("starting_page").await?;
        let same_domain = context.evaluate_pin::<bool>("same_domain").await?;
        let depth_input = context.evaluate_pin::<i32>("depth").await?;
        let offset_ms = context.evaluate_pin::<i64>("offset_ms").await? as u64;

        let max_depth = std::cmp::min(depth_input.max(0), 40) as usize;

        let start_url = Url::parse(&starting_page)?;

        let found_links = crawl_links(start_url.clone(), max_depth, same_domain, offset_ms).await;

        context.set_pin_value("links", json!(found_links)).await?;
        context.activate_exec_pin("exec_out").await?;
        Ok(())
    }
}

/// Crawls up to `max_depth` starting from `start_url`.
/// If `same_domain` is true, only follows links on the same domain.
async fn crawl_links(
    start_url: Url,
    max_depth: usize,
    same_domain: bool,
    offset_ms: u64,
) -> HashSet<String> {
    let mut visited = HashSet::new();
    let mut links = HashSet::new();
    let mut queue = VecDeque::new();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("FlowLike/0.1")
        .build()
        .unwrap_or_default();

    visited.insert(start_url.to_string());
    queue.push_back((start_url, max_depth));

    let selector = Selector::parse("a[href]").unwrap();

    while let Some((url, depth)) = queue.pop_front() {
        if depth == 0 {
            continue;
        }

        tokio::time::sleep(Duration::from_millis(offset_ms)).await;

        let resp = match client.get(url.clone()).send().await {
            Ok(r) => r,
            Err(_) => continue,
        };

        if !resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.starts_with("text/html"))
        {
            continue;
        }

        let body = match resp.text().await {
            Ok(t) => t,
            Err(_) => continue,
        };

        let document = Html::parse_document(&body);
        for el in document.select(&selector) {
            if let Some(href) = el.value().attr("href") {
                if href.starts_with('#')
                    || href.starts_with("mailto:")
                    || href.starts_with("javascript:")
                {
                    continue;
                }
                if let Ok(link) = url.join(href) {
                    let link_str = link.to_string();

                    if visited.insert(link_str.clone())
                        && (!same_domain || link.domain() == url.domain())
                    {
                        links.insert(link_str.clone());
                        queue.push_back((link, depth - 1));
                    }
                }
            }
        }
    }

    links
}
