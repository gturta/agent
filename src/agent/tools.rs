use std::sync::Arc;
use std::pin::Pin;
use std::future::Future;
use std::collections::HashMap;
use async_openai::types::responses::{Tool, FunctionToolArgs, FunctionToolCall, WebSearchToolArgs};
use serde_json::{Value,json};
use serde::{de::DeserializeOwned, Serialize};
use schemars::{JsonSchema, schema_for};
use tracing::{info, error};

use crate::error::{Error, Result};
mod doc_search_tool;
use doc_search_tool::DocSearchTool;

pub trait ToolDefinition{
    type Params;
    type Output;
    // required functions
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn execute(&self, args: Self::Params) -> impl std::future::Future<Output = Result<Self::Output>> + Send + Sync;
}

trait ToolDefinitionDyn{
    // required functions
    fn tool_name(&self) -> String;
    fn tool_description(&self) -> String;
    fn tool_parameters(&self) -> Value;
    fn tool_execute(&self, args: Value) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + Sync + '_ >>;
}
impl<T> ToolDefinitionDyn for T 
    where 
        T: ToolDefinition + Sync,
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
    fn tool_execute(&self, args: Value) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + Sync + '_ >> {
        Box::pin(async {
            info!("Start tool execute: {}({})", self.name(), args);
            let parsed_args: T::Params = serde_json::from_value(args)
                .map_err(|e| Error::Serde(format!("Unable to parse params for {}: {}", self.name(), e)))?;

            let output = self.execute(parsed_args).await
                .map_err(|e| Error::Tool(format!("Tool {} execute error: {}", self.name(), e)))?;
            let val = serde_json::to_value(output)
                .map_err(|e| Error::Tool(format!("Tool {} output parse error: {}", self.name(), e)))?;
            Ok(val)
        })
    }
}

pub struct AgentTools{
    web_search: bool,
    custom_tools: HashMap<String, Arc<dyn ToolDefinitionDyn + Sync + Send>>, 
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
        self.custom_tools.insert(search_tool.name(), Arc::new(search_tool));
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

    pub async fn execute_collected_requests(&mut self) -> Result<()> {
        let mut handles = Vec::new();
        for tool_req in &self.execution_requests {
            // get the tool requested in FunctionToolCall
            if let Some(tool) = self.custom_tools.get(&tool_req.name){
                // concurrent execution
                let tool_clone = tool.clone();
                let req_clone = tool_req.clone();
                handles.push(tokio::spawn(async move {
                    // unpack arguments
                    let args: Value = serde_json::from_str(&req_clone.arguments)?;
                    // execute tool, return result
                    tool_clone.tool_execute(args).await
                }));
            } 
            else {
                error!("Tool {} not found", tool_req.name);
            }
        }
        for h in handles {
            match h.await{
                Ok(result) => match result {
                    Ok(result) => {
                        info!("FunctionToolCall returned: {}", result);
                        // return InputItem::Item::FunctionCallOutput 
                        // with the id's from the FunctionToolCall
                        todo!();
                    },
                    Err(err) =>  error!("FunctionToolCall error: {}", err),
                },
                Err(err) => error!("Join error for FunctionToolCall: {}", err),
            }
        }
        Ok(())
    }


}

