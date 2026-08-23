use std::fmt::Display;
use std::io::{self, Write};

/// Trait for items that can be rendered in a tree hierarchy.
pub trait TreeItem: Display {
    fn children(&self) -> &[Self]
    where
        Self: Sized;
}

/// A generic node in a tree hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode<T> {
    pub value: T,
    pub children: Vec<TreeNode<T>>,
}

impl<T> TreeNode<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            children: Vec::new(),
        }
    }

    pub fn with_children(
        value: T,
        children: Vec<TreeNode<T>>,
    ) -> Self {
        Self { value, children }
    }
}

impl<T: Display> Display for TreeNode<T> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl<T: Display> TreeItem for TreeNode<T> {
    fn children(&self) -> &[Self] {
        &self.children
    }
}

/// Render any collection of [`TreeItem`] roots to the given writer.
pub fn render_tree<T: TreeItem>(
    roots: &[T],
    writer: &mut impl Write,
) -> io::Result<()> {
    render_tree_with(
        roots,
        |node| node.children(),
        |node| node.to_string(),
        writer,
    )
}

/// Render a tree hierarchy using custom child-accessor and format closures.
///
/// Indentation and branch characters:
/// - Dash connector for intermediate children: `├── `
/// - Hook connector for last children:         `└── `
/// - Continuation vertical line:               `│   `
/// - Space continuation for last child:        `    `
pub fn render_tree_with<T, C, F>(
    roots: &[T],
    get_children: C,
    format_node: F,
    writer: &mut impl Write,
) -> io::Result<()>
where
    C: Fn(&T) -> &[T] + Copy,
    F: Fn(&T) -> String + Copy,
{
    for (i, root) in roots.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        writeln!(writer, "{}", format_node(root))?;
        render_subtree(get_children(root), get_children, format_node, "", writer)?;
    }
    Ok(())
}

pub fn render_subtree<T, C, F>(
    nodes: &[T],
    get_children: C,
    format_node: F,
    prefix: &str,
    writer: &mut impl Write,
) -> io::Result<()>
where
    C: Fn(&T) -> &[T] + Copy,
    F: Fn(&T) -> String + Copy,
{
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        writeln!(writer, "{prefix}{connector}{}", format_node(node))?;
        render_subtree(
            get_children(node),
            get_children,
            format_node,
            &format!("{prefix}{child_prefix}"),
            writer,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_tree_hierarchy() {
        let child1 = TreeNode::new("child_1");
        let grandchild = TreeNode::new("grandchild_1");
        let child2 = TreeNode::with_children("child_2", vec![grandchild]);
        let root = TreeNode::with_children("root", vec![child1, child2]);

        let mut buf = Vec::new();
        render_tree(&[root], &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let expected = "\
root
├── child_1
└── child_2
    └── grandchild_1
";
        assert_eq!(output, expected);
    }
}
