use flow_like::{
    flow::{
        execution::context::ExecutionContext,
        node::{Node, NodeLogic},
        pin::PinOptions,
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{anyhow, async_trait, json::json};

use crate::data::path::FlowPath;

/// InsertRowNode
/// --------------
/// Inserts one or more rows **before/after** a target row in a worksheet
/// of an existing `.xlsx` workbook.
///
/// This is an **impure** node because it mutates a file.
///
/// Inputs
/// - `exec_in` (Execution): trigger.
/// - `file` (Struct<FlowPath>): the XLSX file to modify.
/// - `sheet_name` (String): target worksheet, defaults to `Sheet1`.
/// - `row` (Integer): 1-based target row index (e.g. `1`, `3`).
/// - `position` (Enum String): `before` (default) or `after` the target row.
/// - `num_rows` (Integer): how many rows to insert, default `1`, min `1`.
/// - `adjust_references` (Boolean): if `true` (default), adjust formulas/refs across the workbook
///   using the Spreadsheet-level API. If `false`, only the current sheet is adjusted.
///
/// Outputs
/// - `exec_out` (Execution): fired when operation completes successfully.
/// - `inserted` (Boolean): true when rows were inserted.
/// - `final_row_index` (Integer): the actual 1-based row index used.
/// - `total_rows_inserted` (Integer): echo of the `num_rows` actually applied.
#[derive(Default)]
pub struct InsertRowNode {}

impl InsertRowNode {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl NodeLogic for InsertRowNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "files_spreadsheet_insert_row",
            "Insert Row(s)",
            "Insert one or more rows into a worksheet",
            "Data/Excel",
        );
        node.add_icon("/flow/icons/file-spreadsheet.svg");

        // Exec
        node.add_input_pin("exec_in", "Input", "Trigger", VariableType::Execution);

        // File input
        node.add_input_pin(
            "file",
            "File",
            "The .xlsx file to modify",
            VariableType::Struct,
        )
        .set_schema::<FlowPath>()
        .set_options(PinOptions::new().set_enforce_schema(true).build());

        // Worksheet selector
        node.add_input_pin(
            "sheet_name",
            "Sheet Name",
            "Target worksheet name",
            VariableType::String,
        )
        .set_default_value(Some(json!("Sheet1")));

        // Row reference (1-based)
        node.add_input_pin(
            "row",
            "Row",
            "1-based target row index",
            VariableType::Integer,
        )
        .set_default_value(Some(json!(1)));

        // Before/After
        node.add_input_pin(
            "position",
            "Position",
            "Insert before or after the target row",
            VariableType::String,
        )
        .set_default_value(Some(json!("before")))
        .set_options(
            PinOptions::new()
                .set_valid_values(vec!["before".to_string(), "after".to_string()])
                .build(),
        );

        // Count
        node.add_input_pin(
            "num_rows",
            "Count",
            "How many rows to insert",
            VariableType::Integer,
        )
        .set_default_value(Some(json!(1)));

        // Reference adjustment
        node.add_input_pin(
            "adjust_references",
            "Adjust References",
            "Adjust formulas across workbook",
            VariableType::Boolean,
        )
        .set_default_value(Some(json!(true)));

        // Outputs
        node.add_output_pin(
            "exec_out",
            "Done",
            "Continue on success",
            VariableType::Execution,
        );
        node.add_output_pin(
            "inserted",
            "Inserted",
            "Whether rows were inserted",
            VariableType::Boolean,
        );
        node.add_output_pin(
            "final_row_index",
            "Final Row Index",
            "1-based row index used",
            VariableType::Integer,
        );
        node.add_output_pin(
            "total_rows_inserted",
            "Total Inserted",
            "How many rows were inserted",
            VariableType::Integer,
        );

        node
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        use std::io::Cursor;

        // Only one outgoing exec pin; deactivate so the graph won't continue on failure.
        context.deactivate_exec_pin("exec_out").await?;

        // Inputs
        let file: FlowPath = context.evaluate_pin("file").await?; // throws on failure
        let sheet_name: String = context.evaluate_pin("sheet_name").await?;
        let row_index_in: i64 = context.evaluate_pin("row").await.unwrap_or(1);
        let position: String = context.evaluate_pin("position").await?;
        let mut num_rows: i64 = context.evaluate_pin("num_rows").await.unwrap_or(1);
        let adjust_refs: bool = context
            .evaluate_pin("adjust_references")
            .await
            .unwrap_or(true);

        if row_index_in < 1 {
            return Err(anyhow!("Row must be >= 1 (got {})", row_index_in));
        }
        if num_rows < 1 {
            num_rows = 1;
        }

        let base_index: u32 = row_index_in as u32; // safe after check
        let insert_index: u32 = match position.as_str() {
            "after" => base_index + 1,
            _ => base_index,
        };
        let num_rows_u32: u32 = num_rows as u32;

        // Load workbook
        let bytes = file.get(context, false).await?; // throws on failure
        let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true)
            .map_err(|e| anyhow!("Failed to read workbook: {}", e))?;

        // Ensure sheet exists
        if book.get_sheet_by_name(&sheet_name).is_none() {
            return Err(anyhow!("Sheet '{}' not found", sheet_name));
        }

        // Insert rows
        if adjust_refs {
            // Spreadsheet-level insert adjusts references across workbook
            book.insert_new_row(&sheet_name, &insert_index, &num_rows_u32);
        } else {
            // Worksheet-only adjustment
            if let Some(ws) = book.get_sheet_by_name_mut(&sheet_name) {
                ws.insert_new_row(&insert_index, &num_rows_u32);
            } else {
                return Err(anyhow!("Sheet '{}' not found (mut)", sheet_name));
            }
        }

        // Persist
        let mut out = Cursor::new(Vec::<u8>::new());
        umya_spreadsheet::writer::xlsx::write_writer(&book, &mut out)
            .map_err(|e| anyhow!("Failed to write workbook: {}", e))?;
        file.put(context, out.into_inner(), false).await?; // throws on failure

        // Outputs
        context.set_pin_value("inserted", json!(true)).await?;
        context
            .set_pin_value("final_row_index", json!(insert_index as i64))
            .await?;
        context
            .set_pin_value("total_rows_inserted", json!(num_rows as i64))
            .await?;

        context.activate_exec_pin("exec_out").await?;
        Ok(())
    }
}
