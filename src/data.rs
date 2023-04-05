use crate::error;
use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_if_empty<'de, D, T>(de: D) -> error::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Option::<T>::deserialize(de).map(|x| x.unwrap_or_else(|| T::default()))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    pid: Option<usize>,
    #[serde(rename(serialize = "classification", deserialize = "id"))]
    node: String,
    #[serde(rename(serialize = "classification_label", deserialize = "label"))]
    node_label: String,
    #[serde(rename(serialize = "classification_parent", deserialize = "parent"))]
    parent_node: Option<String>,
    parent_id: Option<usize>,
    #[serde(default, deserialize_with = "default_if_empty")]
    leaf: bool,
    lft: Option<usize>,
    rgt: Option<usize>,
    count: Option<usize>,
}

#[derive(Debug)]
pub struct NestedSet {
    pub nodes: Vec<Node>,
    root: Option<usize>,
    children: Vec<Option<Vec<usize>>>,
}

impl NestedSet {
    pub fn new(mut nodes: Vec<Node>) -> error::Result<Self> {
        let mut root = None;
        let mut lookup = BTreeMap::<String, usize>::new();

        for (i, x) in nodes.iter_mut().enumerate() {
            x.pid = Some(i + 1);

            if x.leaf {
                continue;
            }
            if x.parent_node.is_none() {
                root = Some(i);
            }
            lookup.insert(x.node.to_owned(), i);
        }

        let mut children: Vec<Option<Vec<usize>>> = vec![None; nodes.len()];
        for (i, x) in nodes.iter_mut().enumerate() {
            let parent = match x.parent_node.as_ref() {
                Some(p) => *lookup
                    .get(p)
                    .ok_or(Error::ParentNodeNotFoundError(p.to_owned()))?,
                None => continue,
            };

            x.parent_id = Some(parent + 1);

            match children.get_mut(parent).unwrap() {
                Some(x) => x.push(i),
                None => children[parent] = Some(vec![i]),
            }
        }

        Ok(Self {
            nodes,
            root,
            children,
        })
    }

    pub fn rebuild(&mut self) -> error::Result<&Self> {
        if self.root.is_none() {
            Err(Error::RootNodeNotFoundError())?
        }

        fn fill(set: &mut NestedSet, i: usize, n: usize) -> usize {
            {
                let node = set.nodes.get_mut(i).unwrap();
                node.lft = Some(n);
            }

            match set.children.get(i).unwrap().to_owned() {
                Some(ref children) => {
                    let mut r = n;

                    for &ci in children {
                        r = fill(set, ci, r + 1);
                    }

                    {
                        let node = set.nodes.get_mut(i).unwrap();
                        node.rgt = Some(r + 1);
                        node.count = Some(children.len());
                    }

                    r + 1
                }
                None => {
                    {
                        let node = set.nodes.get_mut(i).unwrap();
                        node.rgt = Some(n + 1);
                        node.count = Some(0);
                    }

                    n + 1
                }
            }
        }

        fill(self, self.root.unwrap(), 1);

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{NestedSet, Node};

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
    fn test() {
        let data = vec![
            Node {
                pid: None,
                node: "Clothing".to_owned(),
                node_label: "Clothing".to_owned(),
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
                node_label: "Men's".to_owned(),
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
                node_label: "Women's".to_owned(),
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
                node_label: "Suits".to_owned(),
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
                node_label: "Slacks".to_owned(),
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
                node_label: "Jackets".to_owned(),
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
                node_label: "Dresses".to_owned(),
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
                node_label: "Skirts".to_owned(),
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
                node_label: "Blouses".to_owned(),
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
                node_label: "Evening Gowns".to_owned(),
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
                node_label: "Sun Dresses".to_owned(),
                parent_node: Some("Dresses".to_owned()),
                parent_id: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
        ];

        let mut set = NestedSet::new(data).unwrap();
        let set = set.rebuild().unwrap();

        assert_eq!(set.nodes.get(0).unwrap().pid, Some(1));
        assert_eq!(set.nodes.get(0).unwrap().parent_id, None);
        assert_eq!(set.nodes.get(0).unwrap().lft, Some(1));
        assert_eq!(set.nodes.get(0).unwrap().rgt, Some(22));

        assert_eq!(set.nodes.get(1).unwrap().pid, Some(2));
        assert_eq!(set.nodes.get(1).unwrap().parent_id, Some(1));
        assert_eq!(set.nodes.get(1).unwrap().lft, Some(2));
        assert_eq!(set.nodes.get(1).unwrap().rgt, Some(9));

        assert_eq!(set.nodes.get(2).unwrap().pid, Some(3));
        assert_eq!(set.nodes.get(2).unwrap().parent_id, Some(1));
        assert_eq!(set.nodes.get(2).unwrap().lft, Some(10));
        assert_eq!(set.nodes.get(2).unwrap().rgt, Some(21));

        assert_eq!(set.nodes.get(3).unwrap().pid, Some(4));
        assert_eq!(set.nodes.get(3).unwrap().parent_id, Some(2));
        assert_eq!(set.nodes.get(3).unwrap().lft, Some(3));
        assert_eq!(set.nodes.get(3).unwrap().rgt, Some(8));

        assert_eq!(set.nodes.get(4).unwrap().pid, Some(5));
        assert_eq!(set.nodes.get(4).unwrap().parent_id, Some(4));
        assert_eq!(set.nodes.get(4).unwrap().lft, Some(4));
        assert_eq!(set.nodes.get(4).unwrap().rgt, Some(5));

        assert_eq!(set.nodes.get(5).unwrap().pid, Some(6));
        assert_eq!(set.nodes.get(5).unwrap().parent_id, Some(4));
        assert_eq!(set.nodes.get(5).unwrap().lft, Some(6));
        assert_eq!(set.nodes.get(5).unwrap().rgt, Some(7));

        assert_eq!(set.nodes.get(6).unwrap().pid, Some(7));
        assert_eq!(set.nodes.get(6).unwrap().parent_id, Some(3));
        assert_eq!(set.nodes.get(6).unwrap().lft, Some(11));
        assert_eq!(set.nodes.get(6).unwrap().rgt, Some(16));

        assert_eq!(set.nodes.get(7).unwrap().pid, Some(8));
        assert_eq!(set.nodes.get(7).unwrap().parent_id, Some(3));
        assert_eq!(set.nodes.get(7).unwrap().lft, Some(17));
        assert_eq!(set.nodes.get(7).unwrap().rgt, Some(18));

        assert_eq!(set.nodes.get(8).unwrap().pid, Some(9));
        assert_eq!(set.nodes.get(8).unwrap().parent_id, Some(3));
        assert_eq!(set.nodes.get(8).unwrap().lft, Some(19));
        assert_eq!(set.nodes.get(8).unwrap().rgt, Some(20));

        assert_eq!(set.nodes.get(9).unwrap().pid, Some(10));
        assert_eq!(set.nodes.get(9).unwrap().parent_id, Some(7));
        assert_eq!(set.nodes.get(9).unwrap().lft, Some(12));
        assert_eq!(set.nodes.get(9).unwrap().rgt, Some(13));

        assert_eq!(set.nodes.get(10).unwrap().pid, Some(11));
        assert_eq!(set.nodes.get(10).unwrap().parent_id, Some(7));
        assert_eq!(set.nodes.get(10).unwrap().lft, Some(14));
        assert_eq!(set.nodes.get(10).unwrap().rgt, Some(15));
    }
}
