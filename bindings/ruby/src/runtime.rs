//! Ruby runtime bindings for the v1 supported surface.

use magnus::{Error, RArray, RHash, Ruby, Symbol, class, define_module, exception, function, method, prelude::*};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Store {
    next_id: u64,
    customers: Vec<Customer>,
    orders: Vec<Order>,
    products: Vec<Product>,
    inventory: Vec<InventoryItem>,
    carts: Vec<Cart>,
    returns: Vec<Return>,
    shipments: Vec<Shipment>,
}

impl Store {
    fn next_uuid(&mut self) -> String {
        self.next_id += 1;
        format!("00000000-0000-0000-0000-{:012}", self.next_id)
    }
}

type SharedStore = Arc<Mutex<Store>>;

fn lock_store(store: &SharedStore) -> Result<std::sync::MutexGuard<'_, Store>, Error> {
    store
        .lock()
        .map_err(|err| Error::new(exception::runtime_error(), format!("Lock error: {err}")))
}

fn decimal_value(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

fn hash_string(hash: &RHash, key: &str) -> String {
    hash.fetch(Symbol::new(key)).unwrap_or_default()
}

fn hash_i32(hash: &RHash, key: &str, default: i32) -> i32 {
    hash.fetch(Symbol::new(key)).unwrap_or(default)
}

fn hash_f64(hash: &RHash, key: &str, default: f64) -> f64 {
    decimal_value(hash.fetch(Symbol::new(key)).unwrap_or(default))
}

fn push_array<T>(array: &RArray, value: T) -> Result<(), Error>
where
    T: IntoValue,
{
    array.push(value)
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Commerce", free_immediately, size)]
pub struct Commerce {
    store: SharedStore,
}

impl Commerce {
    fn new(_db_path: String) -> Self {
        Self { store: Arc::new(Mutex::new(Store::default())) }
    }

    fn customers(&self) -> Customers {
        Customers { store: self.store.clone() }
    }

    fn orders(&self) -> Orders {
        Orders { store: self.store.clone() }
    }

    fn products(&self) -> Products {
        Products { store: self.store.clone() }
    }

    fn inventory(&self) -> Inventory {
        Inventory { store: self.store.clone() }
    }

    fn returns(&self) -> Returns {
        Returns { store: self.store.clone() }
    }

    fn payments(&self) -> Payments {
        Payments { store: self.store.clone() }
    }

    fn shipments(&self) -> Shipments {
        Shipments { store: self.store.clone() }
    }

    fn warranties(&self) -> Warranties {
        Warranties { _store: self.store.clone() }
    }

    fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders { _store: self.store.clone() }
    }

    fn invoices(&self) -> Invoices {
        Invoices { _store: self.store.clone() }
    }

    fn bom(&self) -> BomApi {
        BomApi { _store: self.store.clone() }
    }

    fn work_orders(&self) -> WorkOrders {
        WorkOrders { _store: self.store.clone() }
    }

    fn carts(&self) -> Carts {
        Carts { store: self.store.clone() }
    }

    fn analytics(&self) -> Analytics {
        Analytics { store: self.store.clone() }
    }

    fn currency(&self) -> CurrencyOps {
        CurrencyOps { _store: self.store.clone() }
    }

    fn subscriptions(&self) -> Subscriptions {
        Subscriptions { _store: self.store.clone() }
    }

    fn promotions(&self) -> Promotions {
        Promotions { _store: self.store.clone() }
    }

    fn tax(&self) -> Tax {
        Tax { _store: self.store.clone() }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Customer", free_immediately, size)]
pub struct Customer {
    id: String,
    email: String,
    first_name: String,
    last_name: String,
    phone: Option<String>,
    accepts_marketing: bool,
}

impl Customer {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn email(&self) -> String {
        self.email.clone()
    }

    fn first_name(&self) -> String {
        self.first_name.clone()
    }

    fn last_name(&self) -> String {
        self.last_name.clone()
    }

    fn phone(&self) -> Option<String> {
        self.phone.clone()
    }

    fn accepts_marketing(&self) -> bool {
        self.accepts_marketing
    }

    fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Customers", free_immediately, size)]
pub struct Customers {
    store: SharedStore,
}

impl Customers {
    fn create(
        &self,
        email: String,
        first_name: String,
        last_name: String,
        phone: Option<String>,
        accepts_marketing: Option<bool>,
    ) -> Result<Customer, Error> {
        let mut store = lock_store(&self.store)?;
        let customer = Customer {
            id: store.next_uuid(),
            email,
            first_name,
            last_name,
            phone,
            accepts_marketing: accepts_marketing.unwrap_or(false),
        };
        store.customers.push(customer.clone());
        Ok(customer)
    }

    fn get(&self, id: String) -> Result<Option<Customer>, Error> {
        let store = lock_store(&self.store)?;
        Ok(store.customers.iter().find(|customer| customer.id == id).cloned())
    }

    fn get_by_email(&self, email: String) -> Result<Option<Customer>, Error> {
        let store = lock_store(&self.store)?;
        Ok(store.customers.iter().find(|customer| customer.email == email).cloned())
    }

    fn list(&self) -> Result<RArray, Error> {
        let store = lock_store(&self.store)?;
        let array = RArray::new();
        for customer in store.customers.iter().cloned() {
            push_array(&array, customer)?;
        }
        Ok(array)
    }

    fn count(&self) -> Result<usize, Error> {
        let store = lock_store(&self.store)?;
        Ok(store.customers.len())
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::OrderItem", free_immediately, size)]
pub struct OrderItem {
    id: String,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
}

impl OrderItem {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn sku(&self) -> String {
        self.sku.clone()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn quantity(&self) -> i32 {
        self.quantity
    }

    fn unit_price(&self) -> f64 {
        self.unit_price
    }

    fn total(&self) -> f64 {
        self.unit_price * f64::from(self.quantity)
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Order", free_immediately, size)]
pub struct Order {
    id: String,
    order_number: String,
    customer_id: String,
    status: String,
    total_amount: f64,
    currency: String,
    items: Vec<OrderItem>,
}

impl Order {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn order_number(&self) -> String {
        self.order_number.clone()
    }

    fn customer_id(&self) -> String {
        self.customer_id.clone()
    }

    fn status(&self) -> String {
        self.status.clone()
    }

    fn total_amount(&self) -> f64 {
        self.total_amount
    }

    fn currency(&self) -> String {
        self.currency.clone()
    }

    fn items(&self) -> Result<RArray, Error> {
        let array = RArray::new();
        for item in self.items.iter().cloned() {
            push_array(&array, item)?;
        }
        Ok(array)
    }

    fn item_count(&self) -> usize {
        self.items.len()
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Orders", free_immediately, size)]
pub struct Orders {
    store: SharedStore,
}

impl Orders {
    fn create(
        &self,
        customer_id: String,
        items: Vec<RHash>,
        currency: Option<String>,
        _notes: Option<String>,
    ) -> Result<Order, Error> {
        let mut store = lock_store(&self.store)?;
        let order_items = items
            .iter()
            .map(|item| OrderItem {
                id: store.next_uuid(),
                sku: hash_string(item, "sku"),
                name: hash_string(item, "name"),
                quantity: hash_i32(item, "quantity", 1),
                unit_price: hash_f64(item, "unit_price", 0.0),
            })
            .collect::<Vec<_>>();
        let total_amount = order_items.iter().map(OrderItem::total).sum();
        let order = Order {
            id: store.next_uuid(),
            order_number: format!("ORD-{:06}", store.orders.len() + 1),
            customer_id,
            status: "created".to_string(),
            total_amount,
            currency: currency.unwrap_or_else(|| "USD".to_string()),
            items: order_items,
        };
        store.orders.push(order.clone());
        Ok(order)
    }

    fn list(&self) -> Result<RArray, Error> {
        let store = lock_store(&self.store)?;
        let array = RArray::new();
        for order in store.orders.iter().cloned() {
            push_array(&array, order)?;
        }
        Ok(array)
    }

    fn ship(&self, id: String, _tracking_number: Option<String>, _carrier: Option<String>) -> Result<Order, Error> {
        let mut store = lock_store(&self.store)?;
        let order = store
            .orders
            .iter_mut()
            .find(|order| order.id == id)
            .ok_or_else(|| Error::new(exception::runtime_error(), "Order not found"))?;
        order.status = "shipped".to_string();
        Ok(order.clone())
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Product", free_immediately, size)]
pub struct Product {
    id: String,
    name: String,
    description: String,
    vendor: String,
    product_type: String,
}

impl Product {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn description(&self) -> String {
        self.description.clone()
    }

    fn vendor(&self) -> String {
        self.vendor.clone()
    }

    fn product_type(&self) -> String {
        self.product_type.clone()
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Products", free_immediately, size)]
pub struct Products {
    store: SharedStore,
}

impl Products {
    fn create(&self, name: String, description: String, vendor: String, product_type: String) -> Result<Product, Error> {
        let mut store = lock_store(&self.store)?;
        let product = Product {
            id: store.next_uuid(),
            name,
            description,
            vendor,
            product_type,
        };
        store.products.push(product.clone());
        Ok(product)
    }

    fn list(&self) -> Result<RArray, Error> {
        let store = lock_store(&self.store)?;
        let array = RArray::new();
        for product in store.products.iter().cloned() {
            push_array(&array, product)?;
        }
        Ok(array)
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::InventoryItem", free_immediately, size)]
pub struct InventoryItem {
    id: String,
    sku: String,
    quantity_on_hand: f64,
    quantity_reserved: f64,
    reorder_point: Option<f64>,
    reorder_quantity: Option<f64>,
}

impl InventoryItem {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn sku(&self) -> String {
        self.sku.clone()
    }

    fn quantity_on_hand(&self) -> f64 {
        self.quantity_on_hand
    }

    fn quantity_reserved(&self) -> f64 {
        self.quantity_reserved
    }

    fn quantity_available(&self) -> f64 {
        self.quantity_on_hand - self.quantity_reserved
    }

    fn reorder_point(&self) -> Option<f64> {
        self.reorder_point
    }

    fn reorder_quantity(&self) -> Option<f64> {
        self.reorder_quantity
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Inventory", free_immediately, size)]
pub struct Inventory {
    store: SharedStore,
}

impl Inventory {
    fn create(
        &self,
        sku: String,
        quantity_on_hand: f64,
        reorder_point: Option<f64>,
        reorder_quantity: Option<f64>,
    ) -> Result<InventoryItem, Error> {
        let mut store = lock_store(&self.store)?;
        let item = InventoryItem {
            id: store.next_uuid(),
            sku,
            quantity_on_hand: decimal_value(quantity_on_hand),
            quantity_reserved: 0.0,
            reorder_point,
            reorder_quantity,
        };
        store.inventory.push(item.clone());
        Ok(item)
    }

    fn adjust(&self, id: String, delta: f64, _reason: String) -> Result<InventoryItem, Error> {
        let mut store = lock_store(&self.store)?;
        let item = store
            .inventory
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| Error::new(exception::runtime_error(), "Inventory item not found"))?;
        item.quantity_on_hand += decimal_value(delta);
        Ok(item.clone())
    }

    fn reserve(&self, id: String, quantity: f64, _reference: Option<String>) -> Result<InventoryItem, Error> {
        let mut store = lock_store(&self.store)?;
        let item = store
            .inventory
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| Error::new(exception::runtime_error(), "Inventory item not found"))?;
        item.quantity_reserved += decimal_value(quantity);
        Ok(item.clone())
    }

    fn release(&self, id: String, quantity: f64) -> Result<InventoryItem, Error> {
        let mut store = lock_store(&self.store)?;
        let item = store
            .inventory
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| Error::new(exception::runtime_error(), "Inventory item not found"))?;
        item.quantity_reserved = (item.quantity_reserved - decimal_value(quantity)).max(0.0);
        Ok(item.clone())
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::CartItem", free_immediately, size)]
pub struct CartItem {
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
}

impl CartItem {
    fn sku(&self) -> String {
        self.sku.clone()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn quantity(&self) -> i32 {
        self.quantity
    }

    fn unit_price(&self) -> f64 {
        self.unit_price
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Cart", free_immediately, size)]
pub struct Cart {
    id: String,
    customer_id: Option<String>,
    currency: String,
    items: Vec<CartItem>,
}

impl Cart {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn customer_id(&self) -> Option<String> {
        self.customer_id.clone()
    }

    fn currency(&self) -> String {
        self.currency.clone()
    }

    fn items(&self) -> Result<RArray, Error> {
        let array = RArray::new();
        for item in self.items.iter().cloned() {
            push_array(&array, item)?;
        }
        Ok(array)
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Carts", free_immediately, size)]
pub struct Carts {
    store: SharedStore,
}

impl Carts {
    fn create(&self, customer_id: Option<String>, currency: String) -> Result<Cart, Error> {
        let mut store = lock_store(&self.store)?;
        let cart = Cart {
            id: store.next_uuid(),
            customer_id,
            currency,
            items: Vec::new(),
        };
        store.carts.push(cart.clone());
        Ok(cart)
    }

    fn add_item(
        &self,
        cart_id: String,
        sku: String,
        name: String,
        quantity: i32,
        unit_price: f64,
    ) -> Result<Cart, Error> {
        let mut store = lock_store(&self.store)?;
        let cart = store
            .carts
            .iter_mut()
            .find(|cart| cart.id == cart_id)
            .ok_or_else(|| Error::new(exception::runtime_error(), "Cart not found"))?;
        cart.items.push(CartItem {
            sku,
            name,
            quantity,
            unit_price: decimal_value(unit_price),
        });
        Ok(cart.clone())
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::SalesSummary", free_immediately, size)]
pub struct SalesSummary {
    total_orders: usize,
    total_revenue: f64,
}

impl SalesSummary {
    fn total_orders(&self) -> usize {
        self.total_orders
    }

    fn total_revenue(&self) -> f64 {
        self.total_revenue
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Analytics", free_immediately, size)]
pub struct Analytics {
    store: SharedStore,
}

impl Analytics {
    fn sales_summary(&self, _days: i32) -> Result<SalesSummary, Error> {
        let store = lock_store(&self.store)?;
        Ok(SalesSummary {
            total_orders: store.orders.len(),
            total_revenue: store.orders.iter().map(|order| order.total_amount).sum(),
        })
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Return", free_immediately, size)]
pub struct Return {
    id: String,
    order_id: String,
    reason: String,
    status: String,
}

impl Return {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn order_id(&self) -> String {
        self.order_id.clone()
    }

    fn reason(&self) -> String {
        self.reason.clone()
    }

    fn status(&self) -> String {
        self.status.clone()
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Returns", free_immediately, size)]
pub struct Returns {
    store: SharedStore,
}

impl Returns {
    fn create(&self, order_id: String, reason: String) -> Result<Return, Error> {
        let mut store = lock_store(&self.store)?;
        let return_request = Return {
            id: store.next_uuid(),
            order_id,
            reason,
            status: "requested".to_string(),
        };
        store.returns.push(return_request.clone());
        Ok(return_request)
    }

    fn approve(&self, id: String, _notes: Option<String>) -> Result<Return, Error> {
        let mut store = lock_store(&self.store)?;
        let return_request = store
            .returns
            .iter_mut()
            .find(|return_request| return_request.id == id)
            .ok_or_else(|| Error::new(exception::runtime_error(), "Return not found"))?;
        return_request.status = "approved".to_string();
        Ok(return_request.clone())
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Payments", free_immediately, size)]
pub struct Payments {
    store: SharedStore,
}

impl Payments {
    fn record(&self, order_id: String, amount: f64, method: String) -> Result<bool, Error> {
        let store = lock_store(&self.store)?;
        let order_exists = store.orders.iter().any(|order| order.id == order_id);
        Ok(order_exists && decimal_value(amount) >= 0.0 && !method.is_empty())
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Shipment", free_immediately, size)]
pub struct Shipment {
    id: String,
    order_id: String,
    carrier: String,
    tracking_number: Option<String>,
    status: String,
}

impl Shipment {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn order_id(&self) -> String {
        self.order_id.clone()
    }

    fn carrier(&self) -> String {
        self.carrier.clone()
    }

    fn tracking_number(&self) -> Option<String> {
        self.tracking_number.clone()
    }

    fn status(&self) -> String {
        self.status.clone()
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Shipments", free_immediately, size)]
pub struct Shipments {
    store: SharedStore,
}

impl Shipments {
    fn create(&self, order_id: String, carrier: String, tracking_number: Option<String>) -> Result<Shipment, Error> {
        let mut store = lock_store(&self.store)?;
        let shipment = Shipment {
            id: store.next_uuid(),
            order_id,
            carrier,
            tracking_number,
            status: "pending".to_string(),
        };
        store.shipments.push(shipment.clone());
        Ok(shipment)
    }

    fn ship(&self, id: String, tracking_number: String) -> Result<Shipment, Error> {
        let mut store = lock_store(&self.store)?;
        let shipment = store
            .shipments
            .iter_mut()
            .find(|shipment| shipment.id == id)
            .ok_or_else(|| Error::new(exception::runtime_error(), "Shipment not found"))?;
        shipment.tracking_number = Some(tracking_number);
        shipment.status = "shipped".to_string();
        Ok(shipment.clone())
    }
}

macro_rules! define_stub_api {
    ($name:ident, $class:literal) => {
        #[derive(Clone)]
        #[magnus::wrap(class = $class, free_immediately, size)]
        pub struct $name {
            _store: SharedStore,
        }
    };
}

define_stub_api!(Warranties, "StateSet::Warranties");
define_stub_api!(PurchaseOrders, "StateSet::PurchaseOrders");
define_stub_api!(Invoices, "StateSet::Invoices");
define_stub_api!(BomApi, "StateSet::BomApi");
define_stub_api!(WorkOrders, "StateSet::WorkOrders");
define_stub_api!(CurrencyOps, "StateSet::CurrencyOps");
define_stub_api!(Subscriptions, "StateSet::Subscriptions");
define_stub_api!(Promotions, "StateSet::Promotions");
define_stub_api!(Tax, "StateSet::Tax");

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("StateSet")?;

    let commerce_class = module.define_class("Commerce", ruby.class_object())?;
    commerce_class.define_singleton_method("new", function!(Commerce::new, 1))?;
    commerce_class.define_method("customers", method!(Commerce::customers, 0))?;
    commerce_class.define_method("orders", method!(Commerce::orders, 0))?;
    commerce_class.define_method("products", method!(Commerce::products, 0))?;
    commerce_class.define_method("inventory", method!(Commerce::inventory, 0))?;
    commerce_class.define_method("returns", method!(Commerce::returns, 0))?;
    commerce_class.define_method("payments", method!(Commerce::payments, 0))?;
    commerce_class.define_method("shipments", method!(Commerce::shipments, 0))?;
    commerce_class.define_method("warranties", method!(Commerce::warranties, 0))?;
    commerce_class.define_method("purchase_orders", method!(Commerce::purchase_orders, 0))?;
    commerce_class.define_method("invoices", method!(Commerce::invoices, 0))?;
    commerce_class.define_method("bom", method!(Commerce::bom, 0))?;
    commerce_class.define_method("work_orders", method!(Commerce::work_orders, 0))?;
    commerce_class.define_method("carts", method!(Commerce::carts, 0))?;
    commerce_class.define_method("analytics", method!(Commerce::analytics, 0))?;
    commerce_class.define_method("currency", method!(Commerce::currency, 0))?;
    commerce_class.define_method("subscriptions", method!(Commerce::subscriptions, 0))?;
    commerce_class.define_method("promotions", method!(Commerce::promotions, 0))?;
    commerce_class.define_method("tax", method!(Commerce::tax, 0))?;

    let customer_class = module.define_class("Customer", ruby.class_object())?;
    customer_class.define_method("id", method!(Customer::id, 0))?;
    customer_class.define_method("email", method!(Customer::email, 0))?;
    customer_class.define_method("first_name", method!(Customer::first_name, 0))?;
    customer_class.define_method("last_name", method!(Customer::last_name, 0))?;
    customer_class.define_method("phone", method!(Customer::phone, 0))?;
    customer_class.define_method("accepts_marketing", method!(Customer::accepts_marketing, 0))?;
    customer_class.define_method("full_name", method!(Customer::full_name, 0))?;

    let customers_class = module.define_class("Customers", ruby.class_object())?;
    customers_class.define_method("create", method!(Customers::create, 5))?;
    customers_class.define_method("get", method!(Customers::get, 1))?;
    customers_class.define_method("get_by_email", method!(Customers::get_by_email, 1))?;
    customers_class.define_method("list", method!(Customers::list, 0))?;
    customers_class.define_method("count", method!(Customers::count, 0))?;

    let order_item_class = module.define_class("OrderItem", ruby.class_object())?;
    order_item_class.define_method("id", method!(OrderItem::id, 0))?;
    order_item_class.define_method("sku", method!(OrderItem::sku, 0))?;
    order_item_class.define_method("name", method!(OrderItem::name, 0))?;
    order_item_class.define_method("quantity", method!(OrderItem::quantity, 0))?;
    order_item_class.define_method("unit_price", method!(OrderItem::unit_price, 0))?;
    order_item_class.define_method("total", method!(OrderItem::total, 0))?;

    let order_class = module.define_class("Order", ruby.class_object())?;
    order_class.define_method("id", method!(Order::id, 0))?;
    order_class.define_method("order_number", method!(Order::order_number, 0))?;
    order_class.define_method("customer_id", method!(Order::customer_id, 0))?;
    order_class.define_method("status", method!(Order::status, 0))?;
    order_class.define_method("total_amount", method!(Order::total_amount, 0))?;
    order_class.define_method("currency", method!(Order::currency, 0))?;
    order_class.define_method("items", method!(Order::items, 0))?;
    order_class.define_method("item_count", method!(Order::item_count, 0))?;

    let orders_class = module.define_class("Orders", ruby.class_object())?;
    orders_class.define_method("create", method!(Orders::create, 4))?;
    orders_class.define_method("list", method!(Orders::list, 0))?;
    orders_class.define_method("ship", method!(Orders::ship, 3))?;

    let product_class = module.define_class("Product", ruby.class_object())?;
    product_class.define_method("id", method!(Product::id, 0))?;
    product_class.define_method("name", method!(Product::name, 0))?;
    product_class.define_method("description", method!(Product::description, 0))?;
    product_class.define_method("vendor", method!(Product::vendor, 0))?;
    product_class.define_method("product_type", method!(Product::product_type, 0))?;

    let products_class = module.define_class("Products", ruby.class_object())?;
    products_class.define_method("create", method!(Products::create, 4))?;
    products_class.define_method("list", method!(Products::list, 0))?;

    let inventory_item_class = module.define_class("InventoryItem", ruby.class_object())?;
    inventory_item_class.define_method("id", method!(InventoryItem::id, 0))?;
    inventory_item_class.define_method("sku", method!(InventoryItem::sku, 0))?;
    inventory_item_class.define_method("quantity_on_hand", method!(InventoryItem::quantity_on_hand, 0))?;
    inventory_item_class.define_method("quantity_reserved", method!(InventoryItem::quantity_reserved, 0))?;
    inventory_item_class.define_method("quantity_available", method!(InventoryItem::quantity_available, 0))?;
    inventory_item_class.define_method("reorder_point", method!(InventoryItem::reorder_point, 0))?;
    inventory_item_class.define_method("reorder_quantity", method!(InventoryItem::reorder_quantity, 0))?;

    let inventory_class = module.define_class("Inventory", ruby.class_object())?;
    inventory_class.define_method("create", method!(Inventory::create, 4))?;
    inventory_class.define_method("adjust", method!(Inventory::adjust, 3))?;
    inventory_class.define_method("reserve", method!(Inventory::reserve, 3))?;
    inventory_class.define_method("release", method!(Inventory::release, 2))?;

    let cart_item_class = module.define_class("CartItem", ruby.class_object())?;
    cart_item_class.define_method("sku", method!(CartItem::sku, 0))?;
    cart_item_class.define_method("name", method!(CartItem::name, 0))?;
    cart_item_class.define_method("quantity", method!(CartItem::quantity, 0))?;
    cart_item_class.define_method("unit_price", method!(CartItem::unit_price, 0))?;

    let cart_class = module.define_class("Cart", ruby.class_object())?;
    cart_class.define_method("id", method!(Cart::id, 0))?;
    cart_class.define_method("customer_id", method!(Cart::customer_id, 0))?;
    cart_class.define_method("currency", method!(Cart::currency, 0))?;
    cart_class.define_method("items", method!(Cart::items, 0))?;

    let carts_class = module.define_class("Carts", ruby.class_object())?;
    carts_class.define_method("create", method!(Carts::create, 2))?;
    carts_class.define_method("add_item", method!(Carts::add_item, 5))?;

    let sales_summary_class = module.define_class("SalesSummary", ruby.class_object())?;
    sales_summary_class.define_method("total_orders", method!(SalesSummary::total_orders, 0))?;
    sales_summary_class.define_method("total_revenue", method!(SalesSummary::total_revenue, 0))?;

    let analytics_class = module.define_class("Analytics", ruby.class_object())?;
    analytics_class.define_method("sales_summary", method!(Analytics::sales_summary, 1))?;

    let return_class = module.define_class("Return", ruby.class_object())?;
    return_class.define_method("id", method!(Return::id, 0))?;
    return_class.define_method("order_id", method!(Return::order_id, 0))?;
    return_class.define_method("reason", method!(Return::reason, 0))?;
    return_class.define_method("status", method!(Return::status, 0))?;

    let returns_class = module.define_class("Returns", ruby.class_object())?;
    returns_class.define_method("create", method!(Returns::create, 2))?;
    returns_class.define_method("approve", method!(Returns::approve, 2))?;

    let payments_class = module.define_class("Payments", ruby.class_object())?;
    payments_class.define_method("record", method!(Payments::record, 3))?;

    let shipment_class = module.define_class("Shipment", ruby.class_object())?;
    shipment_class.define_method("id", method!(Shipment::id, 0))?;
    shipment_class.define_method("order_id", method!(Shipment::order_id, 0))?;
    shipment_class.define_method("carrier", method!(Shipment::carrier, 0))?;
    shipment_class.define_method("tracking_number", method!(Shipment::tracking_number, 0))?;
    shipment_class.define_method("status", method!(Shipment::status, 0))?;

    let shipments_class = module.define_class("Shipments", ruby.class_object())?;
    shipments_class.define_method("create", method!(Shipments::create, 3))?;
    shipments_class.define_method("ship", method!(Shipments::ship, 2))?;

    module.define_class("Warranties", ruby.class_object())?;
    module.define_class("PurchaseOrders", ruby.class_object())?;
    module.define_class("Invoices", ruby.class_object())?;
    module.define_class("BomApi", ruby.class_object())?;
    module.define_class("WorkOrders", ruby.class_object())?;
    module.define_class("CurrencyOps", ruby.class_object())?;
    module.define_class("Subscriptions", ruby.class_object())?;
    module.define_class("Promotions", ruby.class_object())?;
    module.define_class("Tax", ruby.class_object())?;

    Ok(())
}
