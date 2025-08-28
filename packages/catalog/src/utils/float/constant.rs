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
    rand::{self, Rng},
};

#[derive(Default)]
pub struct ConstantFloatValue {}

impl ConstantFloatValue {
    pub fn new() -> Self {
        ConstantFloatValue {}
    }
}

#[async_trait]
impl NodeLogic for ConstantFloatValue {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "constant_float",
            "Constant Float",
            "Generates a constant float value",
            "Math/Float",
        );
        node.add_icon("/flow/icons/grip.svg");

        node.add_input_pin(
            "_literal",
            "_literal",
            "The value of the float",
            VariableType::Float,
        )
            .set_default_value(Some(json!(0.0)));
        
        node.add_output_pin(
            "value",
            "Value",
            "The generated float value",
            VariableType::Float,
        );

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        let value: f64 = context.evaluate_pin("_literal").await?;
        context.set_pin_value("value", json!(value)).await?;
        Ok(())
    }
}
