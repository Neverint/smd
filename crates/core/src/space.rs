//! Space Lens tree: built once by the disk walker, then browsed one folder at a time.
//!
//! The walker produces a [`RawNode`] tree. [`Arena`] flattens it into one vector
//! with children stored next to each other and sorted by size, so the UI can ask
//! for any folder by id without sending the whole tree.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Dir,
    File,
    /// Small files of one folder grouped together (see the walker's keep rules).
    Others,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawNode {
    pub name: String,
    pub kind: NodeKind,
    /// Bytes, including everything below.
    pub size: u64,
    /// Number of files, including everything below.
    pub files: u64,
    /// The folder could not be read (access denied); its size is unknown.
    pub denied: bool,
    pub children: Vec<RawNode>,
}

impl RawNode {
    pub fn file(name: impl Into<String>, size: u64) -> Self {
        RawNode {
            name: name.into(),
            kind: NodeKind::File,
            size,
            files: 1,
            denied: false,
            children: Vec::new(),
        }
    }

    pub fn others(count: u64, size: u64) -> Self {
        RawNode {
            name: String::new(),
            kind: NodeKind::Others,
            size,
            files: count,
            denied: false,
            children: Vec::new(),
        }
    }

    /// A folder whose size and file count are the sums of its children.
    pub fn dir(name: impl Into<String>, children: Vec<RawNode>) -> Self {
        let size = children.iter().map(|c| c.size).sum();
        let files = children.iter().map(|c| c.files).sum();
        RawNode {
            name: name.into(),
            kind: NodeKind::Dir,
            size,
            files,
            denied: false,
            children,
        }
    }
}

const NO_PARENT: u32 = u32::MAX;

#[derive(Debug, Clone)]
struct FlatNode {
    name: Box<str>,
    kind: NodeKind,
    size: u64,
    files: u64,
    denied: bool,
    parent: u32,
    first_child: u32,
    child_count: u32,
}

