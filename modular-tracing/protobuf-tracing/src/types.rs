use prost::*;
use std::collections::HashMap;

#[derive(Message)]
pub struct Record {
    #[prost(string, tag = "1")]
    pub message: String,

    #[prost(string, tag = "2")]
    pub target: String,

    #[prost(string, tag = "3")]
    pub level: String,

    #[prost(string, tag = "4", optional)]
    pub file: Option<String>,

    #[prost(uint32, tag = "5", optional)]
    pub line: Option<u32>,

    #[prost(hash_map = "string, message", tag = "6")]
    pub fields: HashMap<String, Values>,

    #[prost(message, repeated, tag = "7")]
    pub spans: Vec<Span>,

    #[prost(string, tag = "8", optional)]
    pub thread: Option<String>,

    #[prost(string, tag = "10")]
    pub timestamp: String,
}

#[derive(Message)]
pub struct Span {
    #[prost(string, tag = "1")]
    pub name: String,

    #[prost(string, tag = "2")]
    pub target: String,

    #[prost(string, tag = "3")]
    pub level: String,

    #[prost(string, tag = "4", optional)]
    pub file: Option<String>,

    #[prost(uint32, tag = "5", optional)]
    pub line: Option<u32>,
}

#[derive(Message, PartialEq)]
pub struct Values {
    #[prost(message, repeated, tag = "1")]
    pub values: Vec<Value>,
}

#[derive(Message, PartialEq)]
pub struct Value {
    #[prost(oneof = "ValueType", tags = "1, 2, 3, 4, 5, 6, 7, 8")]
    pub value: Option<ValueType>,
}

#[derive(Oneof, PartialEq)]
pub enum ValueType {
    #[prost(double, tag = "1")]
    F64(f64),
    #[prost(int64, tag = "2")]
    I64(i64),
    #[prost(uint64, tag = "3")]
    U64(u64),
    #[prost(bytes, tag = "4")]
    U128(Vec<u8>),
    #[prost(bytes, tag = "5")]
    I128(Vec<u8>),
    #[prost(bool, tag = "6")]
    Bool(bool),
    #[prost(string, tag = "7")]
    String(String),
    #[prost(string, tag = "8")]
    Error(String),
}
