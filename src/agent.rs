use async_openai::{Client, config::{Config, OpenAIConfig},
    types::responses::*
};
use tracing::{error, info, debug};
use crate::error::Error;

type Result<T> = std::result::Result<T, Error>;

pub struct Agent<C: Config>{
    client: Client<C>,
    web_search: bool,
    preserve_history: bool,
}
impl Agent<OpenAIConfig>{
    pub fn new() -> Self {
        let client = Client::new();
        Agent{
            client,
            web_search: false,
            preserve_history: false,
        }
    }
}
impl<C: Config> Agent<C>{

    pub async fn ask_one(&self, question: &str) -> Result<String> {
        let model = std::env::var("LLM_MODEL")
            .map_err(|err|Error::Generic(format!("LLM_MODEL variable not defined: {:?}", err)))?; 
        // build input 
        let input = self.build_input(question)?;
        // build tools
        let tools = self.build_tools()?;
        let request = CreateResponseArgs::default()
            .model(model)
            .input(input)
            .tools(tools)
            .build()?;
        debug!("OpenAI Request: {:#?}", request);
        let response = self.client.responses()
            .create(request)
            .await?;
        debug!("OpenAI Response: {:#?}", response);
        let mut out_text = String::new();
        for output_item in response.output {
            match self.process_output_item(&output_item) {
                Ok(maybe_text) => {
                    if let Some(text) = maybe_text{
                        out_text.push_str(&format!("{}\n{}\n", "-".repeat(10), text));
                    }
                },
                Err(err) => {
                    match err {
                        Error::Unimplemented(feature) => info!("Unimplemented: {}", feature),
                        _ => error!("Error processing output item: {:?}", err),
                    }
                },
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

    pub fn with_web_search(&mut self, enable: bool) {
        self.web_search = enable;
    }

    fn build_tools(&self) -> Result<Vec<Tool>> {
        let mut tools = Vec::new();
        if self.web_search {
            tools.push(Tool::WebSearch(WebSearchToolArgs::default().build()?));
        }
        Ok(tools)
    }

    fn build_input(&self, input_text: &str) -> Result<InputParam> {
        let items = vec![
            // add user message
            InputItem::EasyMessage(EasyInputMessageArgs::default()
                .r#type(MessageType::Message)
                .role(Role::User)
                .content(EasyInputContent::Text(input_text.to_string()))
                .build()?
            )];
        
        Ok(InputParam::Items(items))
    }

    fn process_output_item(&self, item: &OutputItem) -> Result<Option<String>> {
        match item {
            OutputItem::Message(output_message) => Ok(Some(self.output_message_text(output_message))),
            OutputItem::FileSearchCall(_file_search_tool_call) => Err(Error::Unimplemented("FileSearchCall".to_owned() )),
            OutputItem::FunctionCall(_function_tool_call) => Err(Error::Unimplemented("FunctionCall".to_owned() )),
            OutputItem::FunctionCallOutput(_function_tool_call_output_resource) => Err(Error::Unimplemented("FunctionCallOutput".to_owned(), )),
            OutputItem::WebSearchCall(web_search_tool_call) => {
                self.handle_web_search_call(web_search_tool_call);
                Ok(None)
            },
            OutputItem::ComputerCall(_computer_tool_call) => Err(Error::Unimplemented("ComputerCall".to_owned() )),
            OutputItem::ComputerCallOutput(_computer_tool_call_output_resource) => Err(Error::Unimplemented("ComputerCallOutput".to_owned(), )),
            OutputItem::Reasoning(_reasoning_item) => Err(Error::Unimplemented("Reasoning".to_owned() )),
            OutputItem::Compaction(_compaction_body) => Err(Error::Unimplemented("Compaction".to_owned(), )),
            OutputItem::ImageGenerationCall(_image_gen_tool_call) => Err(Error::Unimplemented("ImageGenerationCall".to_owned() )),
            OutputItem::CodeInterpreterCall(_code_interpreter_tool_call) => Err(Error::Unimplemented("CodeInterpreterCall".to_owned(), )),
            OutputItem::LocalShellCall(_local_shell_tool_call) => Err(Error::Unimplemented("LocalShellCall".to_owned() )),
            OutputItem::ShellCall(_function_shell_call) => Err(Error::Unimplemented("ShellCall".to_owned(), )),
            OutputItem::ShellCallOutput(_function_shell_call_output) => Err(Error::Unimplemented("ShellCallOutput".to_owned() )),
            OutputItem::ApplyPatchCall(_apply_patch_tool_call) => Err(Error::Unimplemented("ApplyPatchCall".to_owned(), )),
            OutputItem::ApplyPatchCallOutput(_apply_patch_tool_call_output) => Err(Error::Unimplemented("ApplyPatchCallOutput".to_owned() )),
            OutputItem::McpCall(_mcptool_call) => Err(Error::Unimplemented("McpCall".to_owned() )),
            OutputItem::McpListTools(_mcplist_tools) => Err(Error::Unimplemented("McpListTools".to_owned() )),
            OutputItem::McpApprovalRequest(_mcpapproval_request) => Err(Error::Unimplemented("McpApprovalRequest".to_owned() )),
            OutputItem::CustomToolCall(_custom_tool_call) => Err(Error::Unimplemented("CustomToolCall".to_owned(), )),
            OutputItem::CustomToolCallOutput(_custom_tool_call_output_resource) => Err(Error::Unimplemented("CustomToolCallOutput".to_owned() )),
            OutputItem::ToolSearchCall(_tool_search_call) => Err(Error::Unimplemented("ToolSearchCall".to_owned() )),
            OutputItem::ToolSearchOutput(_tool_search_output) => Err(Error::Unimplemented("ToolSearchOutput".to_owned() )),
        }
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
        info!("WebSearch call: \n{:?}\n", web_search_tool_call);
    }

    pub fn preserve_history(&mut self, preserve: bool) {
        self.preserve_history = preserve;
    }
}

