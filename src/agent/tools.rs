use std::collections::HashMap;
use async_openai::types::responses::{Tool, FunctionTool, FunctionToolArgs, FunctionToolCall};
use serde_json::{Value};

use crate::error::Error;
type Result<T> = std::result::Result<T, Error>;
mod doc_search_tool;
use doc_search_tool::DocSearchTool;

pub trait ToolDefinition{
    // required functions
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn parameters(&self) -> Value;
    fn execute(&self, args: Value) -> Result<Value>;

    // provided functions
    fn to_function_tool(&self) -> Result<FunctionTool> {
        Ok(FunctionToolArgs::default()
            .name(self.name())
            .description(self.description())
            .parameters(self.parameters())
            .build()?)
    }
}

pub struct AgentTools{
    web_search: bool,
    custom_tools: HashMap<String, Box<dyn ToolDefinition>>, 
}

impl AgentTools{
    pub fn new() -> Self{
        Self{
            web_search: false,
            custom_tools: HashMap::new(),
        }
    }
    pub fn build(&mut self) {
        // if self.web_search {
        //     self.tools.push(Tool::WebSearch(WebSearchToolArgs::default().build()?));
        // }
        let search_tool = DocSearchTool{};
        self.custom_tools.insert(search_tool.name(), Box::new(search_tool));
    }
    
    pub fn get_tools(&self) -> Vec<Tool> {
        let mut tools = Vec::new();
        for custom_tool in self.custom_tools.values(){
            if let Ok(tool) = custom_tool.to_function_tool() {
                tools.push(Tool::Function(tool));
            }
        }
        tools
    }

    pub fn with_web_search(&mut self, enable: bool) -> &mut Self {
        self.web_search = enable;
        self
    }

    pub fn execute_function_call(&self, function_call: &FunctionToolCall) -> Result<Value> {
        if let Some(tool) = self.custom_tools.get(&function_call.name){
            let args: Value = serde_json::from_str(&function_call.arguments)?;
            let output = tool.execute(args);
            return output;
        }
        Err(Error::Generic("Tool not found".into()))
    }
}

