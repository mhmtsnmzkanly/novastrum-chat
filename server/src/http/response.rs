use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiState {
    Ok,
    Error,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T>
where
    T: Serialize,
{
    pub state: ApiState,
    #[serde(rename = "type")]
    pub response_type: &'static str,
    pub req: Option<String>,
    pub data: T,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn ok(response_type: &'static str, data: T) -> Self {
        Self {
            state: ApiState::Ok,
            response_type,
            req: None,
            data,
        }
    }

    pub fn error(response_type: &'static str, data: T) -> Self {
        Self {
            state: ApiState::Error,
            response_type,
            req: None,
            data,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiErrorPayload {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<serde_json::Value>,
}

impl ApiErrorPayload {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            fields: None,
        }
    }

    pub fn with_fields(
        code: &'static str,
        message: impl Into<String>,
        fields: serde_json::Value,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            fields: Some(fields),
        }
    }
}
