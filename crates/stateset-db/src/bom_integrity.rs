//! Validation shared by every BOM write path.
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CreateBomComponent, Result};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub(crate) fn component(input: &CreateBomComponent) -> Result<()> {
    if input.quantity <= Decimal::ZERO {
        return Err(CommerceError::ValidationError(
            "BOM component quantity must be positive".into(),
        ));
    }
    if input.component_sku.as_ref().is_some_and(|sku| sku.trim().is_empty()) {
        return Err(CommerceError::ValidationError("BOM component SKU cannot be blank".into()));
    }
    Ok(())
}

pub(crate) fn graph(root: Uuid, edges: Vec<(Uuid, Option<Uuid>, Option<Uuid>)>) -> Result<()> {
    let mut graph: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    let mut mismatched = HashSet::new();
    for (parent, explicit, sku_product) in edges {
        if explicit.is_some() && sku_product.is_some() && explicit != sku_product {
            mismatched.insert(parent);
        }
        if let Some(child) = explicit.or(sku_product) {
            graph.entry(parent).or_default().push(child);
        }
    }
    fn visit(
        node: Uuid,
        graph: &HashMap<Uuid, Vec<Uuid>>,
        mismatched: &HashSet<Uuid>,
        path: &mut HashSet<Uuid>,
        done: &mut HashSet<Uuid>,
    ) -> Result<()> {
        if mismatched.contains(&node) {
            return Err(CommerceError::ValidationError(
                "BOM component product and SKU identify different products".into(),
            ));
        }
        if done.contains(&node) {
            return Ok(());
        }
        if path.len() >= 64 || !path.insert(node) {
            return Err(CommerceError::ValidationError(
                "BOM components contain a cycle or exceed 64 assembly levels".into(),
            ));
        }
        if let Some(children) = graph.get(&node) {
            for child in children {
                visit(*child, graph, mismatched, path, done)?;
            }
        }
        path.remove(&node);
        done.insert(node);
        Ok(())
    }
    visit(root, &graph, &mismatched, &mut HashSet::new(), &mut HashSet::new())
}
