use crate::PodState;
use kubelet::state::prelude::*;

/// Pod was deleted.
#[derive(Default, Debug)]
pub struct Terminated;

#[async_trait::async_trait]
impl State<PodState> for Terminated {
    async fn next(self: Box<Self>, pod_state: &mut PodState, _pod: &Pod) -> Transition<PodState> {
        let mut lock = pod_state.shared.handles.write().await;
        if let Some(handle) = lock.get_mut(&pod_state.key) {
            let stop_result = handle.stop().await;
            if let Err(e) = stop_result {
                return Transition::Fatal(e);
            }
        }
        Transition::Complete(Ok(()))
    }

    async fn json_status(
        &self,
        _pod_state: &mut PodState,
        _pod: &Pod,
    ) -> anyhow::Result<serde_json::Value> {
        make_status(Phase::Succeeded, "Terminated")
    }
}
