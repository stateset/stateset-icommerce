use rust_decimal_macros::dec;
use stateset_core::{AddCartItem, CartRepository, CreateCart};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_recalculate_avoids_double_tax_when_item_total_includes_tax() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let carts = db.carts();

    let cart = carts.create(CreateCart::default()).expect("create cart");
    carts
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-TAX".to_string(),
                name: "Taxed Item".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            },
        )
        .expect("add cart item");

    let item_id = carts.get_items(cart.id).expect("get cart items")[0].id;
    {
        let conn = db.conn().expect("sqlite connection");
        conn.execute(
            "UPDATE cart_items SET tax_amount = ?, total = ? WHERE id = ?",
            rusqlite::params!["0.80", "10.80", item_id.to_string()],
        )
        .expect("set item tax and total");
    }

    carts.set_tax(cart.id, dec!(0.80)).expect("set cart tax");
    let recalculated = carts.recalculate(cart.id).expect("recalculate cart");

    assert_eq!(recalculated.subtotal, dec!(10.00));
    assert_eq!(recalculated.tax_amount, dec!(0.80));
    assert_eq!(recalculated.grand_total, dec!(10.80));
}
