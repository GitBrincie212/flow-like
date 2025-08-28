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
};

#[derive(Default)]
pub struct ConstantStringValue {}

impl ConstantStringValue {
    pub fn new() -> Self {
        ConstantStringValue {}
    }
}

#[async_trait]
impl NodeLogic for ConstantStringValue {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "constant_string",
            "Constant String",
            "Generates a constant string value",
            "Utils/String",
        );
        node.add_icon("/flow/icons/grip.svg");

        node.add_input_pin(
            "_literal",
            "_literal",
            "The value of the string",
            VariableType::String,
        )
            .set_default_value(Some(json!("")));
        
        node.add_output_pin(
            "value",
            "Value",
            "The generated string value",
            VariableType::String,
        );

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        let value: String = context.evaluate_pin("_literal").await?;
        context.set_pin_value("value", json!(value)).await?;
        Ok(())
    }
}
