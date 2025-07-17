use flow_like::{
    flow::{
        execution::context::ExecutionContext,
        node::{Node, NodeLogic},
        pin::PinOptions,
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_storage::databases::vector::VectorStore;
use flow_like_types::{async_trait, json::json};

use super::NodeDBConnection;

#[derive(Default)]
pub struct IndexLocalDatabaseNode {}

impl IndexLocalDatabaseNode {
    pub fn new() -> Self {
        IndexLocalDatabaseNode {}
    }
}

#[async_trait]
impl NodeLogic for IndexLocalDatabaseNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "index_local_db",
            "Build Index",
            "Build Index",
            "Database/Local/Optimization",
        );
        node.add_icon("/flow/icons/database.svg");

        node.add_input_pin("exec_in", "Input", "", VariableType::Execution);
        node.add_input_pin(
            "database",
            "Database",
            "Database Connection Reference",
            VariableType::Struct,
        )
        .set_schema::<NodeDBConnection>()
        .set_options(PinOptions::new().set_enforce_schema(true).build());

        node.add_input_pin("column", "Column", "Column to Index", VariableType::String)
            .set_default_value(Some(json!("")));
        node.add_input_pin("type", "Type", "Index Type to build", VariableType::String)
            .set_options(
                PinOptions::new()
                    .set_valid_values(vec![
                        "BTREE".to_string(),
                        "BITMAP".to_string(),
                        "LABEL LIST".to_string(),
                        "FULL TEXT".to_string(),
                        "VECTOR".to_string(),
                        "AUTO".to_string(),
                    ])
                    .build(),
            )
            .set_default_value(Some(json!("AUTO")));

        node.add_output_pin(
            "exec_out",
            "Created Database",
            "Done Creating Database",
            VariableType::Execution,
        );

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        context.deactivate_exec_pin("exec_out").await?;

        let index_type: String = context.evaluate_pin("type").await?;
        let database: NodeDBConnection = context.evaluate_pin("database").await?;
        let database = database
            .load(context, &database.cache_key)
            .await?
            .db
            .clone();
        let database = database.read().await;
        let column: String = context.evaluate_pin("column").await?;
        database.index(&column, Some(&index_type)).await?;

        context.activate_exec_pin("exec_out").await?;
        Ok(())
    }
}
