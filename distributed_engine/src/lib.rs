use serde::{Deserialize, Serialize};
use snsx_runtime::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub cpu_cores: usize,
    pub memory_mb: usize,
    pub gpu: bool,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    pub entry_function: String,
    pub cpu_cost: usize,
    pub memory_cost_mb: usize,
    pub requires_gpu: bool,
    pub replicas: usize,
    pub payload: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub task_id: String,
    pub node_id: String,
    pub replica: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterPlan {
    pub assignments: Vec<Assignment>,
    pub replication_map: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterState {
    pub nodes: Vec<Node>,
}

impl ClusterState {
    pub fn schedule(&self, tasks: &[TaskSpec]) -> ClusterPlan {
        let mut scores = self
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id.clone(),
                    (node.cpu_cores as isize * 4)
                        + (node.memory_mb as isize / 256)
                        + if node.gpu { 64 } else { 0 },
                )
            })
            .collect::<HashMap<_, _>>();
        let mut assignments = Vec::new();
        let mut replication_map: HashMap<String, Vec<String>> = HashMap::new();

        for task in tasks {
            for replica in 0..task.replicas.max(1) {
                let mut candidates = self
                    .nodes
                    .iter()
                    .filter(|node| !task.requires_gpu || node.gpu)
                    .collect::<Vec<_>>();
                candidates.sort_by_key(|node| -scores.get(&node.id).copied().unwrap_or_default());
                if let Some(node) = candidates.into_iter().next() {
                    let entry = Assignment {
                        task_id: task.id.clone(),
                        node_id: node.id.clone(),
                        replica,
                    };
                    assignments.push(entry);
                    replication_map
                        .entry(task.id.clone())
                        .or_default()
                        .push(node.id.clone());
                    let weight = task.cpu_cost as isize + (task.memory_cost_mb as isize / 128);
                    *scores.entry(node.id.clone()).or_default() -= weight.max(1);
                }
            }
        }

        ClusterPlan {
            assignments,
            replication_map,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultEvent {
    pub node_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryPlan {
    pub evacuate_to: Vec<Assignment>,
}

pub fn recover(state: &ClusterState, plan: &ClusterPlan, fault: &FaultEvent) -> RecoveryPlan {
    let survivor_nodes = state
        .nodes
        .iter()
        .filter(|node| node.id != fault.node_id)
        .cloned()
        .collect::<Vec<_>>();
    let survivor_state = ClusterState {
        nodes: survivor_nodes,
    };
    let tasks = plan
        .assignments
        .iter()
        .filter(|assignment| assignment.node_id == fault.node_id)
        .map(|assignment| TaskSpec {
            id: format!("{}-recovery-{}", assignment.task_id, assignment.replica),
            entry_function: assignment.task_id.clone(),
            cpu_cost: 1,
            memory_cost_mb: 64,
            requires_gpu: false,
            replicas: 1,
            payload: Vec::new(),
        })
        .collect::<Vec<_>>();
    let cluster_plan = survivor_state.schedule(&tasks);
    RecoveryPlan {
        evacuate_to: cluster_plan.assignments,
    }
}
