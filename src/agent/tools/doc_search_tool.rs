
use crate::agent::tools::ToolDefinition;
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::info;
use crate::error::{Error, Result};


#[derive(JsonSchema, Deserialize)]
pub struct DocSearchToolArgs{
    /// The search query to be used
    query: String,
    /// Max number of results to return
    max: Option<usize>,
}
pub struct DocSearchTool{}

impl ToolDefinition for DocSearchTool{
    type Params = DocSearchToolArgs;
    type Output = String;

    fn name(&self) -> String {
        "search_documentation".to_owned()
    }
    fn description(&self) -> String {
        "Documentation search for boiler type XV33AB".to_owned()
    }
    fn execute(&self, args: Self::Params) -> Result<Self::Output> {
        info!("DocSearchTool execute(query: \"{}\", max: {:?})", args.query, args.max);

        Err(Error::Generic("DocSearchTool unimplemented".to_string()))
    }
}
