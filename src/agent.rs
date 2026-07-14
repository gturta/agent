use async_openai::{Client, config::{Config, OpenAIConfig},
    types::responses::*
};
use tracing::{error, warn, info, debug};
use crate::error::{Error, Result};
mod memory;
mod tools;
use memory::AgentMemory;
use tools::AgentTools;


pub struct Agent<C: Config>{
    client: Client<C>,
    model: String,
    memory: AgentMemory,
    tools: AgentTools,
    max_loops: u32,
    temp_response: String,
}
impl Agent<OpenAIConfig>{
    pub fn build() -> Result<Self> {
        let client = Client::new();
        let model = std::env::var("LLM_MODEL")
            .map_err(|err|Error::Generic(format!("LLM_MODEL variable not defined: {:?}", err)))?; 
        // build tools
        let tools = AgentTools::new()
            .with_web_search(false)
            .build();
        let memory = AgentMemory::new();
        Ok(Agent{
            client,
            model,
            memory,
            tools,
            max_loops: 3,
            temp_response: String::new()
        })
    }
}
impl<C: Config> Agent<C>{

    pub async fn ask_one(&mut self, question: &str) -> Result<String> {
        // add user question to history
        self.memory.add_item(InputItem::EasyMessage(EasyInputMessageArgs::default()
                .r#type(MessageType::Message)
                .role(Role::User)
                .content(EasyInputContent::Text(question.to_string()))
                .build()?));
        
        let out_text = self.process_one_request().await?;
        Ok(out_text)
    }

    async fn process_one_request(&mut self) -> Result<String> {
        let mut done = false;
        let mut current_iteration = 0;
        while !done {
            done = true;
            // 1. build request
            let request = self.build_request()?;
            // 2. send request
            let response = self.send_request(request).await?;
            // 3. process output
            self.process_response(response)?;
            // 4. execute tools
            self.execute_tools().await?;
            
            current_iteration += 1;
            if current_iteration > self.max_loops {
                warn!("Agent loop iteration reached max loops, exiting");
                break;
            }
        }
        Ok(self.build_final_output())
    }

    fn process_response(&mut self, response: Response) -> Result<()> {
        self.temp_response.clear();
        // update memory from response
        self.memory.history_from_response(&response);
        // Process Response.output
        for output_item in response.output {
            match self.process_output_item(&output_item) {
                Ok(text) => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        self.temp_response.push_str(&format!("{}\n{}\n", "-".repeat(10), trimmed));
                    }
                },
                Err(err) => error!("Error processing output item: {:?}", err),
            }
        }

        // check for incomplete details
        if let Some(incomplete) = response.incomplete_details {
            let incomplete_text = format!("\n{}\nIncomplete answer reason: {}",
                "-".repeat(10), incomplete.reason);
            self.temp_response.push_str(&incomplete_text);
        }
        
        // add usage info
        if let Some(usage) = response.usage {
            let usage_txt = format!("\n{}\nUsage total: {}, input: {}, output: {}",
                "-".repeat(10), usage.total_tokens, usage.input_tokens, usage.output_tokens);
            self.temp_response.push_str(&usage_txt);
        }
        Ok(())
    }

    fn build_final_output(&mut self) -> String {
        self.temp_response.clone()
    }

    fn build_request(&mut self) -> Result<CreateResponse> {
        let mut request = CreateResponseArgs::default()
            .model(&self.model)
            .reasoning(Reasoning { effort: Some(ReasoningEffort::Low), summary: Some(ReasoningSummary::Auto) })
            .tools(self.tools.get_tools())
            .build()?;
        // add input 
        self.memory.history_to_request(&mut request);
        debug!("OpenAI Request: {:#?}", request);
        Ok(request)
    }

    async fn send_request(&mut self, request: CreateResponse) -> Result<Response> {
        let response = self.client.responses()
            .create(request)
            .await?;
        debug!("OpenAI Response: {:#?}", response);
        Ok(response)
    }

    fn process_output_item(&mut self, item: &OutputItem) -> Result<String> {
        let mut output = String::new();
        match item {
            OutputItem::Message(output_message) => output.push_str(&self.output_message_text(output_message)),
            OutputItem::WebSearchCall(web_search_tool_call) => self.handle_web_search_call(web_search_tool_call),
            OutputItem::Reasoning(reasoning) => self.handle_reasoning(reasoning),
            OutputItem::FunctionCall(function_call) => self.handle_function_call(function_call),
            other => info!("Unimplemented: {:?}", other),
        }
        Ok(output)
    }

    fn output_message_text(&self, message: &OutputMessage) -> String {
        let mut output = String::new();
        for content in &message.content {
            output.push_str(match content {
                    OutputMessageContent::OutputText(output_text_content) => &output_text_content.text,
                    OutputMessageContent::Refusal(refusal_content) => &refusal_content.refusal,
                });
        }
        output
    }
    fn handle_web_search_call(&self, web_search_tool_call: &WebSearchToolCall) {
        if let Some(action) = &web_search_tool_call.action {
            let buf = match action {
                WebSearchToolCallAction::Search(action) => format!("Search ({})", action.query),
                WebSearchToolCallAction::OpenPage(action) => format!("OpenPage ({})", 
                    action.url.clone().unwrap_or_default()),
                WebSearchToolCallAction::Find(action) => format!("Find ({} : {})", 
                    action.url, action.pattern),
                WebSearchToolCallAction::FindInPage(action) => format!("FindInPage {} : {}", 
                    action.url, action.pattern),
            };
            info!("WebSearchToolCall: {}", buf);
        }
    }
    fn handle_reasoning(&self, reasoning: &ReasoningItem) {
        let mut buf = String::new();
        for SummaryPart::SummaryText(SummaryTextContent{text}) in &reasoning.summary{
            buf.push_str(&format!("summary: {text}\n"));
        }
        if let Some(content) = &reasoning.content {
            for ReasoningItemContent::ReasoningText(ReasoningTextContent{text}) in content {
                buf.push_str(&format!("summary: {text}\n"));
            }
        }
        if !buf.trim().is_empty(){
            info!("===== REASONING START =====\n{buf}===== REASONING END ======");
        }
    }

    fn handle_function_call(&mut self, function_call: &FunctionToolCall) {
        info!("Received FunctionToolCall request for {}", function_call.name);
        self.tools.collect_execution_request(function_call);
    }
    async fn execute_tools(&mut self) -> Result<()> {
        let _= self.tools.execute_collected_requests().await;
        Ok(())
    }
}

