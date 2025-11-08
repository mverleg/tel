
pub fn vec_and<T>(mut items: Vec<T>, addition: Option<T>) -> Vec<T> {
    if let Some(addition) = addition {
        items.push(addition);
    }
    items
}
