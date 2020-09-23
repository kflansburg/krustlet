//! Used to define a state machine of Pod states.
use log::{debug, error};

pub mod prelude;

use crate::pod::Pod;
use k8s_openapi::api::core::v1::Pod as KubePod;
use kube::api::{Api, PatchParams};

/// Holds arbitrary State objects in Box, and prevents manual construction of Transition::Next
///
/// ```compile_fail
/// use kubelet::state::{Transition, StateHolder, Stub};
///
/// struct PodState;
///
/// // This fails because `state` is a private field. Use Transition::next classmethod instead.
/// let _transition = Transition::<PodState>::Next(StateHolder {
///     state: Box::new(Stub),
/// });
/// ```
pub struct StateHolder<PodState> {
    // This is private, preventing manual construction of Transition::Next
    state: Box<dyn State<PodState>>,
}

/// Represents result of state execution and which state to transition to next.
pub enum Transition<PodState> {
    /// Transition to new state.
    Next(StateHolder<PodState>),
    /// Stop executing the state machine and report the result of the execution.
    Complete(anyhow::Result<()>),
    /// Stop executing the state machine and report that the state machine
    /// failed.
    // TODO: is this really any different from Complete(Err(...))?
    Fatal(anyhow::Error),
}

/// Mark an edge exists between two states.
pub trait TransitionTo<S> {}

impl<PodState> Transition<PodState> {
    // This prevents user from having to box everything AND allows us to enforce edge constraint.
    /// Construct Transition::Next from old state and new state.
    /// Both states must be State<PodState> with matching PodState.
    /// Input state must implement TransitionTo<OutputState>.
    ///
    /// ```
    /// use kubelet::state::{Transition, State, TransitionTo};
    /// use kubelet::pod::Pod;
    ///
    /// #[derive(Debug)]
    /// struct TestState;
    ///
    /// impl TransitionTo<TestState> for TestState {}
    ///
    /// struct PodState;
    ///
    /// #[async_trait::async_trait]
    /// impl State<PodState> for TestState {
    ///     async fn next(
    ///         self: Box<Self>,
    ///         _pod_state: &mut PodState,
    ///         _pod: &Pod,
    ///     ) -> Transition<PodState> {
    ///         Transition::next(self, TestState)
    ///     }
    ///
    ///     async fn json_status(
    ///         &self,
    ///         _pod_state: &mut PodState,
    ///         _pod: &Pod,
    ///     ) -> anyhow::Result<serde_json::Value> {
    ///         Ok(serde_json::json!(null))
    ///     }
    /// }
    /// ```
    ///
    /// The next state must also be State<PodState>, if it is not State, if fails to compile::
    /// ```compile_fail
    /// use kubelet::state::{Transition, State, TransitionTo};
    /// use kubelet::pod::Pod;
    ///
    /// #[derive(Debug)]
    /// struct TestState;
    ///
    /// struct PodState;
    ///
    /// #[derive(Debug)]
    /// struct NotState;
    ///
    /// impl TransitionTo<NotState> for TestState {}
    ///
    /// #[async_trait::async_trait]
    /// impl State<PodState> for TestState {
    ///     async fn next(
    ///         self: Box<Self>,
    ///         _pod_state: &mut PodState,
    ///         _pod: &Pod,
    ///     ) -> anyhow::Result<Transition<PodState>> {
    ///         // This fails because NotState is not State
    ///         Ok(Transition::next(self, NotState))
    ///     }
    ///
    ///     async fn json_status(
    ///         &self,
    ///         _pod_state: &mut PodState,
    ///         _pod: &Pod,
    ///     ) -> anyhow::Result<serde_json::Value> {
    ///         Ok(serde_json::json!(null))
    ///     }
    /// }
    /// ```
    ///
    /// Edges must be defined, even for self-transition, with edge removed, compilation fails:
    /// ```compile_fail
    /// use kubelet::state::{Transition, State};
    /// use kubelet::pod::Pod;
    ///
    /// #[derive(Debug)]
    /// struct TestState;
    ///
    /// // impl TransitionTo<TestState> for TestState {}
    ///
    /// struct PodState;
    ///
    /// #[async_trait::async_trait]
    /// impl State<PodState> for TestState {
    ///     async fn next(
    ///         self: Box<Self>,
    ///         _pod_state: &mut PodState,
    ///         _pod: &Pod,
    ///     ) -> anyhow::Result<Transition<PodState>> {
    ///         // This fails because TestState is not TransitionTo<TestState>
    ///         Ok(Transition::next(self, TestState))
    ///     }
    ///
    ///     async fn json_status(
    ///         &self,
    ///         _pod_state: &mut PodState,
    ///         _pod: &Pod,
    ///     ) -> anyhow::Result<serde_json::Value> {
    ///         Ok(serde_json::json!(null))
    ///     }
    /// }
    /// ```
    ///
    /// The next state must have the same PodState type, otherwise compilation will fail:
    /// ```compile_fail
    /// use kubelet::state::{Transition, State, TransitionTo};
    /// use kubelet::pod::Pod;
    ///
    /// #[derive(Debug)]
    /// struct TestState;
    ///
    /// struct PodState;
    ///
    /// #[derive(Debug)]
    /// struct OtherState;
    ///
    /// struct OtherPodState;
    ///
    /// impl TransitionTo<OtherState> for TestState {}
    ///
    /// #[async_trait::async_trait]
    /// impl State<PodState> for TestState {
    ///     async fn next(
    ///         self: Box<Self>,
    ///         _pod_state: &mut PodState,
    ///         _pod: &Pod,
    ///     ) -> anyhow::Result<Transition<PodState>> {
    ///         // This fails because OtherState is State<OtherPodState>
    ///         Ok(Transition::next(self, OtherState))
    ///     }
    ///
    ///     async fn json_status(
    ///         &self,
    ///         _pod_state: &mut PodState,
    ///         _pod: &Pod,
    ///     ) -> anyhow::Result<serde_json::Value> {
    ///         Ok(serde_json::json!(null))
    ///     }
    /// }
    ///
    /// #[async_trait::async_trait]
    /// impl State<OtherPodState> for OtherState {
    ///     async fn next(
    ///         self: Box<Self>,
    ///         _pod_state: &mut OtherPodState,
    ///         _pod: &Pod,
    ///     ) -> anyhow::Result<Transition<OtherPodState>> {
    ///         Ok(Transition::Complete(Ok(())))
    ///     }
    ///
    ///     async fn json_status(
    ///         &self,
    ///         _pod_state: &mut OtherPodState,
    ///         _pod: &Pod,
    ///     ) -> anyhow::Result<serde_json::Value> {
    ///         Ok(serde_json::json!(null))
    ///     }
    /// }
    /// ```
    #[allow(clippy::boxed_local)]
    pub fn next<I: State<PodState>, S: State<PodState>>(_i: Box<I>, s: S) -> Transition<PodState>
    where
        I: TransitionTo<S>,
    {
        Transition::Next(StateHolder { state: Box::new(s) })
    }
}

