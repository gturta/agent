use async_openai::{error::{OpenAIError}, types::responses::WebSearchToolArgsError};

#[derive(Debug)]
pub enum Error{
    Generic(String),
    Unimplemented(String),
    OpenAI(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            Error::Generic(text) => write!(f, "Generic error: {}", text),
            Error::Unimplemented(text) => write!(f, "Unimplemented error: {}", text),
            Error::OpenAI(text) => write!(f, "OpenAI error: {}", text),
        }
            }
}

impl From<OpenAIError> for Error {
    fn from(value: OpenAIError) -> Self {
        Self::OpenAI(format!("OpenAIError: {:?}", value))
    }
}

impl From<WebSearchToolArgsError> for Error {
    fn from(value: WebSearchToolArgsError) -> Self {
        Self::OpenAI(format!("WebSearchToolArgsError: {:?}", value))
    }
}

