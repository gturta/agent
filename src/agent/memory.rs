
pub struct AgentMemory<T>{
    items: Vec<T>
}
impl<T:Clone> AgentMemory<T> {
    pub fn new() -> Self{
        AgentMemory{
            items: Vec::new(),
        }
    }

    pub fn add_item<I: Into<T>+Clone>(&mut self, item: &I) {
        let clone = (*item).clone();
        self.items.push(clone.into());
    }
    pub fn add_items(&mut self, items: &[T]) {
        let mut clones = Vec::from(items);
        self.items.append(&mut clones);
    }
    pub fn get_items(&self) -> Vec<T> {
        self.items.clone()
    }
}

