use std::borrow::Cow;

use crate::context::documents::{Context, Document};

use tree_sitter::{Parser, Query, Point, QueryCursor, StreamingIterator, Node, Language, Tree};
use tower_lsp::lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Position, Location, Range, Url};
use tower_lsp::jsonrpc::{Result, Error};

pub fn goto_definition(params: GotoDefinitionParams, context: &impl Context) -> Result<Option<GotoDefinitionResponse>>  {

    // Extract parameters
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    // Find document in memory
    let document = context.get_document(&uri)
        .map_err(|msg| error(&msg))?;

    // Parse the document js content
    let tree = parse_js(&document.text);

    // Extract the searched attribute
    let point = to_point(&position);
    let Some(attribute_name) = ref_at_position(tree.root_node(), &point, document.text.as_bytes()) else {
        return Ok(None)
    };

    // Find the current class
    let class = find_classes(tree.root_node(), &document).into_iter()
        .find(|c| c.contains(&point))
        .ok_or(error("Could not find extjs class around the cursor"))?;

    find_attribute(&attribute_name, &class, &document, context)
}

fn find_attribute(searched_attribute: &str, class: &ExtClass<'_>, document: &Document, context: &impl Context) -> Result<Option<GotoDefinitionResponse>> {

    // Search in local class
    let local_attribute = find_attribute_in_class(class, searched_attribute, document.text.as_bytes());
    if let Some(attribute) = local_attribute {
        return Ok(Some(response(document.url.clone(), &attribute.start_position())));
    }

    // Find parent
    let parent = context.find_file(&possible_filenames(&class.parent))
        .map_err(|err| error(&err))?
        .into_iter().next()
        .ok_or(error(&format!("Could not find definition for class : {}", &class.parent)))?;

    // Parse the parent js source
    let parent_tree = parse_js(&parent.text);

    // Extract the extjs class from the parent tree
    let parent_class = find_classes(parent_tree.root_node(), &parent).into_iter()
        .find(|c| c.name == class.parent)
        .ok_or(error("Could not find class in parent file"))?;

    return find_attribute(searched_attribute, &parent_class, &parent, context)
}

fn js_language() -> Language {
    tree_sitter_javascript::LANGUAGE.into()
}

fn parse_js(src: &str) -> Tree {
    let mut parser = Parser::new();
    parser.set_language(&js_language()).expect("Failed to set js language");
    parser.parse(src, None).expect("Failed to parse the source code")
}

fn ref_at_position(node: Node<'_>, pos: &Point, src_bytes: &[u8]) -> Option<String> {
    let descendant = node.descendant_for_point_range(*pos, *pos)?;

    let previous_words = vec![
        descendant.prev_sibling()?.prev_sibling()?,
        descendant.prev_sibling()?
    ]
        .iter()
        .map(|node| node.utf8_text(src_bytes))
        .map(|text| text.expect("Failed to parse previous words as UTF-8"))
        .collect::<Vec<_>>();

    if previous_words != [ "this", "." ] {
        return None;
    }

    let word = descendant.utf8_text(src_bytes)
        .expect("Failed to parse the word under cursor as UTF-8")
        .to_string();

    Some(word)
}

struct ExtClass<'tree> {
    name: String,
    parent: String,
    body: Node<'tree>
}

impl<'tree> ExtClass<'tree> {
    fn contains(&self, point: &Point) -> bool {
        *point > self.body.start_position() && *point < self.body.end_position()
    }
}

fn find_classes<'tree>(node: Node<'tree>, document: &Document) -> Vec<ExtClass<'tree>> {
    let bytes = document.text.as_bytes();

    let mut result = Vec::new(); // Collect the result in a vector because ownership cannot be taken
                                 // from a StreamingIterator

    // const MyClass = Ext.extend(Ext.ux.Counter, { ...
    let query = "\
(lexical_declaration (variable_declarator
  name: (identifier) @ext_class
  value: (call_expression
    function: (member_expression) @ext_extend (#eq? @ext_extend \"Ext.extend\")
    arguments: (arguments
        . (_) @ext_parent
        . (object) @ext_body
    )
  )
))
";
    let query = Query::new(&js_language(), query).expect("Failed to parse query");
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, node, bytes);

    // Ext.ux.MyClass = Ext.extend(Ext.ux.Counter, { ...
    let query2 = "\
(expression_statement (assignment_expression
  left: (_) @ext_class
  right: (call_expression
    function: (member_expression) @ext_extend (#eq? @ext_extend \"Ext.extend\")
    arguments: (arguments
        . (_) @ext_parent
        . (object) @ext_body
    )
  )
))
";
    let query2 = Query::new(&js_language(), query2).expect("Failed to parse query");
    let mut cursor2 = QueryCursor::new();
    let matches2 = cursor2.matches(&query2, node, bytes);

    matches.chain(matches2)
        .for_each(|match_class| {
            let name = match_class.nodes_for_capture_index(0)
                .next().expect("Failed to capture class name")
                .utf8_text(bytes).expect("Failed to parse class name as UTF-8")
                .to_string();

            let parent = match_class.nodes_for_capture_index(2)
                .next().expect("Failed to capture class parent")
                .utf8_text(bytes).expect("Failed to parse class parent as UTF-8")
                .to_string();

            let body = match_class.nodes_for_capture_index(3)
                .next().expect("Failed to capture class body");

            result.push(ExtClass { name, parent, body })
        });

    result
}

