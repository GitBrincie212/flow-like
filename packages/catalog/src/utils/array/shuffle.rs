use flow_like::{
    flow::{
        board::Board,
        execution::context::ExecutionContext,
        node::{Node, NodeLogic},
        pin::{PinOptions, ValueType},
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{async_trait, json::json, rand::{self, rngs::SmallRng, seq::SliceRandom, SeedableRng}, Value};
use std::sync::Arc;

#[derive(Default)]
pub struct ShuffleArrayNode {}

impl ShuffleArrayNode {
    pub fn new() -> Self {
        ShuffleArrayNode {}
    }
}

#[async_trait]
impl NodeLogic for ShuffleArrayNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "array_shuffle",
            "Shuffle",
            "Shuffle Array Items",
            "Utils/Array",
        );
        node.add_icon("/flow/icons/grip.svg");

        node.add_input_pin("array_in", "Array", "Your Array", VariableType::Generic)
            .set_value_type(ValueType::Array)
            .set_options(
                PinOptions::new()
                    .set_enforce_generic_value_type(true)
                    .build(),
            );

        node.add_output_pin(
            "array_out",
            "Array",
            "Adjusted Array",
            VariableType::Generic,
        )
        .set_value_type(ValueType::Array)
        .set_options(
            PinOptions::new()
                .set_enforce_generic_value_type(true)
                .build(),
        );

        node
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        let mut array: Vec<Value> = context.evaluate_pin("array_in").await?;
        let mut rng = SmallRng::from_rng(&mut rand::rng());  // non-cryptographic is ok here as we just want good statistical quality
        array.shuffle(&mut rng);
        context.set_pin_value("array_out", json!(array)).await?;
        Ok(())
    }

    async fn on_update(&self, node: &mut Node, board: Arc<Board>) {
        let _ = node.match_type("array_out", board.clone(), Some(ValueType::Array), None);
        let _ = node.match_type("array_in", board.clone(), Some(ValueType::Array), None);
        let _ = node.match_type("value", board, Some(ValueType::Normal), None);
        node.harmonize_type(vec!["array_in", "array_out", "value"], true);
    }
}