/// One folder as the UI sees it, with its children down to the requested depth.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewNode {
    pub id: u32,
    pub name: String,
    pub kind: NodeKind,
    pub size: u64,
    pub files: u64,
    pub denied: bool,
    /// Number of children this node has in total (loaded or not).
    pub child_count: u32,
    pub children: Vec<ViewNode>,
    /// Children not sent because of the per-folder limit: count and total size.
    pub hidden_count: u32,
    pub hidden_size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Crumb {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Arena {
    root_path: String,
    nodes: Vec<FlatNode>,
}

impl Arena {
    /// Flattens the tree breadth-first; children are sorted largest first.
    pub fn from_tree(root_path: impl Into<String>, mut root: RawNode) -> Self {
        let root_path = root_path.into();
        if root.name.is_empty() {
            root.name = root_path.clone();
        }
        let mut nodes = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        nodes.push(flat(&root, NO_PARENT));
        queue.push_back((0u32, root));
        while let Some((id, mut node)) = queue.pop_front() {
            node.children
                .sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
            let first = nodes.len() as u32;
            nodes[id as usize].first_child = first;
            nodes[id as usize].child_count = node.children.len() as u32;
            for child in &node.children {
                nodes.push(flat(child, id));
            }
            for (i, child) in node.children.into_iter().enumerate() {
                if !child.children.is_empty() {
                    queue.push_back((first + i as u32, child));
                }
            }
        }
        Arena { root_path, nodes }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn root_path(&self) -> &str {
        &self.root_path
    }

    /// A folder with its children down to `depth` levels, at most `max_children` per folder.
    pub fn view(&self, id: u32, depth: u8, max_children: usize) -> Option<ViewNode> {
        let node = self.nodes.get(id as usize)?;
        let mut children = Vec::new();
        let (mut hidden_count, mut hidden_size) = (0u32, 0u64);
        if depth > 0 {
            for i in 0..node.child_count {
                let child_id = node.first_child + i;
                if (i as usize) < max_children {
                    children.push(self.view(child_id, depth - 1, max_children)?);
                } else {
                    hidden_count += 1;
                    hidden_size += self.nodes[child_id as usize].size;
                }
            }
        }
        Some(ViewNode {
            id,
            name: node.name.to_string(),
            kind: node.kind,
            size: node.size,
            files: node.files,
            denied: node.denied,
            child_count: node.child_count,
            children,
            hidden_count,
            hidden_size,
        })
    }

    /// Root first, `id` last.
    pub fn crumbs(&self, id: u32) -> Vec<Crumb> {
        let mut out = Vec::new();
        let mut cur = id;
        while let Some(node) = self.nodes.get(cur as usize) {
            out.push(Crumb {
                id: cur,
                name: node.name.to_string(),
            });
            if node.parent == NO_PARENT {
                break;
            }
            cur = node.parent;
        }
        out.reverse();
        out
    }

    /// Full path of a file or folder. `None` for grouped small files.
    pub fn path_of(&self, id: u32) -> Option<String> {
        let node = self.nodes.get(id as usize)?;
        if node.kind == NodeKind::Others {
            return None;
        }
        let crumbs = self.crumbs(id);
        let mut path = self.root_path.trim_end_matches(['\\', '/']).to_string();
        for crumb in crumbs.iter().skip(1) {
            path.push('\\');
            path.push_str(&crumb.name);
        }
        if crumbs.len() == 1 && self.root_path.ends_with(['\\', '/']) {
            return Some(self.root_path.clone());
        }
        Some(path)
    }
}

fn flat(node: &RawNode, parent: u32) -> FlatNode {
    FlatNode {
        name: node.name.clone().into_boxed_str(),
        kind: node.kind,
        size: node.size,
        files: node.files,
        denied: node.denied,
        parent,
        first_child: 0,
        child_count: 0,
    }
}

/// Space used by one file extension across the scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtStat {
    /// Lower-case, without the dot. Empty for files without an extension.
    pub ext: String,
    pub bytes: u64,
    pub files: u64,
}

/// Largest extensions first, at most `limit`; the rest is summed into one entry named `*`.
pub fn top_extensions(all: impl IntoIterator<Item = ExtStat>, limit: usize) -> Vec<ExtStat> {
    let mut v: Vec<ExtStat> = all.into_iter().collect();
    v.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.ext.cmp(&b.ext)));
    if v.len() > limit {
        let rest = v.split_off(limit);
        v.push(ExtStat {
            ext: "*".into(),
            bytes: rest.iter().map(|e| e.bytes).sum(),
            files: rest.iter().map(|e| e.files).sum(),
        });
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Arena {
        let tree = RawNode::dir(
            "",
            vec![
                RawNode::dir("small", vec![RawNode::file("a.txt", 10)]),
                RawNode::dir(
                    "big",
                    vec![
                        RawNode::file("x.iso", 500),
                        RawNode::others(3, 20),
                        RawNode::dir("sub", vec![RawNode::file("y.bin", 100)]),
                    ],
                ),
                RawNode::file("root.log", 50),
            ],
        );
        Arena::from_tree(r"D:\data", tree)
    }

    #[test]
    fn sizes_add_up_and_children_are_sorted() {
        let a = sample();
        let root = a.view(0, 1, 10).unwrap();
        assert_eq!(root.size, 10 + 500 + 20 + 100 + 50);
        assert_eq!(root.files, 1 + 1 + 3 + 1 + 1);
        let names: Vec<_> = root.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["big", "root.log", "small"]);
        assert!(
            root.children[0].children.is_empty(),
            "depth 1 does not load grandchildren"
        );
        assert_eq!(root.children[0].child_count, 3);
    }

    #[test]
    fn view_limits_children() {
        let a = sample();
        let root = a.view(0, 2, 2).unwrap();
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.hidden_count, 1);
        assert_eq!(root.hidden_size, 10);
        let big = &root.children[0];
        assert_eq!(big.children.len(), 2);
        assert_eq!(big.hidden_count, 1);
    }

    #[test]
    fn paths_and_crumbs() {
        let a = sample();
        let big = a.view(0, 1, 10).unwrap().children[0].id;
        let big_view = a.view(big, 1, 10).unwrap();
        let sub = big_view
            .children
            .iter()
            .find(|c| c.name == "sub")
            .unwrap()
            .id;
        let y = a.view(sub, 1, 10).unwrap().children[0].id;
        assert_eq!(a.path_of(y).as_deref(), Some(r"D:\data\big\sub\y.bin"));
        assert_eq!(a.path_of(0).as_deref(), Some(r"D:\data"));
        let others = big_view
            .children
            .iter()
            .find(|c| c.kind == NodeKind::Others)
            .unwrap()
            .id;
        assert_eq!(a.path_of(others), None);
        let names: Vec<_> = a.crumbs(y).into_iter().map(|c| c.name).collect();
        assert_eq!(names, [r"D:\data", "big", "sub", "y.bin"]);
    }

    #[test]
    fn drive_root_path() {
        let a = Arena::from_tree(r"C:\", RawNode::dir("", vec![RawNode::file("f", 1)]));
        assert_eq!(a.path_of(0).as_deref(), Some(r"C:\"));
        let f = a.view(0, 1, 5).unwrap().children[0].id;
        assert_eq!(a.path_of(f).as_deref(), Some(r"C:\f"));
    }

    #[test]
    fn unknown_ids() {
        let a = sample();
        assert!(a.view(999, 1, 5).is_none());
        assert!(a.crumbs(999).is_empty());
    }

    #[test]
    fn extensions_are_ranked_and_capped() {
        let stats = vec![
            ExtStat {
                ext: "mp4".into(),
                bytes: 900,
                files: 2,
            },
            ExtStat {
                ext: "txt".into(),
                bytes: 5,
                files: 50,
            },
            ExtStat {
                ext: "iso".into(),
                bytes: 700,
                files: 1,
            },
            ExtStat {
                ext: "".into(),
                bytes: 10,
                files: 4,
            },
        ];
        let top = top_extensions(stats, 2);
        assert_eq!(
            top.iter().map(|e| e.ext.as_str()).collect::<Vec<_>>(),
            ["mp4", "iso", "*"]
        );
        assert_eq!(top[2].bytes, 15);
        assert_eq!(top[2].files, 54);
    }
}
