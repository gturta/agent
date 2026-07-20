use std::sync::Arc;
use std::collections::HashMap;
use tokio::process::Command;
use rmcp::{RoleClient, transport::{StreamableHttpClientTransport, child_process::TokioChildProcess}, service::{ServiceExt, RunningService}, model::CallToolRequestParams};
use anyhow::{anyhow, Result};
use tracing::{info, error, debug, warn};
use serde::Deserialize;
use crate::{agent::tools::{ToolDefinitionDyn, ToolProvider} };

#[derive(Deserialize)]
pub struct McpConfig {
    pub transport: McpTransport,
}
#[derive(Deserialize)]
pub enum McpTransport{
    Stdio{
        command: String,
        args: Vec<String>,
    },
    Http{
        uri: String
    },
}
pub struct McpTools {
    name: String,
    tools: HashMap<String, Arc<McpTool>>, 
    service: Option<Arc<RunningService<RoleClient, ()>>>,
}

impl McpTools {
    pub async fn build(config: McpConfig) -> Result<Self> {
        let service = match config.transport {
            McpTransport::Stdio { command, args } => {
                let mut cmd = Command::new(command);
                cmd.args(args);
                let transport = TokioChildProcess::new(cmd)?;
                ().serve(transport).await?
            },
            McpTransport::Http { uri } => {
                let transport = StreamableHttpClientTransport::from_uri(uri);
                ().serve(transport).await?
            },
        };
        let server_name = match service.peer_info() {
            Some(info) => info.server_info.name.clone(),
            None => "unnamed".to_string(),
        };
        info!("Querying MCP server {}", server_name);
        let service = Arc::new(service);
        let mut tools = HashMap::new();
        let mcp_tools = service.list_all_tools().await?;
        for mcp_tool in mcp_tools {
            let tool = McpTool::from_mcp_tool(mcp_tool, service.clone());
            tools.insert(tool.tool_name().to_string(), Arc::new(tool));
        }
        Ok(McpTools {
            name: server_name,
            tools,
            service: Some(service),
        })
    }

}

impl ToolProvider for McpTools {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn tool_list(&self) -> Vec<Arc<dyn ToolDefinitionDyn + Send + Sync>> {
        self.tools.values()
            .map(|v| v.clone() as Arc<dyn ToolDefinitionDyn + Send + Sync> )
            .collect()
    }

    fn get(&self, function_name: &str) -> Option<Arc<dyn ToolDefinitionDyn + Send + Sync>> {
        let tool = self.tools.get(function_name).cloned();
        tool.map(|a| a as Arc<dyn ToolDefinitionDyn + Send + Sync>)
    }

    fn cleanup(&mut self) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
        info!("McpTools cleanup called");
        if let Some(service) = self.service.take()
            && let Ok(service) = Arc::try_unwrap(service) {
                info!("Closing MCP service connection");
                return Box::pin(async {
                    let _ = service.cancel().await;
                });
        }
        Box::pin(std::future::ready(()))
    }
}

struct McpTool {
    name: String,
    description: String,
    params: serde_json::Value,
    service: Arc<RunningService<RoleClient, ()>>,
}
impl McpTool {
    pub fn from_mcp_tool(tool: rmcp::model::Tool, service: Arc<RunningService<RoleClient, ()>>) -> Self {
        McpTool{
            name: tool.name.to_string(),
            description: tool.description.unwrap_or_default().to_string(),
            params: serde_json::json!(tool.input_schema),
            service
        }
    }
}
impl ToolDefinitionDyn for McpTool {
    fn tool_name(&self) -> &str {
        &self.name
    }

    fn tool_description(&self) -> &str {
        &self.description
    }

    fn tool_parameters(&self) -> &serde_json::Value {
        &self.params
    }

    fn tool_execute(&self, args: serde_json::Value) -> std::pin::Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send + '_ >> {
        let name = self.tool_name().to_owned();
        info!("Executing MCP tool: {}", name);
        debug!("Call MCPT tool {} with params: {}", name, args);
        Box::pin(async move {
            match args {
                serde_json::Value::Object(args_map) => {
                    let params = CallToolRequestParams::new(name.clone())
                        .with_arguments(args_map);
                    let result = self.service.call_tool(params).await?;
                    let Ok(output) = serde_json::to_value(result) else {
                        error!("MCP tool {} returned invalid output", name);
                        return Err(anyhow!("MCP invalid result"));
                    };
                    debug!("MCP tool {} returned: {}", name, output);
                    Ok(output)
                },
                other => {
                    error!("MCP tool {} arguments are invalid: {}", name, other);
                    Err(anyhow!("MCP invalid arguments"))
                }
            }
        })
    }
}
