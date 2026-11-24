use super::document_node::DocumentNode;

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentOutput {
    pub documents: Vec<DocumentNode>,
    pub more_documents: bool,
}
