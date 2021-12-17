use std::collections::BTreeMap;
use std::io;

use serde::{Deserialize, Serialize};

fn main() {
    let stdin = io::stdin();
    let data = serde_json::from_reader(stdin.lock()).unwrap();

    let mut set = NestedSet::new(data);
    let set = set.rebuild();

    let stdout = io::stdout();
    serde_json::to_writer_pretty(io::BufWriter::new(stdout.lock()), &set.nodes).unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
struct Node {
    id: String,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    #[serde(default)]
    leaf: bool,
    lft: Option<usize>,
    rgt: Option<usize>,
    count: Option<usize>,
}

impl Node {
    #[allow(dead_code)]
    pub fn new(id: String, label: String, parent: Option<String>, leaf: bool) -> Self {
        Self {
            id,
            label,
            parent,
            leaf,
            lft: None,
            rgt: None,
            count: None,
        }
    }
}

#[derive(Debug)]
struct NestedSet {
    nodes: Vec<Node>,
    lookup: BTreeMap<String, usize>,
    root: Option<usize>,
    children: Vec<Option<Vec<usize>>>,
}

impl NestedSet {
    pub fn new(nodes: Vec<Node>) -> Self {
        let mut root = None;
        let mut lookup = BTreeMap::<String, usize>::new();

        for (i, x) in nodes.iter().enumerate() {
            if x.leaf {
                continue;
            }
            if x.parent.is_none() {
                root = Some(i);
            }
            lookup.insert(x.id.to_owned(), i);
        }

        let mut children: Vec<Option<Vec<usize>>> = vec![None; nodes.len()];
        for (i, x) in nodes.iter().enumerate() {
            let parent = match x.parent {
                Some(ref p) => *lookup.get(p).unwrap(),
                None => continue,
            };

            match children.get_mut(parent).unwrap() {
                Some(x) => x.push(i),
                None => children[parent] = Some(vec![i]),
            }
        }

        Self {
            nodes,
            lookup,
            root,
            children,
        }
    }

    pub fn rebuild(&mut self) -> &Self {
        if self.root.is_none() {
            panic!("root node not found. remove `\"parent\"` of root object or set to `null`")
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

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Example from https://en.wikipedia.org/wiki/Nested_set_model
    ///
    /// | Node          | Lft | Rgt |
    /// |---------------|-----|-----|
    /// | Clothing      |   1 |  22 |
    /// | Men's         |   2 |   9 |
    /// | Women's       |  10 |  21 |
    /// | Suits         |   3 |   8 |
    /// | Slacks        |   4 |   5 |
    /// | Jackets       |   6 |   7 |
    /// | Dresses       |  11 |  16 |
    /// | Skirts        |  17 |  18 |
    /// | Blouses       |  19 |  20 |
    /// | Evening Gowns |  12 |  13 |
    /// | Sun Dresses   |  14 |  15 |
    #[test]
    fn test() {
        let data = vec![
            Node::new("Clothing".to_owned(), "Clothing".to_owned(), None, false),
            Node::new(
                "Men's".to_owned(),
                "Men's".to_owned(),
                Some("Clothing".to_owned()),
                false,
            ),
            Node::new(
                "Women's".to_owned(),
                "Women's".to_owned(),
                Some("Clothing".to_owned()),
                false,
            ),
            Node::new(
                "Suits".to_owned(),
                "Suits".to_owned(),
                Some("Men's".to_owned()),
                false,
            ),
            Node::new(
                "Slacks".to_owned(),
                "Slacks".to_owned(),
                Some("Suits".to_owned()),
                false,
            ),
            Node::new(
                "Jackets".to_owned(),
                "Jackets".to_owned(),
                Some("Suits".to_owned()),
                false,
            ),
            Node::new(
                "Dresses".to_owned(),
                "Dresses".to_owned(),
                Some("Women's".to_owned()),
                false,
            ),
            Node::new(
                "Skirts".to_owned(),
                "Skirts".to_owned(),
                Some("Women's".to_owned()),
                false,
            ),
            Node::new(
                "Blouses".to_owned(),
                "Blouses".to_owned(),
                Some("Women's".to_owned()),
                false,
            ),
            Node::new(
                "Evening Gowns".to_owned(),
                "Evening Gowns".to_owned(),
                Some("Dresses".to_owned()),
                false,
            ),
            Node::new(
                "Sun Dresses".to_owned(),
                "Sun Dresses".to_owned(),
                Some("Dresses".to_owned()),
                false,
            ),
        ];

        let mut set = NestedSet::new(data);
        let set = set.rebuild();

        assert_eq!(set.nodes.get(0).unwrap().lft, Some(1));
        assert_eq!(set.nodes.get(0).unwrap().rgt, Some(22));
        assert_eq!(set.nodes.get(1).unwrap().lft, Some(2));
        assert_eq!(set.nodes.get(1).unwrap().rgt, Some(9));
        assert_eq!(set.nodes.get(2).unwrap().lft, Some(10));
        assert_eq!(set.nodes.get(2).unwrap().rgt, Some(21));
        assert_eq!(set.nodes.get(3).unwrap().lft, Some(3));
        assert_eq!(set.nodes.get(3).unwrap().rgt, Some(8));
        assert_eq!(set.nodes.get(4).unwrap().lft, Some(4));
        assert_eq!(set.nodes.get(4).unwrap().rgt, Some(5));
        assert_eq!(set.nodes.get(5).unwrap().lft, Some(6));
        assert_eq!(set.nodes.get(5).unwrap().rgt, Some(7));
        assert_eq!(set.nodes.get(6).unwrap().lft, Some(11));
        assert_eq!(set.nodes.get(6).unwrap().rgt, Some(16));
        assert_eq!(set.nodes.get(7).unwrap().lft, Some(17));
        assert_eq!(set.nodes.get(7).unwrap().rgt, Some(18));
        assert_eq!(set.nodes.get(8).unwrap().lft, Some(19));
        assert_eq!(set.nodes.get(8).unwrap().rgt, Some(20));
        assert_eq!(set.nodes.get(9).unwrap().lft, Some(12));
        assert_eq!(set.nodes.get(9).unwrap().rgt, Some(13));
        assert_eq!(set.nodes.get(10).unwrap().lft, Some(14));
        assert_eq!(set.nodes.get(10).unwrap().rgt, Some(15));
    }
}
