use flow_like::{
    flow::{
        board::Board,
        execution::context::ExecutionContext,
        node::{Node, NodeLogic},
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{Value, async_trait, json::json};
use std::sync::Arc;

#[derive(Default)]
pub struct ToStringNode {}

impl ToStringNode {
    pub fn new() -> Self {
        ToStringNode {}
    }
}

#[async_trait]
impl NodeLogic for ToStringNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "val_to_string",
            "To String",
            "Convert any object to String",
            "Utils/Conversions",
        );
        node.add_icon("/flow/icons/convert.svg");

        node.add_input_pin("value", "Value", "Input Value", VariableType::Generic);
        node.add_input_pin(
            "pretty",
            "Pretty?",
            "Should the struct be pretty printed?",
            VariableType::Boolean,
        )
        .set_default_value(Some(json!(true)));

        node.add_output_pin("string", "String", "Output String", VariableType::String);

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        let string = context.evaluate_pin_to_ref("value").await?;
        let pretty = context.evaluate_pin::<bool>("pretty").await?;

        let value: String = {
            let guard = string.as_ref().lock().await;
            let v: &flow_like_types::Value = &guard;

            let s: String = match v {
                Value::String(inner) => inner.clone(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => n.to_string(),
                _ if pretty => flow_like_types::json::to_string_pretty(v)?,
                _ => flow_like_types::json::to_string(v)?,
            };
            s
        };
        context
            .set_pin_value("string", flow_like_types::json::json!(value))
            .await?;
        Ok(())
    }

    async fn on_update(&self, node: &mut Node, board: Arc<Board>) {
        let _ = node.match_type("value", board, None, None);
    }
}
