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

        for (i, node) in self.nodes.iter().enumerate() {
            let mut classification = node.to_owned();
            let mut leaf = classification.to_owned();

            classification.node = format!("c{}{}", SEPARATOR, classification.node);
            if let Some(node) = classification.parent_node {
                classification.parent_node = Some(format!("c{}{}", SEPARATOR, node));
            }
            classification.leaf = false;

            leaf.parent_node = Some(classification.node.to_owned());
            leaf.leaf = true;

            if i == self.root {
                nodes.push_front(leaf);
                nodes.push_front(classification);
            } else {
                nodes.push_back(classification);
                nodes.push_back(leaf);
            }
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
                    let mut r = n;

                    for (i, _child) in children {
                        r = fill(nodes, child_map, parent_map, *i, r + 1)?;
                    }

                    {
                        let node = nodes.get_mut(i).unwrap();
                        node.rgt = Some(r + 1);
                        node.count = Some(children.len());
                    }

                    Ok(r + 1)
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

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{Graph, Node};

    /// Example from https://en.wikipedia.org/wiki/Nested_set_model
    ///
    /// | id | parent_id | Node          | Lft | Rgt |
    /// |----|-----------|---------------|-----|-----|
    /// |  1 |           | Clothing      |   1 |  22 |
    /// |  2 |         1 | Men's         |   2 |   9 |
    /// |  3 |         1 | Women's       |  10 |  21 |
    /// |  4 |         2 | Suits         |   3 |   8 |
    /// |  5 |         4 | Slacks        |   4 |   5 |
    /// |  6 |         4 | Jackets       |   6 |   7 |
    /// |  7 |         3 | Dresses       |  11 |  16 |
    /// |  8 |         3 | Skirts        |  17 |  18 |
    /// |  9 |         3 | Blouses       |  19 |  20 |
    /// | 10 |         7 | Evening Gowns |  12 |  13 |
    /// | 11 |         7 | Sun Dresses   |  14 |  15 |
    #[test]
    fn test_build_index() {
        let data = vec![
            Node {
                pid: None,
                node: "Clothing".to_owned(),
                label: "Clothing".to_owned(),
                parent_node: None,
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Men's".to_owned(),
                label: "Men's".to_owned(),
                parent_node: Some("Clothing".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Women's".to_owned(),
                label: "Women's".to_owned(),
                parent_node: Some("Clothing".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Suits".to_owned(),
                label: "Suits".to_owned(),
                parent_node: Some("Men's".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Slacks".to_owned(),
                label: "Slacks".to_owned(),
                parent_node: Some("Suits".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Jackets".to_owned(),
                label: "Jackets".to_owned(),
                parent_node: Some("Suits".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Dresses".to_owned(),
                label: "Dresses".to_owned(),
                parent_node: Some("Women's".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Skirts".to_owned(),
                label: "Skirts".to_owned(),
                parent_node: Some("Women's".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Blouses".to_owned(),
                label: "Blouses".to_owned(),
                parent_node: Some("Women's".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Evening Gowns".to_owned(),
                label: "Evening Gowns".to_owned(),
                parent_node: Some("Dresses".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "Sun Dresses".to_owned(),
                label: "Sun Dresses".to_owned(),
                parent_node: Some("Dresses".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
        ];

        let mut graph = Graph::new(data).unwrap();

        assert_eq!(graph.is_dag(), false);

        let graph = graph.build_index().unwrap();
        let nodes = &graph.nodes;

        assert_eq!(nodes.len(), 11);

        assert_eq!(nodes.get(0).unwrap().pid, Some(1));
        assert_eq!(nodes.get(0).unwrap().parent_id, None);
        assert_eq!(nodes.get(0).unwrap().lft, Some(1));
        assert_eq!(nodes.get(0).unwrap().rgt, Some(22));

        assert_eq!(nodes.get(1).unwrap().pid, Some(2));
        assert_eq!(nodes.get(1).unwrap().parent_id, Some(1));
        assert_eq!(nodes.get(1).unwrap().lft, Some(2));
        assert_eq!(nodes.get(1).unwrap().rgt, Some(9));

        assert_eq!(nodes.get(2).unwrap().pid, Some(3));
        assert_eq!(nodes.get(2).unwrap().parent_id, Some(1));
        assert_eq!(nodes.get(2).unwrap().lft, Some(10));
        assert_eq!(nodes.get(2).unwrap().rgt, Some(21));

        assert_eq!(nodes.get(3).unwrap().pid, Some(4));
        assert_eq!(nodes.get(3).unwrap().parent_id, Some(2));
        assert_eq!(nodes.get(3).unwrap().lft, Some(3));
        assert_eq!(nodes.get(3).unwrap().rgt, Some(8));

        assert_eq!(nodes.get(4).unwrap().pid, Some(5));
        assert_eq!(nodes.get(4).unwrap().parent_id, Some(4));
        assert_eq!(nodes.get(4).unwrap().lft, Some(4));
        assert_eq!(nodes.get(4).unwrap().rgt, Some(5));

        assert_eq!(nodes.get(5).unwrap().pid, Some(6));
        assert_eq!(nodes.get(5).unwrap().parent_id, Some(4));
        assert_eq!(nodes.get(5).unwrap().lft, Some(6));
        assert_eq!(nodes.get(5).unwrap().rgt, Some(7));

        assert_eq!(nodes.get(6).unwrap().pid, Some(7));
        assert_eq!(nodes.get(6).unwrap().parent_id, Some(3));
        assert_eq!(nodes.get(6).unwrap().lft, Some(11));
        assert_eq!(nodes.get(6).unwrap().rgt, Some(16));

        assert_eq!(nodes.get(7).unwrap().pid, Some(8));
        assert_eq!(nodes.get(7).unwrap().parent_id, Some(3));
        assert_eq!(nodes.get(7).unwrap().lft, Some(17));
        assert_eq!(nodes.get(7).unwrap().rgt, Some(18));

        assert_eq!(nodes.get(8).unwrap().pid, Some(9));
        assert_eq!(nodes.get(8).unwrap().parent_id, Some(3));
        assert_eq!(nodes.get(8).unwrap().lft, Some(19));
        assert_eq!(nodes.get(8).unwrap().rgt, Some(20));

        assert_eq!(nodes.get(9).unwrap().pid, Some(10));
        assert_eq!(nodes.get(9).unwrap().parent_id, Some(7));
        assert_eq!(nodes.get(9).unwrap().lft, Some(12));
        assert_eq!(nodes.get(9).unwrap().rgt, Some(13));

        assert_eq!(nodes.get(10).unwrap().pid, Some(11));
        assert_eq!(nodes.get(10).unwrap().parent_id, Some(7));
        assert_eq!(nodes.get(10).unwrap().lft, Some(14));
        assert_eq!(nodes.get(10).unwrap().rgt, Some(15));
    }

    #[test]
    fn test_dag_to_tree() {
        let data = vec![
            Node {
                pid: None,
                node: "0".to_owned(),
                label: "0".to_owned(),
                parent_node: None,
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "1".to_owned(),
                label: "1".to_owned(),
                parent_node: Some("0".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "2".to_owned(),
                label: "2".to_owned(),
                parent_node: Some("0".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "3".to_owned(),
                label: "3".to_owned(),
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
                label: "4".to_owned(),
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
                label: "5".to_owned(),
                parent_node: Some("2".to_owned()),
                parent_id: None,
                leaf: true,
                lft: None,
                rgt: None,
                count: None,
            },
        ];

        let graph = Graph::new(data).unwrap();

        assert_eq!(graph.is_dag(), true);

        let tree = graph.dag_to_tree().unwrap();
        let nodes = &tree.nodes;

        assert_eq!(tree.is_dag(), false);
        assert_eq!(nodes.len(), 10);

        assert_eq!(nodes.get(0).unwrap().node, "0".to_owned());
        assert_eq!(nodes.get(0).unwrap().parent_node, None);

        assert_eq!(nodes.get(1).unwrap().node, "1".to_owned());
        assert_eq!(nodes.get(1).unwrap().parent_node, Some("0".to_owned()));

        assert_eq!(nodes.get(2).unwrap().node, "2".to_owned());
        assert_eq!(nodes.get(2).unwrap().parent_node, Some("0".to_owned()));

        assert_eq!(nodes.get(3).unwrap().node, "3".to_owned());
        assert_eq!(nodes.get(3).unwrap().parent_node, Some("1".to_owned()));

        assert_eq!(nodes.get(4).unwrap().node, "3__1".to_owned());
        assert_eq!(nodes.get(4).unwrap().parent_node, Some("2".to_owned()));

        assert_eq!(nodes.get(5).unwrap().node, "5".to_owned());
        assert_eq!(nodes.get(5).unwrap().parent_node, Some("2".to_owned()));

        assert_eq!(nodes.get(6).unwrap().node, "4".to_owned());
        assert_eq!(nodes.get(6).unwrap().parent_node, Some("3".to_owned()));

        assert_eq!(nodes.get(7).unwrap().node, "5".to_owned());
        assert_eq!(nodes.get(7).unwrap().parent_node, Some("3".to_owned()));

        assert_eq!(nodes.get(8).unwrap().node, "4".to_owned());
        assert_eq!(nodes.get(8).unwrap().parent_node, Some("3__1".to_owned()));

        assert_eq!(nodes.get(9).unwrap().node, "5".to_owned());
        assert_eq!(nodes.get(9).unwrap().parent_node, Some("3__1".to_owned()));
    }

    #[test]
    fn test_complement_leaf() {
        let data = vec![
            Node {
                pid: None,
                node: "1".to_owned(),
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
                label: "3".to_owned(),
                parent_node: Some("1".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                pid: None,
                node: "4".to_owned(),
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
                node: "5".to_owned(),
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
                node: "6".to_owned(),
                label: "6".to_owned(),
                parent_node: Some("3".to_owned()),
                parent_id: None,
                leaf: true,
                lft: None,
                rgt: None,
                count: None,
            },
        ];

        let graph = Graph::new(data).unwrap();

        assert_eq!(graph.is_dag(), false);

        let graph = graph.complement_leaf().unwrap();
        let nodes = &graph.nodes;

        assert_eq!(nodes.len(), 12);

        assert_eq!(nodes.get(0).unwrap().node, "c__1".to_owned());
        assert_eq!(nodes.get(0).unwrap().parent_node, None);

        assert_eq!(nodes.get(1).unwrap().node, "1".to_owned());
        assert_eq!(nodes.get(1).unwrap().parent_node, Some("c__1".to_owned()));

        assert_eq!(nodes.get(2).unwrap().node, "c__2".to_owned());
        assert_eq!(nodes.get(2).unwrap().parent_node, Some("c__1".to_owned()));

        assert_eq!(nodes.get(3).unwrap().node, "2".to_owned());
        assert_eq!(nodes.get(3).unwrap().parent_node, Some("c__2".to_owned()));

        assert_eq!(nodes.get(4).unwrap().node, "c__3".to_owned());
        assert_eq!(nodes.get(4).unwrap().parent_node, Some("c__1".to_owned()));

        assert_eq!(nodes.get(5).unwrap().node, "3".to_owned());
        assert_eq!(nodes.get(5).unwrap().parent_node, Some("c__3".to_owned()));

        assert_eq!(nodes.get(6).unwrap().node, "c__4".to_owned());
        assert_eq!(nodes.get(6).unwrap().parent_node, Some("c__3".to_owned()));

        assert_eq!(nodes.get(7).unwrap().node, "4".to_owned());
        assert_eq!(nodes.get(7).unwrap().parent_node, Some("c__4".to_owned()));

        assert_eq!(nodes.get(8).unwrap().node, "c__5".to_owned());
        assert_eq!(nodes.get(8).unwrap().parent_node, Some("c__3".to_owned()));

        assert_eq!(nodes.get(9).unwrap().node, "5".to_owned());
        assert_eq!(nodes.get(9).unwrap().parent_node, Some("c__5".to_owned()));

        assert_eq!(nodes.get(10).unwrap().node, "c__6".to_owned());
        assert_eq!(nodes.get(10).unwrap().parent_node, Some("c__3".to_owned()));

        assert_eq!(nodes.get(11).unwrap().node, "6".to_owned());
        assert_eq!(nodes.get(11).unwrap().parent_node, Some("c__6".to_owned()));
    }
}
