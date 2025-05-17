use flow_like::{
    flow::{
        board::Board,
        execution::{LogLevel, context::ExecutionContext, internal_node::InternalNode},
        node::{Node, NodeLogic},
        pin::{PinOptions, ValueType},
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{Value, async_trait};
use std::sync::Arc;

#[derive(Default)]
pub struct LoopNode {}

impl LoopNode {
    pub fn new() -> Self {
        LoopNode {}
    }
}

#[async_trait]
impl NodeLogic for LoopNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "control_for_each",
            "For Each",
            "Loops over an Array",
            "Control",
        );
        node.add_icon("/flow/icons/for-each.svg");

        node.add_input_pin("exec_in", "Input", "Trigger Pin", VariableType::Execution);
        node.add_input_pin("array", "Array", "Array to Loop", VariableType::Generic)
            .set_value_type(ValueType::Array)
            .set_options(
                PinOptions::new()
                    .set_enforce_generic_value_type(true)
                    .build(),
            );

        node.add_output_pin(
            "exec_out",
            "For Each Element",
            "Executes the current item",
            VariableType::Execution,
        );
        node.add_output_pin(
            "value",
            "Value",
            "The current item Value",
            VariableType::Generic,
        );
        node.add_output_pin(
            "index",
            "Index",
            "Current Array Index",
            VariableType::Integer,
        );

        node.add_output_pin(
            "done",
            "Done",
            "Executes once the array is dealt with.",
            VariableType::Execution,
        );

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        let done = context.get_pin_by_name("done").await?;
        context.deactivate_exec_pin_ref(&done).await?;

        let array = context.get_pin_by_name("array").await?;
        let value = context.get_pin_by_name("value").await?;
        let exec_item = context.get_pin_by_name("exec_out").await?;
        let connected = exec_item.lock().await.get_connected_nodes().await;
        let index = context.get_pin_by_name("index").await?;

        let array_value: Value = context.evaluate_pin_ref(array).await?;
        let array_value = array_value
            .as_array()
            .ok_or(flow_like_types::anyhow!("Array value is not an array"))?;

        context.activate_exec_pin_ref(&exec_item).await?;
        for (i, item) in array_value.iter().enumerate() {
            let item = item.to_owned();
            value.lock().await.set_value(item).await;
            index
                .lock()
                .await
                .set_value(flow_like_types::Value::from(i))
                .await;
            for node in connected.iter() {
                let mut sub_context = context.create_sub_context(&node).await;
                let run = InternalNode::trigger(&mut sub_context, &mut None, true).await;
                sub_context.end_trace();
                context.push_sub_context(sub_context);

                if run.is_err() {
                    let error = run.err().unwrap();
                    context.log_message(
                        &format!("Error: {:?} in iteration {}", error, i),
                        LogLevel::Error,
                    );
                }
            }
        }

        context.deactivate_exec_pin_ref(&exec_item).await?;
        context.activate_exec_pin_ref(&done).await?;

        return Ok(());
    }

    async fn on_update(&self, node: &mut Node, board: Arc<Board>) {
        let _ = node.match_type(
            "array",
            board.clone(),
            Some(ValueType::Array),
            Some(ValueType::Array),
        );
        let _ = node.match_type(
            "value",
            board,
            Some(ValueType::Normal),
            Some(ValueType::Normal),
        );
        node.harmonize_type(vec!["array", "value"], true);
    }
}
