use crate::helpers::find_node_for_point;
use ropey::RopeSlice;
use std::{collections::HashMap, ops::Range};
use tree_sitter::Tree;

pub fn goto_definition(
    tree: &Tree,
    rope: RopeSlice,
    line: usize,
    col: usize,
) -> Option<Range<usize>> {
    let root_node = tree.root_node();
    let mut cursor = tree.walk();

    if !cursor.goto_first_child() {
        return None;
    }

    let mut definitions = HashMap::<String, Range<usize>>::new();

    loop {
        let node = cursor.node();

        if node.kind() != "line" {
            if !cursor.goto_next_sibling() {
                break;
            }

            continue;
        }

        if !cursor.goto_first_child() {
            break;
        }

        let reference_node = cursor.node();

        definitions.insert(
            rope.byte_slice(reference_node.byte_range()).to_string(),
            reference_node.byte_range(),
        );

        cursor.goto_parent();

        if !cursor.goto_next_sibling() {
            break;
        }
    }

    cursor.reset(root_node);

    if find_node_for_point(&mut cursor, rope, line, col) {
        definitions
            .get(&rope.byte_slice(cursor.node().byte_range()).to_string())
            .cloned()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::goto_definition;
    use ropey::Rope;
    use tree_sitter::Parser;

    #[test]
    fn test_goto_definition() {
        let mut parser = Parser::new();

        parser
            .set_language(tree_sitter_glicol::language())
            .expect("Error loading Rust grammar");

        let source_code = r#"
~s1: sp \guitar >> mul 0.8;
mix: ~s1 >> mul 0.1;
"#;

        let tree = parser.parse(source_code, None).unwrap();
        let rope = Rope::from_str(source_code);

        let m = goto_definition(&tree, rope.slice(..), 2, 5).unwrap();
        assert_eq!(rope.byte_slice(m).to_string(), "~s1");

        assert!(goto_definition(&tree, rope.slice(..), 2, 10).is_none());
    }

    #[test]
    fn test_goto_definition2() {
        let mut parser = Parser::new();

        parser
            .set_language(tree_sitter_glicol::language())
            .expect("Error loading Rust grammar");

        let source_code = r#"
~a: choose 70 71
~m: choose 67 68

~t1: speed 0.25 >> seq 60 _ _ 69 _ _ 69 _ _ 67 _ 67 >> sp \guitar >> mul 0.15
~t2: speed 0.25 >> seq 64 _ _ 72 _ _ 72 _ _ 73 _ 71 >> sp \guitar >> mul 0.15
~t3: speed 0.25 >> seq ~m _ _ 64 _ _ 74 _ _ 74 _ 74 >> sp \guitar >> mul 0.15
~t4: speed 0.25 >> seq ~a _ _ 79 _ _ 77 _ _ 77 _ 77 >> sp \guitar >> mul 0.15

// ~u1: speed 4.0 >> seq 72 74 76 77 67 69 71 72 >> sawsynth 0.1 0.1 >> lpf 1000.0 1.0 >> mul 0.1
~u2: speed 4.0 >> seq 60 64 67 71 74 71 67 64 60 >> squsynth 0.5 0.1 >> mul ~mod2 >> lpf 400.0 1.0

~mod: sin 800 >> mul 0.1

~mod2: sin 0.3 

~c: choose 88 89 90

~l: choose 48 58

~b1: speed 2.0 >> seq ~l _ ~l _ >> sp \bass3 >> mul 0.2
~b2: speed 2.0 >> seq 79 _ 74 _ >> sp \kick1 >> mul 0.4
~b3: speed 2.0 >> seq ~c _ ~c _~c >> sp \snare1 >> mul 0.3

out: mix ~t.. ~b.. ~u.. >> mul 1

// out: seq 60 61 >> sp \peri007_xbigclang >> mul 0.5
"#;

        let tree = parser.parse(source_code, None).unwrap();
        let rope = Rope::from_str(source_code);

        let m = goto_definition(&tree, rope.slice(..), 6, 24).unwrap();
        assert_eq!(rope.byte_slice(m).to_string(), "~m");

        let m = goto_definition(&tree, rope.slice(..), 7, 24).unwrap();
        assert_eq!(rope.byte_slice(m).to_string(), "~a");
    }
}
