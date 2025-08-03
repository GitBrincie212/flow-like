/// # LLM Branch Node
/// Like the Control > Branch Node but with integrated decision making about the input being either True or False
/// Useful to route a execution flow between two possible downstream branches.
/// Node execution might fail if the LLM-output cannot be parsed according to the decision data schema.
use crate::ai::generative::llm::with_tools::extract_tagged;
use flow_like::{
    bit::Bit,
    flow::{
        execution::{LogLevel, context::ExecutionContext},
        node::{Node, NodeLogic},
        pin::PinOptions,
        variable::VariableType,
    },
    state::FlowLikeState,
};
use flow_like_model_provider::history::{History, HistoryMessage, Role};
use flow_like_types::{anyhow, async_trait, json};
use serde::Deserialize;

// todo: refactor / align with tool use node
const SYSTEM_PROMPT: &str = r#"
# Instruction
You are a binary decision maker that generates decisions as json data.

Rate the user's input whether it evaluates to true/yes or false/no.

Use the tool schema below to generate valid json data.

Do not ask follow-up questions.

At every input, reply with valid json data.

Wrap json data in <tooluse></tooluse> xml tags.

# Schema
Here is the tool schema, you **must** use to make your response:

{
  "description": "Your decision represented with a boolean value. Choose true if the decision is yes, choose false if the decision is no.",
  "name": "submit_decision",
  "parameters": {
    "additionalProperties": false,
    "properties": {
      "reason": {
        "description": "Short explanation for the decision made.",
        "type": "string"
      },
      "decision": {
        "description": "The decision made.",
        "type": "boolean"
      }
    },
    "required": [
      "reason",
      "decision"
    ],
    "type": "object"
  },
  "strict": true,
  "type": "function"
}

# Required Response Format
<tooluse>
    {
        "name": "name", 
        "args": {
            "reason": ...,
            "decision": ...
        }
    }
</tooluse>

# Important Instructions
- Your json data will be validated by the json schema above. It **MUST** pass this validation.
- You **MUST** wrap your json data in xml tags <tooluse></tooluse>
"#;

// refactor / share between llm nodes with tool use
#[derive(Debug, Deserialize)]
struct BranchToolCallArgs {
    reason: String,
    decision: bool,
}

#[derive(Debug, Deserialize)]
struct BranchToolCall {
    name: String,
    args: BranchToolCallArgs,
}

#[derive(Default)]
pub struct LLMBranchNode {}

impl LLMBranchNode {
    pub fn new() -> Self {
        LLMBranchNode {}
    }
}

#[async_trait]
impl NodeLogic for LLMBranchNode {
    async fn get_node(&self, _app_state: &FlowLikeState) -> Node {
        let mut node = Node::new(
            "llm_branch",
            "LLM Branch",
            "LLM If-Else Router",
            "AI/Generative",
        );
        node.add_icon("/flow/icons/split.svg");

        node.add_input_pin("exec_in", "Input", "Trigger Pin", VariableType::Execution);

        node.add_input_pin("model", "Model", "Model", VariableType::Struct)
            .set_schema::<Bit>()
            .set_options(PinOptions::new().set_enforce_schema(true).build());

        // todo: name this 'Condition' to align with Branch node?
        node.add_input_pin(
            "prompt",
            "Prompt",
            "A statement that can be answered with yes or no.",
            VariableType::String,
        );

        node.add_output_pin(
            "true",
            "True",
            "The flow to follow if the condition is true",
            VariableType::Execution,
        );
        node.add_output_pin(
            "false",
            "False",
            "The flow to follow if the condition is false",
            VariableType::Execution,
        );

        node.set_long_running(true);

        return node;
    }

    async fn run(&self, context: &mut ExecutionContext) -> flow_like_types::Result<()> {
        context.deactivate_exec_pin("true").await?;
        context.deactivate_exec_pin("false").await?;

        // fetch inputs
        let model = context.evaluate_pin::<Bit>("model").await?;
        let prompt: String = context.evaluate_pin::<String>("prompt").await?;
        let mut model_name = model.id.clone();
        if let Some(meta) = model.meta.get("en") {
            model_name = meta.name.clone();
        }

        let mut history = History::new(model_name.clone(), vec![]);
        history.set_system_prompt(SYSTEM_PROMPT.to_string());
        history.push_message(HistoryMessage::from_string(Role::User, &prompt));

        // generate response
        let response = {
            // load model
            let model_factory = context.app_state.lock().await.model_factory.clone();
            let model = model_factory
                .lock()
                .await
                .build(&model, context.app_state.clone())
                .await?;
            model.invoke(&history, None).await?
        }; // drop model

        // parase tool call from response
        let mut response_string = "".to_string();
        if let Some(response) = response.last_message() {
            response_string = response.content.clone().unwrap_or("".to_string());
        }
        context.log_message(&response_string, LogLevel::Debug);

        let mut tool_calls_str = extract_tagged(&response_string, "tooluse")?;
        let tool_call: BranchToolCall = if tool_calls_str.len() >= 1 {
            // account for reasoning models which might produce tooluse tags multiple times
            // we are assuming that the last occurance of tooluse is the actual one
            let tool_call_str = tool_calls_str.pop().unwrap();
            json::from_str(&tool_call_str)?
        } else {
            return Err(anyhow!(format!(
                "Invalid number of tool calls: Expected 1, got {}",
                tool_calls_str.len()
            )));
        };
        context.log_message(&tool_call.args.reason, LogLevel::Debug);

        // set outputs
        if tool_call.args.decision {
            context.activate_exec_pin("true").await?;
            context.deactivate_exec_pin("false").await?;
        } else {
            context.activate_exec_pin("false").await?;
            context.deactivate_exec_pin("true").await?;
        }
        Ok(())
    }
}