fn find_attribute_in_class<'tree>(class: &ExtClass<'tree>, searched_attribute: &str, bytes: &[u8]) -> Option<Node<'tree>> {
    let query = "(pair key: (property_identifier) @attribute)";
    let query = Query::new(&js_language(), query).expect("Failed to parse query");
    let mut cursor = QueryCursor::new();

    let mut attributes = cursor.matches(&query, class.body.clone(), bytes)
        .map(|attribute| {
            let attribute_node = attribute.nodes_for_capture_index(0)
                .next().expect("Failed to get attribute capture");

            let attribute_name = attribute_node
                .utf8_text(bytes).expect("Failed to parse attribute node content as UTF-8")
                .to_string();

            (attribute_name, attribute_node)
        });

    attributes
        .find(|(attribute_name, _)| attribute_name == searched_attribute)
        .map(|(_, attribute_node)| attribute_node)
        .copied()
}

fn possible_filenames(class_name: &str) -> Vec<&str> {
    let parts = class_name.split(".").collect::<Vec<_>>();
    if parts.len() == 1 {
        return vec![class_name];
    }

    vec![class_name, parts.last().unwrap()]
}

fn response(uri: Url, point: &Point) -> GotoDefinitionResponse {
    GotoDefinitionResponse::Scalar(Location {
        uri,
        range: Range {
            start: to_position(&point),
            end: to_position(&point)
        }
    })
}

fn error(message: &str) -> Error {
    let mut err = Error::internal_error();
    err.message = Cow::from(message.to_string());
    err
}

fn to_point(position: &Position) -> Point {
    Point::new(position.line as usize, position.character as usize)
}

fn to_position(point: &Point) -> Position {
    Position::new(point.row as u32, point.column as u32)
}

#[cfg(test)]
mod tests {

    use super::*;
    use tower_lsp::lsp_types::{
        TextDocumentPositionParams,
        WorkDoneProgressParams,
        PartialResultParams,
        TextDocumentIdentifier,
        Url,
    };

    struct TestContext {
        documents: Vec<(Url, &'static str)>,
        findable_files: Vec<(Url, &'static str, &'static str)>
    }

    impl Context for TestContext {
        fn get_document(&self, url: &Url) -> std::result::Result<Document, String> {
            self.documents.iter()
                .find(|(doc_url, _)| doc_url == url)
                .map(|(doc_url, content)| Document {
                    url: doc_url.clone(),
                    text: content.to_string()
                })
                .ok_or(format!("Failed to find test document : {}", url))
        }
        fn find_file(&self, names: &[&str]) -> std::result::Result<Vec<Document>, String> {
            Ok(self.findable_files.iter()
                .filter(|(_, file_name, _)| names.iter().any(|name| name == file_name))
                .map(|(url, _, content)| Document {
                    url: url.clone(),
                    text: content.to_string()
                })
                .collect()
            )
        }
    }

    fn url(path: &str) -> Url {
        let uri = String::from("file://") + path;
        Url::parse(&uri).unwrap()
    }

