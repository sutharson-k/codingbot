use super::node::Node;
use super::state::AgentState;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::sync::Arc;

pub const START_NODE: &str = "__start__";
pub const END_NODE: &str = "__end__";

pub type RouterFn = Arc<dyn Fn(&AgentState) -> String + Send + Sync>;
pub type StepCallback = Arc<dyn Fn(&str, &AgentState) + Send + Sync>;

/// LangGraph-style State Machine Engine
pub struct StateGraph {
    nodes: HashMap<String, Box<dyn Node>>,
    edges: HashMap<String, String>,
    conditional_edges: HashMap<String, RouterFn>,
    entry_point: Option<String>,
    step_listeners: Vec<StepCallback>,
}

impl Default for StateGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl StateGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            conditional_edges: HashMap::new(),
            entry_point: None,
            step_listeners: Vec::new(),
        }
    }

    pub fn add_node(&mut self, name: &str, node: Box<dyn Node>) -> &mut Self {
        self.nodes.insert(name.to_string(), node);
        self
    }

    pub fn set_entry_point(&mut self, name: &str) -> &mut Self {
        self.entry_point = Some(name.to_string());
        self
    }

    pub fn add_edge(&mut self, from: &str, to: &str) -> &mut Self {
        self.edges.insert(from.to_string(), to.to_string());
        self
    }

    pub fn add_conditional_edge<F>(&mut self, from: &str, router: F) -> &mut Self
    where
        F: Fn(&AgentState) -> String + Send + Sync + 'static,
    {
        self.conditional_edges
            .insert(from.to_string(), Arc::new(router));
        self
    }

    pub fn on_step<F>(&mut self, callback: F) -> &mut Self
    where
        F: Fn(&str, &AgentState) + Send + Sync + 'static,
    {
        self.step_listeners.push(Arc::new(callback));
        self
    }

    pub fn validate(&self) -> Result<()> {
        let entry = self
            .entry_point
            .as_ref()
            .context("Graph has no entry point defined")?;

        if !self.nodes.contains_key(entry) {
            bail!("Entry point '{}' does not exist among nodes", entry);
        }

        for (from, to) in &self.edges {
            if !self.nodes.contains_key(from) && from != START_NODE {
                bail!("Edge origin '{}' is not registered as a node", from);
            }
            if !self.nodes.contains_key(to) && to != END_NODE {
                bail!("Edge target '{}' is not registered as a node", to);
            }
        }

        for from in self.conditional_edges.keys() {
            if !self.nodes.contains_key(from) {
                bail!("Conditional edge origin '{}' is not registered", from);
            }
        }

        Ok(())
    }

    pub fn to_mermaid(&self) -> String {
        let mut lines = Vec::new();
        lines.push("flowchart TD".to_string());
        lines.push(format!("    Start([\"START\"]) --> {}", self.entry_point.as_deref().unwrap_or("Unknown")));

        for (from, to) in &self.edges {
            if to == END_NODE {
                lines.push(format!("    {} --> End([\"END\"])", from));
            } else {
                lines.push(format!("    {} --> {}", from, to));
            }
        }

        for (from, _) in &self.conditional_edges {
            lines.push(format!("    {} -. \"[conditional router]\" .-> Router_{}", from, from));
            lines.push(format!("    Router_{}{{{{\"Evaluate State\"}}}}", from));
        }

        lines.join("\n")
    }

    pub async fn run(&self, mut state: AgentState) -> Result<AgentState> {
        self.validate()?;

        let mut current_node_name = self.entry_point.clone().unwrap();
        let mut total_steps = 0;
        const SAFETY_MAX_STEPS: usize = 50;

        qprintln!(state,
            "\n{}",
            "================================================================"
                .cyan()
                .bold()
        );
        qprintln!(state,
            "{} {}",
            "  LANGGRAPH AGENT WORKFLOW STARTED:".green().bold(),
            state.task.bold()
        );
        qprintln!(state,
            "{}",
            "================================================================"
                .cyan()
                .bold()
        );

        while current_node_name != END_NODE {
            total_steps += 1;
            if total_steps > SAFETY_MAX_STEPS {
                bail!(
                    "State machine exceeded safety limit of {} steps. Terminating cycle.",
                    SAFETY_MAX_STEPS
                );
            }

            let node = self
                .nodes
                .get(&current_node_name)
                .with_context(|| format!("Node '{}' not found in graph", current_node_name))?;

            qprintln!(state,
                "\n{} [{}] Node: {}",
                "->".blue().bold(),
                total_steps.to_string().yellow(),
                current_node_name.to_uppercase().cyan().bold()
            );

            node.execute(&mut state).await.with_context(|| {
                format!("Execution failed at node '{}'", current_node_name)
            })?;

            for listener in &self.step_listeners {
                listener(&current_node_name, &state);
            }

            let next_node = if let Some(router) = self.conditional_edges.get(&current_node_name) {
                let destination = router(&state);
                qprintln!(state,
                    "   {} Conditional route decided: {}",
                    "?".magenta().bold(),
                    destination.yellow().bold()
                );
                destination
            } else if let Some(target) = self.edges.get(&current_node_name) {
                target.clone()
            } else {
                END_NODE.to_string()
            };

            current_node_name = next_node;
        }

        qprintln!(state,
            "\n{}",
            "================================================================"
                .cyan()
                .bold()
        );
        qprintln!(state,
            "{}",
            "  LANGGRAPH AGENT WORKFLOW REACHED END NODE".green().bold()
        );
        qprintln!(state,
            "{}",
            "================================================================"
                .cyan()
                .bold()
        );

        Ok(state)
    }
}
