use ropey::RopeSlice;
use tree_sitter::{Node, TreeCursor};

pub fn find_node_for_point(
    cursor: &mut TreeCursor,
    rope: RopeSlice,
    line: usize,
    col: usize,
) -> bool {
    let mut result = false;

    let query_byte_index = rope.char_to_byte(rope.line_to_char(line) + col);

    loop {
        let current_byte_range = cursor.node().byte_range();

        let in_range = current_byte_range.contains(&query_byte_index);

        result = result || in_range;

        if in_range && cursor.goto_first_child() {
            continue;
        }

        if in_range {
            break;
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }

    result
}

pub fn traverse_rec(mut cursor: TreeCursor, f: impl Fn(&Node)) {
    'outer: loop {
        f(&cursor.node());

        if cursor.goto_first_child() {
            continue;
        }

        if cursor.goto_next_sibling() {
            continue;
        }

        loop {
            if !cursor.goto_parent() {
                break 'outer;
            }

            if cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
