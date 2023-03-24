use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::str::FromStr;

use csv::{ReaderBuilder, WriterBuilder};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, VariantNames};
use thiserror::Error;

fn main() -> Result<()> {
    match CLI::from_args().run() {
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
        _ => Ok(()),
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    RuntimeError(String),

    #[error("Parent node not found: {0}")]
    ParentNodeNotFoundError(String),

    #[error("Root node not found. Remove `\"parent\"` from root node or set it to `null`")]
    RootNodeNotFoundError(),

    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    CsvError(#[from] csv::Error),
}

type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Debug, Clone, EnumString, EnumVariantNames)]
#[strum(serialize_all = "snake_case")]
enum Format {
    CSV,
    TSV,
    JSON,
}

#[derive(Debug, StructOpt)]
struct CLI {
    /// Input format
    #[structopt(short, long, possible_values = Format::VARIANTS)]
    from: Option<Format>,

    /// Output format
    #[structopt(short, long, possible_values = Format::VARIANTS)]
    to: Option<Format>,

    /// Output to a file (default: stdout)
    #[structopt(short, long, parse(from_os_str))]
    output: Option<PathBuf>,

    /// File to process (default: stdin)
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
}

impl CLI {
    pub fn run(&self) -> Result<()> {
        let from = match self.from.as_ref() {
            Some(v) => v.clone(),
            None => match self.format_from_input().as_ref() {
                Some(v) => v.clone(),
                None => Err(Error::RuntimeError(format!("missing option --from")))?,
            },
        };
        let to = match self.to.as_ref() {
            Some(v) => v.clone(),
            None => from.clone(),
        };

        let stdin = io::stdin();
        let input: Box<dyn io::Read> = match self.input.as_ref() {
            Some(path) => {
                let f = File::open(path)?;
                Box::new(f)
            }
            None => Box::new(stdin.lock()),
        };

        let data = match from {
            Format::JSON => serde_json::from_reader(BufReader::new(input))?,
            _ => {
                let mut builder = ReaderBuilder::new();
                if let Format::TSV = from {
                    builder.delimiter(b'\t');
                }

                let mut reader = builder.from_reader(BufReader::new(input));
                reader
                    .deserialize()
                    .filter_map(|x| x.ok())
                    .collect::<Vec<Node>>()
            }
        };

        let mut set = NestedSet::new(data)?;
        let set = set.rebuild()?;

        let stdout = io::stdout();
        let output: Box<dyn io::Write> = match self.output.as_ref() {
            Some(path) => {
                let f = File::create(path)?;
                Box::new(f)
            }
            None => Box::new(stdout.lock()),
        };

        match to {
            Format::JSON => serde_json::to_writer_pretty(BufWriter::new(output), &set.nodes)?,
            _ => {
                let mut builder = WriterBuilder::new();
                if let Format::TSV = to {
                    builder.delimiter(b'\t');
                }

                let mut writer = builder.from_writer(BufWriter::new(output));
                for record in &set.nodes {
                    writer.serialize(record)?;
                }
            }
        };

        Ok(())
    }

    fn format_from_input(&self) -> Option<Format> {
        if let Some(input) = self.input.as_ref() {
            if let Some(ext) = input.extension() {
                if let Some(str) = ext.to_str() {
                    return Format::from_str(str).ok();
                }
            }
        }

        None
    }
}

fn default_if_empty<'de, D, T>(de: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Option::<T>::deserialize(de).map(|x| x.unwrap_or_else(|| T::default()))
}

#[derive(Debug, Serialize, Deserialize)]
struct Node {
    id: String,
    label: String,
    parent: Option<String>,
    #[serde(default, deserialize_with = "default_if_empty")]
    leaf: bool,
    lft: Option<usize>,
    rgt: Option<usize>,
    count: Option<usize>,
}

#[derive(Debug)]
struct NestedSet {
    nodes: Vec<Node>,
    lookup: BTreeMap<String, usize>,
    root: Option<usize>,
    children: Vec<Option<Vec<usize>>>,
}

impl NestedSet {
    pub fn new(nodes: Vec<Node>) -> Result<Self> {
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
            let parent = match x.parent.as_ref() {
                Some(p) => *lookup
                    .get(p)
                    .ok_or(Error::ParentNodeNotFoundError(p.to_owned()))?,
                None => continue,
            };

            match children.get_mut(parent).unwrap() {
                Some(x) => x.push(i),
                None => children[parent] = Some(vec![i]),
            }
        }

        Ok(Self {
            nodes,
            lookup,
            root,
            children,
        })
    }

    pub fn rebuild(&mut self) -> Result<&Self> {
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
            Node {
                id: "Clothing".to_owned(),
                label: "Clothing".to_owned(),
                parent: None,
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Men's".to_owned(),
                label: "Men's".to_owned(),
                parent: Some("Clothing".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Women's".to_owned(),
                label: "Women's".to_owned(),
                parent: Some("Clothing".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Suits".to_owned(),
                label: "Suits".to_owned(),
                parent: Some("Men's".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Slacks".to_owned(),
                label: "Slacks".to_owned(),
                parent: Some("Suits".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Jackets".to_owned(),
                label: "Jackets".to_owned(),
                parent: Some("Suits".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Dresses".to_owned(),
                label: "Dresses".to_owned(),
                parent: Some("Women's".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Skirts".to_owned(),
                label: "Skirts".to_owned(),
                parent: Some("Women's".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Blouses".to_owned(),
                label: "Blouses".to_owned(),
                parent: Some("Women's".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Evening Gowns".to_owned(),
                label: "Evening Gowns".to_owned(),
                parent: Some("Dresses".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
            Node {
                id: "Sun Dresses".to_owned(),
                label: "Sun Dresses".to_owned(),
                parent: Some("Dresses".to_owned()),
                leaf: false,
                lft: None,
                rgt: None,
                count: None,
            },
        ];

        let mut set = NestedSet::new(data).unwrap();
        let set = set.rebuild().unwrap();

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
