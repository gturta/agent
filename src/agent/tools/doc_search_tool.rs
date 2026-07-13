
use crate::agent::tools::ToolDefinition;
use schemars::{JsonSchema, schema_for};
use serde_json::{Value, json};
use serde::Deserialize;
use tracing::info;
use crate::error::Result;


#[derive(JsonSchema, Deserialize)]
pub struct DocSearchToolArgs{
    /// The search query to be used
    query: String,
    /// Max number of results to return
    max: Option<usize>,
}
pub struct DocSearchTool{}

impl ToolDefinition for DocSearchTool{

    fn name(&self) -> String {
        "search_documentation".to_owned()
    }
    fn description(&self) -> String {
        "Documentation search for boiler type XV33AB".to_owned()
    }
    fn parameters(&self) -> Value {
        let schema = schema_for!(DocSearchToolArgs);
        serde_json::to_value(schema).unwrap_or(json!({ "type": "object" }))
    }
    fn execute(&self, args: Value) -> Result<Value> {
        let parsed_args: DocSearchToolArgs = serde_json::from_value(args)?;
        info!("DocSearchTool execute(query: \"{}\", max: {:?})", parsed_args.query, parsed_args.max);

        Ok(Value::Null)
    }
}
