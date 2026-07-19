
use crate::{agent::tools::ToolDefinitionDyn};
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::info;
use anyhow::Result;


#[derive(JsonSchema, Deserialize)]
pub struct DocSearchToolArgs{
    /// The search query to be used
    query: String,
    /// Max number of results to return
    max: Option<usize>,
}
pub struct DocSearchTool{
}
impl DocSearchTool {
    async fn execute(&self, DocSearchToolArgs{query: _ , max: _}: DocSearchToolArgs) -> Result<String> {
        Ok("invarte surubul verde de trei ori la stanga".to_string())
    }
}

impl ToolDefinitionDyn for DocSearchTool {
    fn tool_name(&self) -> &str {
        "search_documentation"
    }
    fn tool_description(&self) -> &str {
        "Documentation search for boiler type XV33AB"
    }
    fn tool_parameters(&self) -> &serde_json::Value {
        static SCHEMA: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
            let schema = schemars::schema_for!(DocSearchToolArgs);
            serde_json::to_value(schema).unwrap_or(serde_json::json!({ "type": "object" }))
        });
        &SCHEMA
    }
    fn tool_execute(&self, args: serde_json::Value) -> std::pin::Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send + '_ >> {
        Box::pin(async {
            info!("Start tool execute: {}({})", self.tool_name(), args);
            let parsed_args: DocSearchToolArgs = serde_json::from_value(args)
                .map_err(|e| anyhow::anyhow!("Unable to parse params for {}: {}", self.tool_name(), e))?;

            let output = self.execute(parsed_args).await
                .map_err(|e| anyhow::anyhow!("Tool {} execute error: {}", self.tool_name(), e))?;
            let val = serde_json::to_value(output)
                .map_err(|e| anyhow::anyhow!("Tool {} output parse error: {}", self.tool_name(), e))?;
            Ok(val)
        })
    }
}

