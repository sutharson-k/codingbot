use super::state::AgentState;
use anyhow::Result;
use async_trait::async_trait;

/// The Node trait represents an autonomous processing unit in the LangGraph-style state machine.
/// Each node consumes and mutates the shared `AgentState`, performing actions like indexing,
/// planning, coding, testing, or self-correcting.
#[async_trait]
pub trait Node: Send + Sync {
    /// Return the unique name of this node
    fn name(&self) -> &'static str;

    /// Execute the node's logic against the shared state
    async fn execute(&self, state: &mut AgentState) -> Result<()>;
}
