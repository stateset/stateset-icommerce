// StateSet Embedded Commerce - Swift example.
//
// Build the native engine first, then run:
//   cargo build -p stateset-swift --release
//   cd examples/swift-basic
//   LD_LIBRARY_PATH=../../target/release swift run -Xlinker -L../../target/release   # Linux
//   DYLD_LIBRARY_PATH=../../target/release swift run -Xlinker -L../../target/release # macOS
//
// Everything below runs inside the Rust engine and is written to
// example-store.db; run it twice and the second run sees the first run's data.

import Foundation
import StateSet

func dec(_ s: String) -> Decimal { Decimal(string: s, locale: Locale(identifier: "en_US_POSIX"))! }

do {
    let commerce = try StateSetCommerce(dbPath: "example-store.db")
    defer { commerce.close() }

    print("=== StateSet Embedded Commerce (Swift) ===\n")

    let tag = UUID().uuidString.prefix(8).lowercased()
    let customer = try commerce.customers.create(email: "alice+\(tag)@example.com", firstName: "Alice",
                                                 lastName: "Smith")
    print("Customer: \(customer.fullName) <\(customer.email)>")

    let sku = "WIDGET-\(tag)"
    let widget = try commerce.products.create(name: "Premium Widget \(sku)", sku: sku, price: dec("29.99"))
    _ = try commerce.products.activate(id: widget.id)
    _ = try commerce.inventory.createItem(sku: sku, name: "Premium Widget", initialQuantity: 100)
    print("Product: \(widget.name) (\(sku)), stock \(try commerce.inventory.getStock(sku: sku)!.totalOnHand)")

    var order = try commerce.orders.create(customerId: customer.id, items: [
        CreateOrderItem(productId: widget.id, sku: sku, name: "Premium Widget", quantity: 3, unitPrice: dec("29.99")),
    ])
    print("Order \(order.orderNumber): \(order.totalAmount) \(order.currency) (\(order.status))")

    order = try commerce.orders.updateStatus(id: order.id, status: .confirmed)
    var payment = try commerce.payments.create(orderId: order.id, amount: order.totalAmount, customerId: customer.id)
    payment = try commerce.payments.complete(id: payment.id)
    print("Payment \(payment.paymentNumber): \(payment.amount) \(payment.status)")

    try commerce.inventory.adjust(sku: sku, quantityDelta: -3, reason: "order fulfillment")
    order = try commerce.orders.ship(id: order.id, trackingNumber: "1Z999AA10123456784")
    print("Shipped with tracking \(order.trackingNumber ?? "-"); stock now \(try commerce.inventory.getStock(sku: sku)!.totalOnHand)")

    let refund = try commerce.payments.refund(paymentId: payment.id, amount: dec("10.00"), reason: "goodwill credit")
    print("Refund \(refund.refundNumber): \(refund.amount)")

    print("\nStore now holds \(try commerce.customers.count()) customers and \(try commerce.orders.count()) orders.")
} catch {
    print("error: \(error)")
    exit(1)
}
