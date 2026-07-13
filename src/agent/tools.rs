use async_openai::types::responses::{Tool, WebSearchToolArgs, FunctionTool, FunctionToolArgs};
use serde_json::json;

use crate::error::Error;

pub struct AgentTools{
    web_search: bool,
    tools: Vec<Tool>, 
}

impl AgentTools{
    pub fn new() -> Self{
        Self{
            web_search: false,
            tools: Vec::new(),
        }
    }

    pub fn build_tools(&mut self) -> Result<Vec<Tool>, Error> {
        if self.web_search {
            self.tools.push(Tool::WebSearch(WebSearchToolArgs::default().build()?));
        }
        self.tools.push(Tool::Function(self.custom_function_tool()));
        Ok(self.tools.clone())
    }
    pub fn with_web_search(&mut self, enable: bool) -> &mut Self {
        self.web_search = enable;
        self
    }

    fn custom_function_tool(&self) -> FunctionTool {
        FunctionToolArgs::default()
            .name("search_documentation")
            .description("Documentation search for boiler type XV33AB")
            .parameters(json!({
                "type": "object",
                "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to be used",
                    },
                },
                "required": ["query"],
            }))
            .build().expect("unable to build custom tool")
    }
}
