use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Person {
    pub alias: String,
}

/// message that Recieved
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub alias: Person,
    pub content: String,
    pub datetime: String,
}

impl Display for Person {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.alias)
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}\n{}\n", self.alias, self.datetime, self.content)
    }
}