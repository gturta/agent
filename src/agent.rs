use async_openai::{Client, config::{Config, OpenAIConfig},
    types::responses::*
};
use tracing::{error, info, debug};
use crate::error::Error;
mod memory;
mod tools;
use memory::AgentMemory;
use tools::AgentTools;


type Result<T> = std::result::Result<T, Error>;

pub struct Agent<C: Config>{
    client: Client<C>,
    memory: AgentMemory,
    tools: AgentTools,
}
impl Agent<OpenAIConfig>{
    pub fn new() -> Self {
        let client = Client::new();
        Agent{
            client,
            memory: AgentMemory::new(),
            tools: AgentTools::new(),
        }
    }
}
impl<C: Config> Agent<C>{

    pub async fn ask_one(&mut self, question: &str) -> Result<String> {
        let model = std::env::var("LLM_MODEL")
            .map_err(|err|Error::Generic(format!("LLM_MODEL variable not defined: {:?}", err)))?; 
        // build tools
        self.tools.build();
        let tools = self.tools.get_tools();
        let mut request = CreateResponseArgs::default()
            .model(model)
            .reasoning(Reasoning { effort: Some(ReasoningEffort::Low), summary: Some(ReasoningSummary::Auto) })
            .tools(tools)
            .build()?;
        // build input 
        self.build_input(question, &mut request)?;
        debug!("OpenAI Request: {:#?}", request);
        let response = self.client.responses()
            .create(request)
            .await?;
        debug!("OpenAI Response: {:#?}", response);
        let mut out_text = String::new();
        // update memory from response
        self.memory.history_from_response(&response);
        for output_item in response.output {
            match self.process_output_item(&output_item) {
                Ok(text) => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        out_text.push_str(&format!("{}\n{}\n", "-".repeat(10), trimmed));
                    }
                },
                Err(err) => error!("Error processing output item: {:?}", err),
            }
        }

        // check for incomplete details
        if let Some(incomplete) = response.incomplete_details {
            let incomplete_text = format!("\n{}\nIncomplete answer reason: {}",
                "-".repeat(10), incomplete.reason);
            out_text.push_str(&incomplete_text);
        }
        
        // add usage info
        if let Some(usage) = response.usage {
            let usage_txt = format!("\n{}\nUsage total: {}, input: {}, output: {}",
                "-".repeat(10), usage.total_tokens, usage.input_tokens, usage.output_tokens);
            out_text.push_str(&usage_txt);
        }
        Ok(out_text)
    }


    fn build_input(&mut self, input_text: &str, request: &mut CreateResponse) -> Result<()> {
        let msg = InputItem::EasyMessage(EasyInputMessageArgs::default()
                .r#type(MessageType::Message)
                .role(Role::User)
                .content(EasyInputContent::Text(input_text.to_string()))
                .build()?);
        // add user message to history
        self.memory.add_item(msg);
        // then add history to request
        self.memory.history_to_request(request);
        Ok(())
    }

    fn process_output_item(&self, item: &OutputItem) -> Result<String> {
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

    fn handle_function_call(&self, function_call: &FunctionToolCall) {
        info!("Calling function {}", function_call.name);

        match self.tools.execute_function_call(function_call){
            Ok(value) => info!("Function call {} returned: {}", function_call.name, value),
            Err(err) => error!("Error execute function call: {}", err),
        };
    }

}

