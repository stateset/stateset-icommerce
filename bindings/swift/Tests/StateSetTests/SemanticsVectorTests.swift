import Foundation
import XCTest
@testable import StateSet

/// The Swift binding against the shared semantic corpus
/// (`bindings/test-vectors/semantics-v1.json`), which pins the meaning of
/// money across every binding and is kept honest against the engine by
/// `crates/stateset-embedded/tests/semantics_vectors.rs`. Categories the Swift
/// surface cannot reach are DECLARED with the reason, and the declaration is
/// checked against the corpus so a new category cannot slip in unnoticed.
///
/// Swift has a real "absent" (`nil`), so unlike Go the blank-currency row is
/// asserted: an explicit `""` must be refused.
final class SemanticsVectorTests: XCTestCase {
    static let notReachable: [String: String] = [
        "currency_decimals": "the Swift surface exposes no currency-scale accessor",
        "canadian_tax_rates": "the Swift surface exposes no Canadian tax lookup",
    ]
    static let asserted = ["decimal_render", "money_scale_enforced", "rejected_inputs", "accepted_inputs"]

    private static func corpus() throws -> [String: Any] {
        let fm = FileManager.default
        var dir = URL(fileURLWithPath: fm.currentDirectoryPath)
        while true {
            let candidate = dir.appendingPathComponent("bindings/test-vectors/semantics-v1.json")
            if fm.fileExists(atPath: candidate.path) {
                let obj = try JSONSerialization.jsonObject(with: Data(contentsOf: candidate)) as! [String: Any]
                XCTAssertEqual(obj["version"] as? Int, 1)
                return obj
            }
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { break }
            dir = parent
        }
        throw XCTSkip("semantics-v1.json not found from \(fm.currentDirectoryPath)")
    }

    private static func rows(_ corpus: [String: Any], _ category: String) -> [[String: Any]] {
        let cats = corpus["categories"] as! [String: Any]
        return (cats[category] as! [String: Any])["rows"] as! [[String: Any]]
    }

    private func dec(_ s: String) -> Decimal { Decimal(string: s, locale: Locale(identifier: "en_US_POSIX"))! }

    private func store() throws -> (StateSetCommerce, Customer) {
        let c = try StateSetCommerce(dbPath: ":memory:")
        return (c, try c.customers.create(email: "vectors@example.com", firstName: "V", lastName: "E"))
    }

    private func line(_ c: Customer, _ price: Decimal, _ qty: Int32 = 1) -> CreateOrderItem {
        CreateOrderItem(productId: c.id, sku: "SKU-1", name: "Widget", quantity: qty, unitPrice: price)
    }

    func testEveryCategoryIsAccountedFor() throws {
        let present = ((try Self.corpus())["categories"] as! [String: Any]).keys.sorted()
        let accounted = (Self.asserted + Self.notReachable.keys).sorted()
        XCTAssertEqual(present, accounted, "assert the new category or declare why not")
    }

    func testDecimalRender() throws {
        let (commerce, cust) = try store()
        defer { commerce.close() }
        var checked = 0
        for row in Self.rows(try Self.corpus(), "decimal_render") {
            guard row["money_scale_ok"] as? Bool == true else { continue }
            let ops = row["operands"] as! [String]
            let items: [CreateOrderItem]
            switch row["op"] as? String {
            case "add": items = ops.map { line(cust, dec($0)) }
            case "mul": items = [line(cust, dec(ops[0]), Int32(NSDecimalNumber(decimal: dec(ops[1])).intValue))]
            default: continue
            }
            let order = try commerce.orders.create(customerId: cust.id, items: items)
            XCTAssertEqual(order.totalAmount, dec(row["expected"] as! String), "\(row["id"]!)")
            checked += 1
        }
        XCTAssertGreaterThan(checked, 0)
    }

    func testMoneyScaleEnforced() throws {
        let (commerce, cust) = try store()
        defer { commerce.close() }
        for row in Self.rows(try Self.corpus(), "money_scale_enforced") where row["currency"] as? String == "USD" {
            let amount = dec(row["amount"] as! String)
            let mustReject = row["must_reject"] as! Bool
            do {
                _ = try commerce.orders.create(customerId: cust.id, items: [line(cust, amount)])
                XCTAssertFalse(mustReject, "\(row["id"]!): \(amount) must be refused")
            } catch let e as StateSetError {
                XCTAssertTrue(mustReject, "\(row["id"]!): \(amount) must be accepted: \(e)")
            }
        }
    }

    func testRejectedInputs() throws {
        let (commerce, cust) = try store()
        defer { commerce.close() }
        for row in Self.rows(try Self.corpus(), "rejected_inputs") {
            let value = row["value"] as! String
            do {
                switch row["kind"] as? String {
                case "currency":
                    _ = try commerce.orders.create(customerId: cust.id, items: [line(cust, dec("10.00"))], currency: value)
                case "uuid":
                    _ = try commerce.orders.create(customerId: value, items: [line(cust, dec("10.00"))])
                default:
                    continue // timestamps and dates have no order-path field
                }
                XCTFail("\(row["id"]!): \(value) must be refused")
            } catch is StateSetError {
                // refused, as required
            }
        }
        XCTAssertEqual(try commerce.orders.list().count, 0)
    }

    func testAcceptedInputs() throws {
        let (commerce, cust) = try store()
        defer { commerce.close() }
        for row in Self.rows(try Self.corpus(), "accepted_inputs") where row["kind"] as? String == "currency" {
            let order = try commerce.orders.create(customerId: cust.id, items: [line(cust, dec("10.00"))],
                                                   currency: row["value"] as? String)
            XCTAssertEqual(order.currency, row["normalizes_to"] as? String)
        }
    }
}
