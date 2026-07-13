use async_openai::types::responses::{CreateResponse, InputParam, InputItem, Response};

pub struct AgentMemory{
    items: Vec<InputItem>
}
impl AgentMemory {
    pub fn new() -> Self{
        AgentMemory{
            items: Vec::new(),
        }
    }

    pub fn add_item(&mut self, item: InputItem) {
        self.items.push(item);
    }
    pub fn history_to_request(&self, request: &mut CreateResponse) {
        request.input = InputParam::Items(self.items.clone());
    }
    pub fn history_from_response(&mut self, response: &Response) {
        for output_item in &response.output {
            self.add_item(output_item.clone().into());
        }
    }
}

