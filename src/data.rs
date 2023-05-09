use crate::error;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

const SEPARATOR: &'static str = "__";

fn default_if_empty<'de, D, T>(de: D) -> error::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Option::<T>::deserialize(de).map(|x| x.unwrap_or_else(|| T::default()))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    #[serde(rename(serialize = "id"))]
    pid: Option<usize>,
    #[serde(rename(serialize = "classification", deserialize = "id"))]
    node: String,
    #[serde(rename(serialize = "classification_origin"))]
    origin: Option<String>,
    #[serde(rename(serialize = "classification_label"))]
    label: String,
    #[serde(rename(serialize = "classification_parent", deserialize = "parent"))]
    parent_node: Option<String>,
    #[serde(rename(serialize = "parent_id"))]
    parent_id: Option<usize>,
    #[serde(default, deserialize_with = "default_if_empty")]
    leaf: bool,
    lft: Option<usize>,
    rgt: Option<usize>,
    count: Option<usize>,
}

#[derive(Debug)]
pub struct Graph {
    pub nodes: Vec<Node>,
    root: usize, // index of root node in the vector
}

impl Graph {
    pub fn new(nodes: Vec<Node>) -> error::Result<Self> {
        let mut root: Option<usize> = None;

        for (i, node) in nodes.iter().enumerate() {
            if node.parent_node.is_none() {
                if root.is_none() {
                    root = Some(i)
                } else {
                    Err(error::Error::MultipleRootNodeError())?
                }
            }
        }

        Ok(Graph {
            nodes,
            root: root.ok_or(error::Error::RootNodeNotFoundError())?,
        })
    }

    pub fn is_dag(&self) -> bool {
        let mut set = HashSet::new();

        for node in self.nodes.iter() {
            if !node.leaf && !set.insert(node.node.to_owned()) {
                return true;
            }
        }

        false
    }

    fn build_child_map(&self) -> HashMap<String, Vec<(usize, String)>> {
        let mut child_map = HashMap::new();

        for (i, node) in self.nodes.iter().enumerate() {
            if let Some(parent) = &node.parent_node {
                child_map
                    .entry(parent.to_owned())
                    .or_insert_with(Vec::new)
                    .push((i, node.node.to_owned()))
            }
        }

        child_map
    }

    pub fn dag_to_tree(&self) -> error::Result<Self> {
        let child_map = self.build_child_map();
        let mut queue = VecDeque::new();
        let mut visited = HashMap::new();

        let mut nodes = Vec::new();
        nodes.push(self.nodes[self.root].to_owned());
        queue.push_back((self.root, nodes.len() - 1));

        while let Some((orig, new)) = queue.pop_front() {
            if let Some(children) = child_map.get(&self.nodes[orig].node) {
                for (i, child) in children {
                    let branch = visited
                        .entry(child)
                        .and_modify(|c| *c += 1)
                        .or_insert(0 as usize);

                    let mut node = self.nodes[*i].to_owned();
                    node.parent_node = Some(nodes[new].node.to_owned());
                    if !node.leaf && *branch != (0 as usize) {
                        node.origin = Some(node.node.to_owned());
                        node.node = format!("{}{}{}", node.node, SEPARATOR, *branch);
                    }

                    nodes.push(node);
                    queue.push_back((*i, nodes.len() - 1));
                }
            }
        }

        Ok(Graph { nodes, root: 0 })
    }

    pub fn complement_leaf(&self) -> error::Result<Self> {
        let mut nodes = VecDeque::new();
        let mut set = HashSet::new();

        let mut push_unless_exist = |node: Node| {
            if set.insert((node.node.to_owned(), node.parent_node.to_owned())) {
                nodes.push_back(node);
            }
        };

        for node in self.nodes.iter() {
            let mut classification = node.to_owned();
            let mut leaf = classification.to_owned();

            classification.node = format!("c{}{}", SEPARATOR, classification.node);
            if let Some(node) = classification.parent_node {
                classification.parent_node = Some(format!("c{}{}", SEPARATOR, node));
            }
            classification.leaf = false;

            leaf.parent_node = Some(classification.node.to_owned());
            leaf.leaf = true;

            push_unless_exist(classification);
            push_unless_exist(leaf);
        }

        Ok(Graph {
            nodes: nodes.into(),
            root: 0,
        })
    }

