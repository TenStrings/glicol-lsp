use crate::helpers::find_node_for_point;
use once_cell::sync::Lazy;
use ropey::RopeSlice;
use std::collections::HashMap;
use tree_sitter::Tree;

const GLICOL_API: &str = include_str!("../glicol/js/src/glicol-api.json");

static NODE_HOVER_DOCS: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let docs: HashMap<String, DocEntry> = serde_json::from_str(GLICOL_API).unwrap();

    docs.into_iter()
        .map(|(k, v)| (k, v.to_markdown()))
        .collect()
});

#[derive(serde::Deserialize)]
pub struct DocEntry {
    description: Option<String>,
    parameters: Option<Vec<serde_json::Value>>,
    input: Option<String>,
    output: Option<String>,
    example: Option<String>,
}

pub fn hover(tree: &Tree, rope: RopeSlice, line: usize, col: usize) -> Option<String> {
    let mut cursor = tree.walk();

    if find_node_for_point(&mut cursor, rope, line, col) {
        NODE_HOVER_DOCS
            .get(&rope.byte_slice(cursor.node().byte_range()).to_string())
            .cloned()
    } else {
        None
    }
}

impl DocEntry {
    fn to_markdown(&self) -> String {
        let mut result = String::new();
        if let Some(description) = self
            .description
            .as_ref()
            .filter(|description| !description.is_empty())
        {
            result.push_str(&format!("## Description\n{}\n", description));
        }

        if let Some(input) = self.input.as_ref().filter(|input| !input.is_empty()) {
            result.push_str(&format!("## Input\n{}\n", input));
        }

        if let Some(output) = self.output.as_ref().filter(|output| !output.is_empty()) {
            result.push_str(&format!("## Output\n{}\n", output));
        }

        if let Some(example) = self.example.as_ref().filter(|example| !example.is_empty()) {
            result.push_str(&format!("## Example\n```glicol\n{}\n```\n", example));
        }

        if let Some(parameters) = &self.parameters {
            result.push_str(&format!(
                "## Parameters\n```json\n{}\n```\n",
                serde_json::to_string(parameters).unwrap()
            ));
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use ropey::Rope;
    use tree_sitter::Parser;

    use crate::hover::hover;

    #[test]
    fn test_parse() {
        let mut parser = Parser::new();

        parser
            .set_language(tree_sitter_glicol::language())
            .expect("Error loading Rust grammar");

        let source_code = r#"
~a: choose 70 71
"#;

        let tree = parser.parse(source_code, None).unwrap();
        let rope = Rope::from_str(source_code);

        dbg!(hover(&tree, rope.slice(..), 1, 5).unwrap());
    }
}
