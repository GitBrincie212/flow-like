use flow_like::{
    flow::{
        board::Board,
        execution::context::ExecutionContext,
        node::{Node, NodeLogic},
        pin::ValueType,
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{Value, async_trait, json::json};
use std::sync::Arc;

#[derive(Default)]
pub struct ArrayIncludesNode {}

impl ArrayIncludesNode {
    pub fn new() -> Self {
        ArrayIncludesNode {}
    }
}

#[async_trait]
impl NodeLogic for ArrayIncludesNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "array_includes",
            "Includes",
            "Checks if an array includes a certain value",
            "Utils/Array",
        );

        node.add_icon("/flow/icons/grip.svg");

        node.add_input_pin("array_in", "Array", "Your Array", VariableType::Generic)
            .set_value_type(ValueType::Array);

        node.add_input_pin(
            "value",
            "Value",
            "Value to search for",
            VariableType::Generic,
        );

        node.add_output_pin(
            "includes",
            "Includes?",
            "Does the array include the value?",
            VariableType::Boolean,
        );

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        let array_in = context.evaluate_pin_to_ref("array_in").await?;
        let value: Value = context.evaluate_pin("value").await?;

        let mut includes = false;

        {
            let array_in = array_in.as_ref().lock().await;

            if let Some(array) = array_in.as_array() {
                for item in array.iter() {
                    if item == &value {
                        includes = true;
                        break;
                    }
                }
            }
        }

        context.set_pin_value("includes", json!(includes)).await?;
        Ok(())
    }

    async fn on_update(&self, node: &mut Node, board: Arc<Board>) {
        let _ = node.match_type("array_in", board.clone(), Some(ValueType::Array), None);
        let _ = node.match_type("value", board, Some(ValueType::Normal), None);
        node.harmonize_type(vec!["array_in", "value"], true);
    }
}
