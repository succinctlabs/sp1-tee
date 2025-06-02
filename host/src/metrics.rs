use metrics::{counter, describe_gauge, gauge};
use strum::{Display, EnumIter, EnumMessage, IntoEnumIterator};

#[derive(Debug, Display, EnumIter, EnumMessage)]
pub enum HostMetrics {
    #[strum(serialize = "sp1_tee_executions", message = "")]
    TeeExecutionCounter,
    #[strum(serialize = "sp1_tee_running_enclaves")]
    RunningEnclaveGauge,
    #[strum(serialize = "sp1_tee_execution_errors")]
    TeeExecutionErrorCounter,
}

impl HostMetrics {
    pub fn register() {
        for metric in Self::iter() {
            if let Some(message) = metric.get_message() {
                describe_gauge!(metric.to_string(), message);
            }
        }
    }

    pub fn increment(&self) {
        match self {
            HostMetrics::TeeExecutionCounter => counter!(self.to_string()).increment(1),
            HostMetrics::RunningEnclaveGauge => gauge!(self.to_string()).increment(1),
            HostMetrics::TeeExecutionErrorCounter => counter!(self.to_string()).increment(1),
        }
    }
}
