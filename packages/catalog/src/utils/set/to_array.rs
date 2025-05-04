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
use std::{collections::HashSet, sync::Arc};

#[derive(Default)]
pub struct SetToArrayNode {}

impl SetToArrayNode {
    pub fn new() -> Self {
        SetToArrayNode {}
    }
}

#[async_trait]
impl NodeLogic for SetToArrayNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "set_to_array",
            "Set to Array",
            "Converts a set to an array",
            "Utils/Set",
        );

        node.add_icon("/flow/icons/grip.svg");

        node.add_input_pin("set_in", "Set", "", VariableType::Generic)
            .set_value_type(ValueType::HashSet);

        node.add_output_pin("array_out", "Array", "", VariableType::Generic)
            .set_value_type(ValueType::Array);

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        let set_in: HashSet<Value> = context.evaluate_pin("set_in").await?;
        let array_out: Vec<Value> = set_in.into_iter().collect();

        context.set_pin_value("array_out", json!(array_out)).await?;
        Ok(())
    }

    async fn on_update(&self, node: &mut Node, board: Arc<Board>) {
        let _ = node.match_type(
            "set_in",
            board.clone(),
            Some(ValueType::HashSet),
            Some(ValueType::HashSet),
        );
        let _ = node.match_type(
            "array_out",
            board,
            Some(ValueType::Array),
            Some(ValueType::Array),
        );
        node.harmonize_type(vec!["set_in", "array_out"], true);
    }
}
