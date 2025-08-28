use flow_like::{
    flow::{
        execution::context::ExecutionContext,
        node::{Node, NodeLogic},
        pin::PinOptions,
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{
    async_trait,
    json::json,
    rand::{self, Rng},
};

#[derive(Default)]
pub struct ConstantBoolNode {}

impl ConstantBoolNode {
    pub fn new() -> Self {
        ConstantBoolNode {}
    }
}

#[async_trait]
impl NodeLogic for ConstantBoolNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "constant_bool",
            "Constant Boolean",
            "Generates a constant boolean value",
            "Utils/Bool",
        );
        node.add_icon("/flow/icons/grip.svg");

        node.add_input_pin(
            "_literal",
            "_literal",
            "The value of the boolean",
            VariableType::Boolean,
        )
        .set_default_value(Some(json!(true)));

        node.add_output_pin(
            "value",
            "Value",
            "The generated boolean value",
            VariableType::Boolean,
        );

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        let value: bool = context.evaluate_pin("_literal").await?;
        context.set_pin_value("value", json!(value)).await?;
        Ok(())
    }
}
