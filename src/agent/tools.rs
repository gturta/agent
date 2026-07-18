use std::sync::Arc;
use std::pin::Pin;
use std::future::Future;
use std::collections::HashMap;
use async_openai::types::responses::{Tool, FunctionToolArgs, FunctionToolCall, WebSearchToolArgs, FunctionCallOutputItemParam, FunctionCallOutput, OutputStatus};
use serde_json::{Value,json};
use serde::{de::DeserializeOwned, Serialize};
use schemars::{JsonSchema, schema_for};
use tracing::{info, error, warn};

use crate::error::{Error, Result};
mod local; 
use local::LocalTools;

pub trait ToolDefinition{
    type Params;
    type Output;
    // required functions
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn execute(&self, args: Self::Params) -> impl std::future::Future<Output = Result<Self::Output>> + Send;
}

trait ToolDefinitionDyn: Send + Sync{
    // required functions
    fn tool_name(&self) -> String;
    fn tool_description(&self) -> String;
    fn tool_parameters(&self) -> Value;
    fn tool_execute(&self, args: Value) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + '_ >>;
}
impl<T> ToolDefinitionDyn for T 
    where 
        T: ToolDefinition + Send + Sync,
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
    fn tool_execute(&self, args: Value) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + '_ >> {
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

trait ToolProvider {
    fn name(&self) -> String;
    fn tool_list(&self) -> Vec<Arc<dyn ToolDefinitionDyn + Send + Sync>> ; 
    fn get(&self, function: &str) -> Option<Arc<dyn ToolDefinitionDyn + Send + Sync>>;
}

pub struct AgentTools{
    web_search: bool,
    execution_requests: Vec<FunctionToolCall>,
    providers: HashMap<String, Arc<dyn ToolProvider>>,
}

impl AgentTools{
    pub fn new() -> Self{
        Self{
            web_search: false,
            execution_requests: Vec::new(),
            providers: HashMap::new(),
        }
    }
    fn add_provider(&mut self, provider: impl ToolProvider + 'static) {
        self.providers.insert(provider.name(), Arc::new(provider));
    }

    pub fn build(mut self) -> Self {
        let local = LocalTools::build();
        self.add_provider(local);
        self
    }
    
    pub fn get_tools(&self) -> Vec<Tool> {
        let mut tools = Vec::new();
        for provider in self.providers.values() {
            // add custom FuntionTools
            for custom_tool in provider.tool_list(){
                // create OpenAI FunctionTool
                let qualified_name = format!("{}__{}", provider.name(), custom_tool.tool_name());
                if let Ok(tool) = FunctionToolArgs::default()
                    // the tool name will be prefixed with the provider
                    .name(&qualified_name)
                        .description(custom_tool.tool_description())
                        .parameters(custom_tool.tool_parameters())
                        .build(){
                            info!("Adding tool {}", qualified_name);
                            tools.push(Tool::Function(tool));
                }
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

    pub async fn execute_collected_requests(&mut self) -> Result<Vec<FunctionCallOutputItemParam>> {
        let mut handles = Vec::new();

        // 1. spawn tasks for each FunctionToolCall request
        for tool_req in &self.execution_requests {
            // as I have multiple tool providers (local, mcp) the function name is
            // "provider_name/function_name"
            if let Some(split_at) = tool_req.name.find("__"){
                let provider_name = &tool_req.name[..split_at];
                let function_name = &tool_req.name[split_at+2..];
                if let Some(provider) = self.providers.get(provider_name) {
                    // get the tool requested in FunctionToolCall
                    if let Some(tool) = provider.get(function_name){
                        // concurrent execution
                        let tool_clone = tool.clone();
                        let req_clone = tool_req.clone();
                        handles.push(tokio::spawn(async move {
                            // unpack arguments
                            let args: Value = serde_json::from_str(&req_clone.arguments)?;
                            // execute tool, return result
                            match tool_clone.tool_execute(args).await{
                                Ok(result) => {
                                    // pack the result 
                                    Ok(FunctionCallOutputItemParam{
                                        call_id: req_clone.call_id,
                                        output: FunctionCallOutput::Text(result.to_string()),
                                        id: None,
                                        status: Some(OutputStatus::Completed),
                                    })
                                },
                                Err(err) => Err(err),
                            }
                        }));
                    } 
                    else {
                        warn!("Tool {} not found", tool_req.name);
                    }

                } else {
                    warn!("Provider {} not registered", provider_name);
                }
            }
        }

        // 2. collect results from spawned tasks
        let mut output = Vec::new();
        for h in handles {
            match h.await{
                Ok(result) => match result {
                    Ok(result) => {
                        info!("FunctionToolCall returned: {:?}", result);
                        // return InputItem::Item::FunctionCallOutput 
                        // with the id's from the FunctionToolCall
                        output.push(result);
                    },
                    Err(err) =>  error!("FunctionToolCall error: {}", err),
                },
                Err(err) => error!("Join error for FunctionToolCall: {}", err),
            }
        }
        Ok(output)
    }


}

