use rig_agent::agent::StreamingError;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserFacingError {
    message: String,
    retryable: bool,
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct ToolInputError(pub &'static str);

impl UserFacingError {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("模型流式请求失败")]
    ModelRequest {
        #[source]
        source: StreamingError,
    },
    #[error("模型流式响应未返回结束事件")]
    MissingFinalResponse,
}

impl AppError {
    pub fn user_facing(&self) -> UserFacingError {
        let (message, retryable) = match self {
            Self::ModelRequest { .. } | Self::MissingFinalResponse => (
                "请求失败：模型请求失败，请检查网络、模型配置和服务状态后重试。",
                true,
            ),
        };

        UserFacingError {
            message: message.to_owned(),
            retryable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_facing_errors_hide_internal_details() {
        let error = AppError::MissingFinalResponse.user_facing();
        assert!(error.retryable());
    }
}