#[async_trait::async_trait]
/// Allow for asynchronous cleanup up of PodState.
pub trait AsyncDrop: Sized {
    /// Clean up PodState.
    async fn async_drop(self);
}

#[async_trait::async_trait]
/// A trait representing a node in the state graph.
pub trait State<PodState>: Sync + Send + 'static + std::fmt::Debug {
    /// Provider supplies method to be executed when in this state.
    async fn next(self: Box<Self>, pod_state: &mut PodState, pod: &Pod) -> Transition<PodState>;

    /// Provider supplies JSON status patch to apply when entering this state.
    async fn json_status(
        &self,
        pod_state: &mut PodState,
        pod: &Pod,
    ) -> anyhow::Result<serde_json::Value>;
}

/// Iteratively evaluate state machine until it returns Complete.
pub async fn run_to_completion<PodState: Send + Sync + 'static>(
    client: &kube::Client,
    state: impl State<PodState>,
    pod_state: &mut PodState,
    pod: &Pod,
) -> anyhow::Result<()> {
    let api: Api<KubePod> = Api::namespaced(client.clone(), pod.namespace());

    let mut state: Box<dyn State<PodState>> = Box::new(state);

    loop {
        debug!("Pod {} entering state {:?}", pod.name(), state);

        let patch = state.json_status(pod_state, &pod).await?;
        debug!("Pod {} status patch: {:?}", pod.name(), &patch);
        let data = serde_json::to_vec(&patch)?;
        api.patch_status(&pod.name(), &PatchParams::default(), data)
            .await?;
        debug!("Pod {} executing state handler {:?}", pod.name(), state);

        let transition = { state.next(pod_state, &pod).await };

        state = match transition {
            Transition::Next(s) => {
                debug!("Pod {} transitioning to {:?}.", pod.name(), s.state);
                s.state
            }
            Transition::Complete(result) => {
                debug!(
                    "Pod {} execution complete with result {:?}.",
                    pod.name(),
                    result
                );
                break result;
            }
            Transition::Fatal(err) => {
                error!("Pod {} execution fatal error {:?}", pod.name(), err);
                break Err(err);
            }
        };
    }
}

#[derive(Default, Debug)]
/// Stub state machine for testing.
pub struct Stub;

#[async_trait::async_trait]
impl<P: 'static + Sync + Send> State<P> for Stub {
    async fn next(self: Box<Self>, _pod_state: &mut P, _pod: &Pod) -> Transition<P> {
        Transition::Complete(Ok(()))
    }

    async fn json_status(
        &self,
        _pod_state: &mut P,
        _pod: &Pod,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!(null))
    }
}

#[cfg(test)]
mod test {
    use crate::pod::Pod;
    use crate::state::{State, Transition, TransitionTo};

    #[derive(Debug)]
    struct PodState;

    #[derive(Debug)]
    struct ValidState;

    #[async_trait::async_trait]
    impl State<PodState> for ValidState {
        async fn next(
            self: Box<Self>,
            _pod_state: &mut PodState,
            _pod: &Pod,
        ) -> Transition<PodState> {
            Transition::Complete(Ok(()))
        }

        async fn json_status(
            &self,
            _pod_state: &mut PodState,
            _pod: &Pod,
        ) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!(null))
        }
    }

    #[test]
    fn it_can_transition_to_valid_state() {
        #[derive(Debug)]
        struct TestState;

        impl TransitionTo<ValidState> for TestState {}

        #[async_trait::async_trait]
        impl State<PodState> for TestState {
            async fn next(
                self: Box<Self>,
                _pod_state: &mut PodState,
                _pod: &Pod,
            ) -> Transition<PodState> {
                Transition::next(self, ValidState)
            }

            async fn json_status(
                &self,
                _pod_state: &mut PodState,
                _pod: &Pod,
            ) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!(null))
            }
        }
    }
}
