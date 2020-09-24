pub(crate) mod crash_loop_backoff;
pub(crate) mod error;
pub(crate) mod finished;
pub(crate) mod image_pull;
pub(crate) mod image_pull_backoff;
pub(crate) mod registered;
pub(crate) mod running;
pub(crate) mod starting;
pub(crate) mod terminated;
pub(crate) mod volume_mount;



/// When called in a state's `next` function, exits the current state
/// and transitions to the Error state.
#[macro_export]
macro_rules! transition_to_error {
    ($slf:ident, $err:ident) => {{
        let aerr = anyhow::Error::from($err);
        log::error!("{:?}", aerr);
        let error_state = super::error::Error {
            message: aerr.to_string(),
        };
        return Transition::next($slf, error_state);
    }};
}

/// Match on result and `transition_to_error` if `Err`.
#[macro_export]
macro_rules! unwrap_result {
    ($slf:ident, $res:expr) => {{
        match $res {
            Ok(t) => t,
            Err(e) => transition_to_error!($slf, e) 
        }
    }};
}


/// When called in a state's `next` function, exits the state machine
/// returns a fatal error to the kubelet.
#[macro_export]
macro_rules! fail_fatal {
    ($err:ident) => {{
        let aerr = anyhow::Error::from($err);
        log::error!("{:?}", aerr);
        return Transition::Fatal(aerr);
    }};
}
