use async_openai::{error::{OpenAIError}, 
    types::responses::{WebSearchToolArgsError, FunctionToolArgsError}
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error{
    Generic(String),
    OpenAI(String),
    Serde(String),
    Tool(String)
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            Error::Generic(text) => write!(f, "Generic error: {}", text),
            Error::OpenAI(text) => write!(f, "OpenAI error: {}", text),
            Error::Serde(text) => write!(f, "Serde error: {}", text),
            Error::Tool(text) => write!(f, "Tool error: {}", text),

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
impl From<FunctionToolArgsError> for Error {
    fn from(value: FunctionToolArgsError) -> Self {
        Self::OpenAI(format!("FunctionToolArgsError: {:?}", value))
    }
}
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(format!("SerdeJson: {:?}", value))
    }
}
