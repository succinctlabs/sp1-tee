use metrics::{counter, describe_counter};
use sp1_sdk::network::tee::api::TEEResponse;
use strum::{Display, EnumIter, EnumMessage, IntoEnumIterator};

use crate::server::ServerError;

#[derive(Debug, Display, EnumIter, EnumMessage)]
pub enum HostMetric {
    #[strum(serialize = "executions_count", message = "Total number of executions")]
    Execution,

    #[strum(
        serialize = "execution_errors_count",
        message = "Total number of execution errors"
    )]
    ExecutionError,

    #[strum(
        serialize = "unexpected_errors_count",
        message = "Total number of unexpected response errors"
    )]
    UnexpectedResponse,

    #[strum(
        serialize = "stdin_too_large",
        message = "Total number of stdin too large errors"
    )]
    StdinTooLarge,

    #[strum(
        serialize = "program_too_large",
        message = "Total number of program too large errors"
    )]
    ProgramTooLarge,
}

impl HostMetric {
    pub fn register() {
        for metric in Self::iter() {
            if let Some(message) = metric.get_message() {
                describe_counter!(metric.to_string(), message);
            }
        }
    }

    pub fn increment(&self) {
        counter!(self.to_string()).increment(1)
    }
}

pub fn emit_response_metric(response: &Result<TEEResponse, ServerError>) {
    if let Err(error) = response {
        match error {
            ServerError::UnexpectedResponseFromEnclave => {
                HostMetric::UnexpectedResponse.increment()
            }
            ServerError::EnclaveError(_) => HostMetric::ExecutionError.increment(),
            ServerError::StdinTooLarge(_) => HostMetric::StdinTooLarge.increment(),
            ServerError::ProgramTooLarge(_) => HostMetric::ProgramTooLarge.increment(),
            _ => (),
        }
    } else {
        HostMetric::Execution.increment();
    }
}
