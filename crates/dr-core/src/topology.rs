//! 船体的连接图及其生成森林。视图不改变 XML，不猜测重复 ID 的归属。
use crate::{Connection, PartKey, Ship};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionRef {
    pub group: usize,
    pub index: usize,
    /// 编辑时校验原连接，拒绝过期的边索引。
    pub expected: Connection,
}

#[derive(Debug, Clone)]
pub struct TopologyEdge {
    pub reference: ConnectionRef,
    pub parent: usize,
    pub child: usize,
    pub dock: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct UnresolvedConnection {
    pub reference: ConnectionRef,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct Topology {
    pub nodes: Vec<PartKey>,
    pub edges: Vec<TopologyEdge>,
    pub unresolved: Vec<UnresolvedConnection>,
}

#[derive(Debug, Clone, Default)]
pub struct LinkForest {
    pub roots: Vec<usize>,
    pub children: Vec<Vec<usize>>,
    pub parent: Vec<Option<usize>>,
    pub parent_edge: Vec<Option<usize>>,
    /// 包括环边和多父连接；不能因树视图不使用而丢弃。
    pub extra_edges: Vec<usize>,
}

impl Topology {
    pub fn from_ship(ship: &Ship) -> Self {
        let nodes: Vec<_> = ship.keyed_parts().map(|(key, _)| key).collect();
        let mut endpoints: HashMap<(usize, i64), Vec<usize>> = HashMap::new();
        for (i, key) in nodes.iter().enumerate() {
            endpoints.entry((key.group, key.id)).or_default().push(i);
        }
        let mut graph = Self {
            nodes,
            ..Self::default()
        };
        for (group, _, connections) in ship.groups() {
            for (index, connection) in connections.iter().enumerate() {
                let reference = ConnectionRef {
                    group,
                    index,
                    expected: connection.clone(),
                };
                let resolve = |id| -> Result<usize, String> {
                    match endpoints.get(&(group, id)).map(Vec::as_slice) {
                        Some([node]) => Ok(*node),
                        Some(_) => Err(format!(
                            "组 {group} 的 ID {id} 有多个实例，需先修复引用归属"
                        )),
                        None => Err(format!("组 {group} 缺少 ID {id}")),
                    }
                };
                let result = (|| {
                    let (parent, child, dock) = match connection {
                        Connection::Normal { parent, child, .. } => {
                            (resolve(*parent)?, resolve(*child)?, None)
                        }
                        Connection::Dock {
                            parent,
                            child,
                            dock,
                        } => (resolve(*parent)?, resolve(*child)?, Some(resolve(*dock)?)),
                    };
                    Ok::<_, String>(TopologyEdge {
                        reference: reference.clone(),
                        parent,
                        child,
                        dock,
                    })
                })();
                match result {
                    Ok(edge) => graph.edges.push(edge),
                    Err(reason) => graph
                        .unresolved
                        .push(UnresolvedConnection { reference, reason }),
                }
            }
        }
        graph
    }

    /// 有向关系用于树和祖先检查；无向关系用于连接分量。
    pub fn adjacency(&self, directed: bool) -> Vec<Vec<(usize, usize)>> {
        let mut neighbors = vec![vec![]; self.nodes.len()];
        for (index, edge) in self.edges.iter().enumerate() {
            for target in std::iter::once(edge.child).chain(
                edge.dock
                    .filter(|node| *node != edge.parent && *node != edge.child),
            ) {
                neighbors[edge.parent].push((target, index));
                if !directed {
                    neighbors[target].push((edge.parent, index));
                }
            }
        }
        neighbors
    }

    pub fn reachable(&self, start: usize, directed: bool) -> Vec<usize> {
        if start >= self.nodes.len() {
            return vec![];
        }
        let neighbors = self.adjacency(directed);
        let mut visited = vec![false; self.nodes.len()];
        let mut queue = VecDeque::from([start]);
        let mut result = vec![];
        visited[start] = true;
        while let Some(node) = queue.pop_front() {
            result.push(node);
            for &(next, _) in &neighbors[node] {
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        result
    }

    /// 确定性的有向生成森林：先无入边节点，再覆盖环分量；所有边仍保留在原图。
    pub fn forest(&self) -> LinkForest {
        let len = self.nodes.len();
        let neighbors = self.adjacency(true);
        let mut incoming = vec![0; len];
        for list in &neighbors {
            for &(node, _) in list {
                incoming[node] += 1;
            }
        }
        let candidates = (0..len).filter(|&i| incoming[i] == 0).chain(0..len);
        let mut forest = LinkForest {
            children: vec![vec![]; len],
            parent: vec![None; len],
            parent_edge: vec![None; len],
            ..LinkForest::default()
        };
        let mut visited = vec![false; len];
        let mut used = vec![false; self.edges.len()];
        let mut extra = vec![false; self.edges.len()];
        for root in candidates {
            if visited[root] {
                continue;
            }
            visited[root] = true;
            forest.roots.push(root);
            let mut queue = VecDeque::from([root]);
            while let Some(node) = queue.pop_front() {
                for &(next, edge) in &neighbors[node] {
                    if !visited[next] {
                        visited[next] = true;
                        forest.parent[next] = Some(node);
                        forest.parent_edge[next] = Some(edge);
                        forest.children[node].push(next);
                        used[edge] = true;
                        queue.push_back(next);
                    } else {
                        extra[edge] = true;
                    }
                }
            }
        }
        forest.extra_edges = used
            .iter()
            .enumerate()
            .filter_map(|(i, used)| (!used || extra[i]).then_some(i))
            .collect();
        forest
    }
}

impl LinkForest {
    /// 迭代遍历避免深链导致栈溢出，返回（节点、相对深度）。
    pub fn subtree(&self, root: usize) -> Vec<(usize, usize)> {
        if root >= self.children.len() {
            return vec![];
        }
        let mut stack = vec![(root, 0)];
        let mut result = vec![];
        while let Some((node, depth)) = stack.pop() {
            result.push((node, depth));
            stack.extend(
                self.children[node]
                    .iter()
                    .rev()
                    .map(|child| (*child, depth + 1)),
            );
        }
        result
    }
}

#[cfg(test)]
mod tests;
