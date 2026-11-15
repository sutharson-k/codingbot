pub mod coder;
pub mod corrector;
pub mod diff_pr;
pub mod indexer;
pub mod planner;
pub mod tester;

pub use coder::CodeGenerationNode;
pub use corrector::{evaluation_router, SelfCorrectionNode};
pub use diff_pr::CreateDiffOrPrNode;
pub use indexer::IndexRepoNode;
pub use planner::PlanNode;
pub use tester::RunTestsNode;

use crate::graph::engine::{StateGraph, END_NODE};
use crate::llm::LlmClient;
use std::sync::Arc;

/// Construct and wire the LangGraph-style autonomous coding agent workflow
pub fn build_autonomous_coding_graph(llm: Arc<dyn LlmClient>) -> StateGraph {
    let mut graph = StateGraph::new();

    // 1. Register processing nodes
    graph.add_node("index_repo", Box::new(IndexRepoNode));
    graph.add_node("plan_changes", Box::new(PlanNode::new(llm.clone())));
    graph.add_node("generate_code", Box::new(CodeGenerationNode::new(llm.clone())));
    graph.add_node("run_tests", Box::new(RunTestsNode));
    graph.add_node("self_correct", Box::new(SelfCorrectionNode::new(llm)));
    graph.add_node("create_diff_pr", Box::new(CreateDiffOrPrNode));

    // 2. Define entry point
    graph.set_entry_point("index_repo");

    // 3. Connect linear edges
    graph.add_edge("index_repo", "plan_changes");
    graph.add_edge("plan_changes", "generate_code");
    graph.add_edge("generate_code", "run_tests");

    // 4. Connect conditional routing edge from test execution
    // Evaluates test outcome: if green -> create_diff_pr, if failed & retries remain -> self_correct
    graph.add_conditional_edge("run_tests", evaluation_router);

    // 5. Connect self-correction back to re-running the test suite (The Self-Correction Cycle)
    graph.add_edge("self_correct", "run_tests");

    // 6. Connect final node to graph termination
    graph.add_edge("create_diff_pr", END_NODE);

    graph
}