    pub fn build_index(&mut self) -> error::Result<&Self> {
        fn fill(
            nodes: &mut Vec<Node>,
            child_map: &HashMap<String, Vec<(usize, String)>>,
            parent_map: &HashMap<String, usize>,
            i: usize,
            n: usize,
        ) -> error::Result<usize> {
            {
                let node = nodes.get_mut(i).unwrap();
                node.lft = Some(n);

                if let Some(p) = &node.parent_node {
                    let pi = parent_map
                        .get(p)
                        .ok_or(error::Error::ParentNodeNotFoundError(p.to_owned()))?;
                    node.parent_id = Some(*pi)
                }
            }

            match child_map.get(&nodes.get(i).unwrap().node) {
                Some(children) => {
                    let mut n2 = n;

                    for (i2, _child) in children {
                        n2 = fill(nodes, child_map, parent_map, *i2, n2 + 1)?;
                    }

                    {
                        let node = nodes.get_mut(i).unwrap();
                        node.rgt = Some(n2 + 1);
                        node.count = Some(children.len());
                    }

                    Ok(n2 + 1)
                }
                None => {
                    {
                        let node = nodes.get_mut(i).unwrap();
                        node.rgt = Some(n + 1);
                        node.count = Some(0);
                    }

                    Ok(n + 1)
                }
            }
        }

        let mut parent_map = HashMap::<String, usize>::new();
        for (i, x) in self.nodes.iter_mut().enumerate() {
            x.pid = Some(i + 1);
            if !x.leaf {
                parent_map.insert(x.node.to_owned(), i + 1);
            }
        }

        let child_map = self.build_child_map();

        fill(self.nodes.as_mut(), &child_map, &parent_map, self.root, 1)?;

        self.nodes.sort_by(|a, b| a.pid.cmp(&b.pid));

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{Graph, Node};

    fn test_data() -> Vec<Node> {
        vec![
            Node {
                pid: None,
                node: "1".to_owned(),
                origin: None,
                label: "1".to_owned(),
                parent_node: None,
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "2".to_owned(),
                origin: None,
                label: "2".to_owned(),
                parent_node: Some("1".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "3".to_owned(),
                origin: None,
                label: "3".to_owned(),
                parent_node: Some("2".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "4".to_owned(),
                origin: None,
                label: "4".to_owned(),
                parent_node: Some("3".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "4".to_owned(),
                origin: None,
                label: "4".to_owned(),
                parent_node: Some("1".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "5".to_owned(),
                origin: None,
                label: "5".to_owned(),
                parent_node: Some("3".to_owned()),
                parent_id: None,
                leaf: true,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "5".to_owned(),
                origin: None,
                label: "5".to_owned(),
                parent_node: Some("4".to_owned()),
                parent_id: None,
                leaf: true,
                lft: None,
                rgt: None,
                count: None,
            },
        ]
    }

    #[test]
    fn test_dag() {
        let graph = Graph::new(test_data()).unwrap();
        assert_eq!(graph.is_dag(), true);
        assert_eq!(graph.nodes.len(), 7);

        let mut graph = graph.dag_to_tree().unwrap();
        assert_eq!(graph.is_dag(), false);
        assert_eq!(graph.nodes.len(), 8);

        let graph = graph.build_index().unwrap();
        let nodes = &graph.nodes;
        {
            let node = nodes.get(0).unwrap();
            assert_eq!(node.pid, Some(1));
            assert_eq!(node.node, "1".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "1".to_owned());
            assert_eq!(node.parent_node, None);
            assert_eq!(node.parent_id, None);
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(1));
            assert_eq!(node.rgt, Some(16));
            assert_eq!(node.count, Some(2));
        }

        {
            let node = nodes.get(1).unwrap();
            assert_eq!(node.pid, Some(2));
            assert_eq!(node.node, "2".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "2".to_owned());
            assert_eq!(node.parent_node, Some("1".to_owned()));
            assert_eq!(node.parent_id, Some(1));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(2));
            assert_eq!(node.rgt, Some(11));
            assert_eq!(node.count, Some(1));
        }

        {
            let node = nodes.get(2).unwrap();
            assert_eq!(node.pid, Some(3));
            assert_eq!(node.node, "4".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "4".to_owned());
            assert_eq!(node.parent_node, Some("1".to_owned()));
            assert_eq!(node.parent_id, Some(1));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(12));
            assert_eq!(node.rgt, Some(15));
            assert_eq!(node.count, Some(1));
        }

        {
            let node = nodes.get(3).unwrap();
            assert_eq!(node.pid, Some(4));
            assert_eq!(node.node, "3".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "3".to_owned());
            assert_eq!(node.parent_node, Some("2".to_owned()));
            assert_eq!(node.parent_id, Some(2));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(3));
            assert_eq!(node.rgt, Some(10));
            assert_eq!(node.count, Some(2));
        }

        {
            let node = nodes.get(4).unwrap();
            assert_eq!(node.pid, Some(5));
            assert_eq!(node.node, "5".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "5".to_owned());
            assert_eq!(node.parent_node, Some("4".to_owned()));
            assert_eq!(node.parent_id, Some(3));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(13));
            assert_eq!(node.rgt, Some(14));
            assert_eq!(node.count, Some(0));
        }

        {
            let node = nodes.get(5).unwrap();
            assert_eq!(node.pid, Some(6));
            assert_eq!(node.node, "4__1".to_owned());
            assert_eq!(node.origin, Some("4".to_owned()));
            assert_eq!(node.label, "4".to_owned());
            assert_eq!(node.parent_node, Some("3".to_owned()));
            assert_eq!(node.parent_id, Some(4));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(4));
            assert_eq!(node.rgt, Some(7));
            assert_eq!(node.count, Some(1));
        }

