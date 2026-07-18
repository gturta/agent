    use std::sync::Arc;
    use std::collections::HashMap;
    use crate::agent::tools::{ToolDefinitionDyn, ToolDefinition, ToolProvider};
    use doc_search_tool::DocSearchTool;
    mod doc_search_tool;

    pub struct LocalTools {
        custom_tools: HashMap<String, Arc<dyn ToolDefinitionDyn + Sync + Send>>, 
    }

    impl LocalTools {
        pub fn build() -> Self {
            let mut custom_tools: HashMap<String, Arc<dyn ToolDefinitionDyn + Sync + Send>> = HashMap::new(); 
            let search_tool = DocSearchTool{};
            custom_tools.insert(search_tool.name(), Arc::new(search_tool));
            Self {
                custom_tools,
            }
        }
    }

    impl ToolProvider for LocalTools {
        fn name(&self) -> String {
            "local".to_string()
        }

        fn tool_list(&self) -> Vec<Arc<dyn ToolDefinitionDyn + Send + Sync>> {
            self.custom_tools.values().cloned().collect()
        }

        fn get(&self, function_name: &str) -> Option<Arc<dyn ToolDefinitionDyn + Send + Sync>> {
            self.custom_tools.get(function_name).cloned()
        }
    }
