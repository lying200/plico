//! Stable tool identities; hidden tools keep their place in the saved order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Commit,
    Comparison,
    Project,
}
impl Tool {
    pub const ALL: [Self; 3] = [Self::Commit, Self::Comparison, Self::Project];
    pub fn key(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Comparison => "comparison",
            Self::Project => "project",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Order([Tool; 3]);
impl Default for Order {
    fn default() -> Self {
        Self(Tool::ALL)
    }
}
impl Order {
    pub fn parse(text: &str) -> Self {
        let mut tools = Vec::new();
        for key in text.lines() {
            if let Some(tool) = Tool::ALL.into_iter().find(|t| t.key() == key) {
                if !tools.contains(&tool) {
                    tools.push(tool);
                }
            }
        }
        for tool in Tool::ALL {
            if !tools.contains(&tool) {
                tools.push(tool);
            }
        }
        Self(tools.try_into().unwrap())
    }
    pub fn encode(&self) -> String {
        self.0
            .iter()
            .map(|t| t.key())
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn visible(&self, comparison: bool) -> Vec<Tool> {
        self.0
            .iter()
            .copied()
            .filter(|t| comparison || *t != Tool::Comparison)
            .collect()
    }
    /// Move only visible slots, preserving the position of temporarily hidden tools.
    pub fn moved(&self, source: Tool, gap: usize, comparison: bool) -> Self {
        let mut visible = self.visible(comparison);
        let Some(index) = visible.iter().position(|t| *t == source) else {
            return self.clone();
        };
        visible.remove(index);
        visible.insert(gap.min(visible.len()), source);
        let mut result = self.clone();
        let mut values = visible.into_iter();
        for tool in &mut result.0 {
            if comparison || *tool != Tool::Comparison {
                *tool = values.next().unwrap();
            }
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reorder_preserves_hidden_tools_and_round_trips() {
        let order = Order::default().moved(Tool::Project, 0, false);
        assert_eq!(order.0, [Tool::Project, Tool::Comparison, Tool::Commit]);
        assert_eq!(Order::parse(&order.encode()), order);
        assert_eq!(order.moved(Tool::Project, 99, false), Order::default());
        assert_eq!(order.moved(Tool::Comparison, 0, false), order);
    }
    #[test]
    fn tolerates_unknown_duplicate_and_missing_ids() {
        assert_eq!(
            Order::parse("project\nunknown\nproject").0,
            [Tool::Project, Tool::Commit, Tool::Comparison]
        );
        assert_eq!(Order::parse(""), Order::default());
        for tool in Tool::ALL {
            for gap in 0..4 {
                let order = Order::default().moved(tool, gap, true);
                for expected in Tool::ALL {
                    assert_eq!(order.0.iter().filter(|t| **t == expected).count(), 1);
                }
            }
        }
    }
}
