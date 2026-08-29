use tree_sitter::{Parser, Query, Point, QueryCursor, StreamingIterator, Node, Language, Tree};

#[allow(unused)]
pub fn go_to_method(src: &str, position: Point) -> Option<Point> {
    let tree = parse_js(src);
    let src_bytes = src.as_bytes();

    let method_name = word_at_position(tree.root_node(), &position, src_bytes)?;
    let class = find_current_class(tree.root_node(), &position, src_bytes)?;
    let method = find_method_in_class(&class, &method_name, src_bytes)?;

    Some(method.start_position())
}

fn js_language() -> Language {
    tree_sitter_javascript::LANGUAGE.into()
}

fn parse_js(src: &str) -> Tree {
    let mut parser = Parser::new();
    parser.set_language(&js_language()).expect("Failed to set js language");
    parser.parse(src, None).expect("Failed to parse the source code")
}

fn word_at_position(node: Node<'_>, pos: &Point, src_bytes: &[u8]) -> Option<String> {
    let name = node.descendant_for_point_range(*pos, *pos)?
        .utf8_text(src_bytes)
        .expect("Failed to parse the word under cursor as UTF-8");
    Some(name.to_string())
}

fn find_current_class<'tree>(node: Node<'tree>, pos: &Point, src_bytes: &[u8]) -> Option<Node<'tree>> {
    let query = "\
(lexical_declaration (variable_declarator
  name: (identifier) @ext_class
  value: (call_expression
    function: (member_expression) @ext_extend (#eq? @ext_extend \"Ext.extend\")
    arguments: (arguments
        . (member_expression) @ext_parent
        . (object) @ext_body
    )
  )
))
";
    let query = Query::new(&js_language(), query).expect("Failed to parse query");

    let mut cursor = QueryCursor::new();
    let mut classes = cursor.matches(&query, node, src_bytes)
        .map(|match_class| {
            match_class.nodes_for_capture_index(3)
                .next().expect("Failed to get body capture")
        });

    classes
        .find(|body| *pos > body.start_position() && *pos < body.end_position())
        .copied()
}

fn find_method_in_class<'tree>(class_body: &Node<'tree>, searched_method: &str, src_bytes: &[u8]) -> Option<Node<'tree>> {
    let query = "(pair key: (property_identifier) @method value: (function_expression))";
    let query = Query::new(&js_language(), query).expect("Failed to parse query");
    let mut cursor = QueryCursor::new();

    let mut methods = cursor.matches(&query, class_body.clone(), src_bytes)
        .map(|method| {
            let method_node = method.nodes_for_capture_index(0)
                .next().expect("Failed to get method capture");

            let method_name = method_node
                .utf8_text(src_bytes).expect("Failed to parse method node content as UTF-8")
                .to_string();

            (method_name, method_node)
        });

    methods
        .find(|(method_name, _)| method_name == searched_method)
        .map(|(_, method_node)| method_node)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_to_method() {
        let src = "\
const MyClass = Ext.extend(com.lyra.MyParent, {
    active: false,
    width: 30,
    enable: function() {
        this.active = true;
    },
    test: function() {
      this.width = 20;
      this.enable()
    }
})";
        let pos = Point::new(8, 11);
        let result = go_to_method(src, pos);
        let expected_result = Point::new(3, 4);

        assert_eq!(expected_result, result.unwrap());
    }
}
