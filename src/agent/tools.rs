use std::collections::HashMap;
use async_openai::types::responses::{Tool, FunctionToolArgs, FunctionToolCall, WebSearchToolArgs};
use serde_json::{Value,json};
use serde::{de::DeserializeOwned, Serialize};
use schemars::{JsonSchema, schema_for};
use tracing::{info, error};

use crate::error::Error;
type Result<T> = std::result::Result<T, Error>;
mod doc_search_tool;
use doc_search_tool::DocSearchTool;

pub trait ToolDefinition{
    type Params;
    type Output;
    // required functions
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn execute(&self, args: Self::Params) -> Result<Self::Output>;
}

trait ToolDefinitionDyn{
    // required functions
    fn tool_name(&self) -> String;
    fn tool_description(&self) -> String;
    fn tool_parameters(&self) -> Value;
    fn tool_execute(&self, args: Value) -> Result<Value>;
}
impl<T> ToolDefinitionDyn for T 
    where 
        T: ToolDefinition,
        T::Params: JsonSchema + DeserializeOwned,
        T::Output: Serialize
{
    fn tool_name(&self) -> String {
        self.name()
    }
    fn tool_description(&self) -> String {
        self.description()
    }
    fn tool_parameters(&self) -> Value {
        let schema = schema_for!(T::Params);
        serde_json::to_value(schema).unwrap_or(json!({ "type": "object" }))
    }
    fn tool_execute(&self, args: Value) -> Result<Value> {
        info!("Start tool execute: {}({})", self.name(), args);
        let parsed_args: T::Params = serde_json::from_value(args)
            .map_err(|e| Error::Serde(format!("Unable to parse params for {}: {}", self.name(), e)))?;

        let output = self.execute(parsed_args)
            .map_err(|e| Error::Tool(format!("Tool {} execute error: {}", self.name(), e)))?;
        let val = serde_json::to_value(output)
            .map_err(|e| Error::Tool(format!("Tool {} output parse error: {}", self.name(), e)))?;
        Ok(val)
    }
}

pub struct AgentTools{
    web_search: bool,
    custom_tools: HashMap<String, Box<dyn ToolDefinitionDyn>>, 
    execution_requests: Vec<FunctionToolCall>
}

impl AgentTools{
    pub fn new() -> Self{
        Self{
            web_search: false,
            custom_tools: HashMap::new(),
            execution_requests: Vec::new(),
        }
    }
    pub fn build(mut self) -> Self {
        let search_tool = DocSearchTool{};
        self.custom_tools.insert(search_tool.name(), Box::new(search_tool));
        self
    }
    
    pub fn get_tools(&self) -> Vec<Tool> {
        let mut tools = Vec::new();
        // add custom FuntionTools
        for custom_tool in self.custom_tools.values(){
            // create OpenAI FunctionTool
            if let Ok(tool) = FunctionToolArgs::default()
                .name(custom_tool.tool_name())
                .description(custom_tool.tool_description())
                .parameters(custom_tool.tool_parameters())
                .build(){
                    tools.push(Tool::Function(tool));
            }
        }
        // add WebSearch tool
        if self.web_search {
            tools.push(Tool::WebSearch(WebSearchToolArgs::default().build()
                    .expect("Unable to build WebSearchTool")));
        }
        tools
    }

    pub fn with_web_search(mut self, enable: bool) -> Self {
        self.web_search = enable;
        self
    }

    pub fn collect_execution_request(&mut self, function_call: &FunctionToolCall) {
        self.execution_requests.clear();
        self.execution_requests.push(function_call.clone());
    }
    pub fn execute_collected_requests(&mut self) {
        for tool_req in &self.execution_requests {
            match self.execute_function_call(tool_req) {
                Ok(result) => info!("FunctionToolCall returned: {}", result),
                Err(err) => error!("FunctionToolCall error: {}", err),
            }
        }
    }


    fn execute_function_call(&self, function_call: &FunctionToolCall) -> Result<Value> {
        if let Some(tool) = self.custom_tools.get(&function_call.name){
            let args: Value = serde_json::from_str(&function_call.arguments)?;
            let output = tool.tool_execute(args);
            return output;
        }
        Err(Error::Generic("Tool not found".into()))
    }
}