    fn params(uri: &str, position: Position) -> GotoDefinitionParams {
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: url(uri)
                },
                position
            },
            work_done_progress_params: WorkDoneProgressParams { work_done_token: None },
            partial_result_params: PartialResultParams { partial_result_token: None }
        }
    }

    #[test]
    fn test_missing_ancester() {
        let context = TestContext {
            documents: vec![
                (url("/here/MyClass.js"), "\
const MyClass = Ext.extend(MyParent, {
    active: false,
    width: 30,
    test: function() {
      this.width = 20;
      this.enable()
    }
})"
                )
            ],
            findable_files: vec![
                (url("/parent/MyParent.js"), "MyParent", "\
const MyParent = Ext.extend(MyGrandPa, {
    active: false,
    width: 30
})"
                ),
            ]
        };

        assert_eq!(
            Err(error("Could not find definition for class : MyGrandPa")),
            goto_definition(params("/here/MyClass.js", Position::new(5, 11)), &context)
        );
    }

    #[test]
    fn test_returns_grand_parent_definition() {
        let context = TestContext {
            documents: vec![
                (url("/here/MyClass.js"), "\
const MyClass = Ext.extend(MyParent, {
    active: false,
    width: 30,
    test: function() {
      this.width = 20;
      this.enable()
    }
})"
                )
            ],
            findable_files: vec![
                (url("/parent/MyParent.js"), "MyParent", "\
const MyParent = Ext.extend(MyGrandPa, {
    active: false,
    width: 30
})"
                ),
                (url("/grand_parent/MyGrandPa.js"), "MyGrandPa", "\
const MyGrandPa = Ext.extend(com.lyra.Base, {
    enable: function() {
        this.active = true;
    }
})"
                )
            ]
        };

        assert_eq!(
            Ok(Some(response(url("/grand_parent/MyGrandPa.js"), &Point::new(1, 4)))),
            goto_definition(params("/here/MyClass.js", Position::new(5, 11)), &context)
        );
    }

    #[test]
    fn test_returns_parent_attribute() {

        let context = TestContext {
            documents: vec![
                (url("/here/MyClass.js"), "\
const MyClass = Ext.extend(MyParent, {
    active: false,
    test: function() {
      this.width = 20;
      this.enable()
    }
})"
                )
            ],
            findable_files: vec![
                (url("/parent/MyParent.js"), "MyParent", "\
const MyParent = Ext.extend(com.lyra.Base, {
    active: false,
    width: 30,
    enable: function() {
        this.active = true;
    }
})"
                )
            ]
        };

        assert_eq!(
            Ok(Some(response(url("/parent/MyParent.js"), &Point::new(2, 4)))),
            goto_definition(params("/here/MyClass.js", Position::new(3, 11)), &context)
        );
    }

    #[test]
    fn test_returns_local_definition() {
        let context = TestContext {
            documents: vec![
                (url("/here/MyClass.js"), "\
const MyClass = Ext.extend(MyParent, {
    active: false,
    width: 30,
    enable: function() {
        this.active = true;
    },
    test: function() {
      this.width = 20;
      this.enable()
    }
})"
                )
            ],
            findable_files: vec![]
        };

        assert_eq!(
            Ok(Some(response(url("/here/MyClass.js"), &Point::new(3, 4)))),
            goto_definition(params("/here/MyClass.js", Position::new(8, 11)), &context)
        );
    }

    #[test]
    fn test_handles_dotted_names() {

        let context = TestContext {
            documents: vec![
                (url("/here/MyClass.js"), "\
const MyClass = Ext.extend(Ext.ux.MyParent, {
    active: false,
    width: 30,
    test: function() {
      this.width = 20;
      this.enable()
    }
})"
                )
            ],
            findable_files: vec![
                (url("/parent/Ext.ux.MyParent.js"), "Ext.ux.MyParent", "\
Ext.ux.MyParent = Ext.extend(com.lyra.Base, {
    active: false,
    width: 30,
    enable: function() {
        this.active = true;
    }
})"
                )
            ]
        };

        assert_eq!(
            Ok(Some(response(url("/parent/Ext.ux.MyParent.js"), &Point::new(3, 4)))),
            goto_definition(params("/here/MyClass.js", Position::new(5, 11)), &context)
        );
    }

    #[test]
    fn test_looks_for_dotted_name_last_part() {

        let context = TestContext {
            documents: vec![
                (url("/here/MyClass.js"), "\
const MyClass = Ext.extend(Ext.ux.MyParent, {
    active: false,
    width: 30,
    test: function() {
      this.width = 20;
      this.enable()
    }
})"
                )
            ],
            findable_files: vec![
                (url("/parent/MyParent.js"), "MyParent", "\
Ext.ux.MyParent = Ext.extend(com.lyra.Base, {
    active: false,
    width: 30,
    enable: function() {
        this.active = true;
    }
})"
                )
            ]
        };

        assert_eq!(
            Ok(Some(response(url("/parent/MyParent.js"), &Point::new(3, 4)))),
            goto_definition(params("/here/MyClass.js", Position::new(5, 11)), &context)
        );
    }

    #[test]
    fn test_returns_none_without_this() {
        let context = TestContext {
            documents: vec![
                (url("/here/MyClass.js"), "\
const MyClass = Ext.extend(MyParent, {
    active: false,
    width: 30,
    enable: function() {
        this.active = true;
    },
    test: function() {
      this.width = 20;
      enable()
    }
})"
                )
            ],
            findable_files: vec![]
        };

        assert_eq!(
            Ok(None),
            goto_definition(params("/here/MyClass.js", Position::new(8, 6)), &context)
        );
    }
}
