use flow_like::{
    flow::{
        execution::{LogLevel, context::ExecutionContext},
        node::{Node, NodeLogic},
        pin::PinOptions,
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_types::{async_trait, bail, json::json};

use crate::data::path::FlowPath;

/// NewWorksheetNode
/// -----------------
/// Creates a new worksheet (tab) inside an existing `.xlsx` workbook.
///
/// This is an **impure** node because it mutates a file.
///
/// Notes
/// - Currently appends the sheet at the end of the workbook.
/// - Excel limits: sheet names are max 31 chars and cannot contain: `:/\\?*[]` and cannot end with a single quote `'`.
#[derive(Default)]
pub struct NewWorksheetNode {}

impl NewWorksheetNode {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl NodeLogic for NewWorksheetNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "files_spreadsheet_new_worksheet",
            "New Worksheet",
            "Creates a new worksheet (tab) inside an existing .xlsx file",
            "Data/Excel",
        );
        node.add_icon("/flow/icons/file-spreadsheet.svg");

        node.add_input_pin("exec_in", "Input", "Trigger", VariableType::Execution);

        node.add_input_pin(
            "file",
            "File",
            "The .xlsx file to modify",
            VariableType::Struct,
        )
        .set_schema::<FlowPath>()
        .set_options(PinOptions::new().set_enforce_schema(true).build());

        node.add_input_pin(
            "sheet_name",
            "Sheet Name",
            "Desired worksheet name",
            VariableType::String,
        )
        .set_default_value(Some(json!("Sheet1")));

        node.add_input_pin(
            "if_exists",
            "If Exists",
            "What to do if the sheet already exists",
            VariableType::String,
        )
        .set_default_value(Some(json!("error")))
        .set_options(
            PinOptions::new()
                .set_valid_values(vec![
                    "error".to_string(),
                    "skip".to_string(),
                    "rename".to_string(),
                ])
                .build(),
        );

        node.add_output_pin(
            "exec_out",
            "Success",
            "Success path",
            VariableType::Execution,
        );
        node.add_output_pin(
            "created",
            "Created",
            "Whether a new sheet was created",
            VariableType::Boolean,
        );
        node.add_output_pin(
            "final_name",
            "Final Name",
            "Actual sheet name used",
            VariableType::String,
        );

        node
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        use std::io::Cursor;

        context.deactivate_exec_pin("exec_out").await?;

        let file: FlowPath = context.evaluate_pin("file").await?;
        let mut sheet_name: String = context.evaluate_pin("sheet_name").await?;
        let if_exists: String = context.evaluate_pin("if_exists").await?;

        sheet_name = sanitize_sheet_name(&sheet_name);
        if sheet_name.is_empty() {
            bail!("Invalid sheet name (empty after sanitization)");
        }

        let bytes = file.get(context, false).await?;

        let mut workbook = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true)?;

        let mut target_name = sheet_name.clone();
        let exists = workbook.get_sheet_by_name(&target_name).is_some();

        let mut created = false;
        if exists {
            match if_exists.as_str() {
                "skip" => {
                    context.log_message(
                        &format!("Sheet '{}' exists. Skipping creation.", target_name),
                        LogLevel::Info,
                    );
                }
                "rename" => {
                    target_name = next_unique_sheet_name(&workbook, &target_name);
                    workbook.new_sheet(&target_name).map_err(|e| {
                        flow_like_types::anyhow!("Failed to create sheet '{}': {}", target_name, e)
                    })?;
                    created = true;
                }
                _ => {
                    bail!("Sheet '{}' already exists", target_name);
                }
            }
        } else {
            workbook.new_sheet(&target_name).map_err(|e| {
                flow_like_types::anyhow!("Failed to create sheet '{}': {}", target_name, e)
            })?;
            created = true;
        }

        if created {
            let mut out = Cursor::new(Vec::<u8>::new());
            umya_spreadsheet::writer::xlsx::write_writer(&workbook, &mut out)?;
            let out_bytes = out.into_inner();
            file.put(context, out_bytes, false).await?;
        }

        context.set_pin_value("created", json!(created)).await?;
        context
            .set_pin_value("final_name", json!(target_name))
            .await?;
        context.activate_exec_pin("exec_out").await?;
        Ok(())
    }
}

/// Replace illegal characters and enforce Excel name limits.
fn sanitize_sheet_name(input: &str) -> String {
    let illegal = [':', '/', '\\', '?', '*', '[', ']'];
    let mut s: String = input
        .chars()
        .map(|c| if illegal.contains(&c) { ' ' } else { c })
        .collect();
    // Remove leading/trailing quotes
    if s.ends_with('\'') {
        s.pop();
    }
    s = s.trim().to_string();
    // Collapse whitespace
    let mut collapsed = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                collapsed.push(' ');
            }
            prev_space = true;
        } else {
            prev_space = false;
            collapsed.push(ch);
        }
    }
    // Max 31 chars
    if collapsed.len() > 31 {
        collapsed.truncate(31);
    }
    collapsed
}

/// Generate a unique sheet name with numeric suffixes: "Name (1)", "Name (2)", ...
fn next_unique_sheet_name(book: &umya_spreadsheet::Spreadsheet, base: &str) -> String {
    let mut n = 1usize;
    loop {
        let candidate = format!("{} ({})", base, n);
        if book.get_sheet_by_name(&candidate).is_none() {
            return candidate;
        }
        n += 1;
        // Guard just in case
        if n > 9999 {
            return format!("{} (copy)", base);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_sheet_name;
    #[test]
    fn test_sanitize() {
        assert_eq!(sanitize_sheet_name("A:B/C\\D?E*F[G]"), "A B C D E F G");
        assert_eq!(sanitize_sheet_name("   Hello   World   "), "Hello World");
        assert_eq!(
            sanitize_sheet_name("012345678901234567890123456789012345"),
            "0123456789012345678901234567890"
        );
    }
}