        {
            let node = nodes.get(6).unwrap();
            assert_eq!(node.pid, Some(7));
            assert_eq!(node.node, "5".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "5".to_owned());
            assert_eq!(node.parent_node, Some("3".to_owned()));
            assert_eq!(node.parent_id, Some(4));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(8));
            assert_eq!(node.rgt, Some(9));
            assert_eq!(node.count, Some(0));
        }

        {
            let node = nodes.get(7).unwrap();
            assert_eq!(node.pid, Some(8));
            assert_eq!(node.node, "5".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "5".to_owned());
            assert_eq!(node.parent_node, Some("4__1".to_owned()));
            assert_eq!(node.parent_id, Some(6));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(5));
            assert_eq!(node.rgt, Some(6));
            assert_eq!(node.count, Some(0));
        }
    }

    #[test]
    fn test_dag_complement_leaf() {
        let graph = Graph::new(test_data()).unwrap();
        assert_eq!(graph.nodes.len(), 7);

        let graph = graph.complement_leaf().unwrap();
        assert_eq!(graph.is_dag(), true);
        assert_eq!(graph.nodes.len(), 12);

        let mut graph = graph.dag_to_tree().unwrap();
        assert_eq!(graph.is_dag(), false);
        assert_eq!(graph.nodes.len(), 16);

        let graph = graph.build_index().unwrap();
        let nodes = &graph.nodes;
        {
            let node = nodes.get(0).unwrap();
            assert_eq!(node.pid, Some(1));
            assert_eq!(node.node, "c__1".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "1".to_owned());
            assert_eq!(node.parent_node, None);
            assert_eq!(node.parent_id, None);
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(1));
            assert_eq!(node.rgt, Some(32));
            assert_eq!(node.count, Some(3));
        }

        {
            let node = nodes.get(1).unwrap();
            assert_eq!(node.pid, Some(2));
            assert_eq!(node.node, "1".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "1".to_owned());
            assert_eq!(node.parent_node, Some("c__1".to_owned()));
            assert_eq!(node.parent_id, Some(1));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(2));
            assert_eq!(node.rgt, Some(3));
            assert_eq!(node.count, Some(0));
        }

        {
            let node = nodes.get(2).unwrap();
            assert_eq!(node.pid, Some(3));
            assert_eq!(node.node, "c__2".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "2".to_owned());
            assert_eq!(node.parent_node, Some("c__1".to_owned()));
            assert_eq!(node.parent_id, Some(1));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(4));
            assert_eq!(node.rgt, Some(23));
            assert_eq!(node.count, Some(2));
        }

        {
            let node = nodes.get(3).unwrap();
            assert_eq!(node.pid, Some(4));
            assert_eq!(node.node, "c__4".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "4".to_owned());
            assert_eq!(node.parent_node, Some("c__1".to_owned()));
            assert_eq!(node.parent_id, Some(1));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(24));
            assert_eq!(node.rgt, Some(31));
            assert_eq!(node.count, Some(2));
        }

        {
            let node = nodes.get(4).unwrap();
            assert_eq!(node.pid, Some(5));
            assert_eq!(node.node, "2".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "2".to_owned());
            assert_eq!(node.parent_node, Some("c__2".to_owned()));
            assert_eq!(node.parent_id, Some(3));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(5));
            assert_eq!(node.rgt, Some(6));
            assert_eq!(node.count, Some(0));
        }

        {
            let node = nodes.get(5).unwrap();
            assert_eq!(node.pid, Some(6));
            assert_eq!(node.node, "c__3".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "3".to_owned());
            assert_eq!(node.parent_node, Some("c__2".to_owned()));
            assert_eq!(node.parent_id, Some(3));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(7));
            assert_eq!(node.rgt, Some(22));
            assert_eq!(node.count, Some(3));
        }

        {
            let node = nodes.get(6).unwrap();
            assert_eq!(node.pid, Some(7));
            assert_eq!(node.node, "4".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "4".to_owned());
            assert_eq!(node.parent_node, Some("c__4".to_owned()));
            assert_eq!(node.parent_id, Some(4));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(25));
            assert_eq!(node.rgt, Some(26));
            assert_eq!(node.count, Some(0));
        }

        {
            let node = nodes.get(7).unwrap();
            assert_eq!(node.pid, Some(8));
            assert_eq!(node.node, "c__5".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "5".to_owned());
            assert_eq!(node.parent_node, Some("c__4".to_owned()));
            assert_eq!(node.parent_id, Some(4));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(27));
            assert_eq!(node.rgt, Some(30));
            assert_eq!(node.count, Some(1));
        }

        {
            let node = nodes.get(8).unwrap();
            assert_eq!(node.pid, Some(9));
            assert_eq!(node.node, "3".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "3".to_owned());
            assert_eq!(node.parent_node, Some("c__3".to_owned()));
            assert_eq!(node.parent_id, Some(6));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(8));
            assert_eq!(node.rgt, Some(9));
            assert_eq!(node.count, Some(0));
        }

        {
            let node = nodes.get(9).unwrap();
            assert_eq!(node.pid, Some(10));
            assert_eq!(node.node, "c__4__1".to_owned());
            assert_eq!(node.origin, Some("c__4".to_owned()));
            assert_eq!(node.label, "4".to_owned());
            assert_eq!(node.parent_node, Some("c__3".to_owned()));
            assert_eq!(node.parent_id, Some(6));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(10));
            assert_eq!(node.rgt, Some(17));
            assert_eq!(node.count, Some(2));
        }

        {
            let node = nodes.get(10).unwrap();
            assert_eq!(node.pid, Some(11));
            assert_eq!(node.node, "c__5__1".to_owned());
            assert_eq!(node.origin, Some("c__5".to_owned()));
            assert_eq!(node.label, "5".to_owned());
            assert_eq!(node.parent_node, Some("c__3".to_owned()));
            assert_eq!(node.parent_id, Some(6));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(18));
            assert_eq!(node.rgt, Some(21));
            assert_eq!(node.count, Some(1));
        }

        {
            let node = nodes.get(11).unwrap();
            assert_eq!(node.pid, Some(12));
            assert_eq!(node.node, "5".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "5".to_owned());
            assert_eq!(node.parent_node, Some("c__5".to_owned()));
            assert_eq!(node.parent_id, Some(8));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(28));
            assert_eq!(node.rgt, Some(29));
            assert_eq!(node.count, Some(0));
        }

        {
            let node = nodes.get(12).unwrap();
            assert_eq!(node.pid, Some(13));
            assert_eq!(node.node, "4".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "4".to_owned());
            assert_eq!(node.parent_node, Some("c__4__1".to_owned()));
            assert_eq!(node.parent_id, Some(10));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(11));
            assert_eq!(node.rgt, Some(12));
            assert_eq!(node.count, Some(0));
        }

        {
            let node = nodes.get(13).unwrap();
            assert_eq!(node.pid, Some(14));
            assert_eq!(node.node, "c__5__2".to_owned());
            assert_eq!(node.origin, Some("c__5".to_owned()));
            assert_eq!(node.label, "5".to_owned());
            assert_eq!(node.parent_node, Some("c__4__1".to_owned()));
            assert_eq!(node.parent_id, Some(10));
            assert_eq!(node.leaf, false);
            assert_eq!(node.lft, Some(13));
            assert_eq!(node.rgt, Some(16));
            assert_eq!(node.count, Some(1));
        }

        {
            let node = nodes.get(14).unwrap();
            assert_eq!(node.pid, Some(15));
            assert_eq!(node.node, "5".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "5".to_owned());
            assert_eq!(node.parent_node, Some("c__5__1".to_owned()));
            assert_eq!(node.parent_id, Some(11));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(19));
            assert_eq!(node.rgt, Some(20));
            assert_eq!(node.count, Some(0));
        }

        {
            let node = nodes.get(15).unwrap();
            assert_eq!(node.pid, Some(16));
            assert_eq!(node.node, "5".to_owned());
            assert_eq!(node.origin, None);
            assert_eq!(node.label, "5".to_owned());
            assert_eq!(node.parent_node, Some("c__5__2".to_owned()));
            assert_eq!(node.parent_id, Some(14));
            assert_eq!(node.leaf, true);
            assert_eq!(node.lft, Some(14));
            assert_eq!(node.rgt, Some(15));
            assert_eq!(node.count, Some(0));
        }
    }
}
