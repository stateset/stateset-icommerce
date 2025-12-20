<?php

declare(strict_types=1);

use PHPUnit\Framework\TestCase;
use StateSet\Commerce;
use StateSet\Customer;
use StateSet\Customers;
use StateSet\Order;
use StateSet\Orders;
use StateSet\Product;
use StateSet\Products;
use StateSet\InventoryItem;
use StateSet\Inventory;
use StateSet\Cart;
use StateSet\Carts;
use StateSet\SalesSummary;
use StateSet\Analytics;

class CommerceTest extends TestCase
{
    private Commerce $commerce;

    protected function setUp(): void
    {
        $this->commerce = new Commerce(':memory:');
    }

    public function testNewCommerce(): void
    {
        $commerce = new Commerce(':memory:');
        $this->assertInstanceOf(Commerce::class, $commerce);
    }

    public function testNewCommerceWithFilePath(): void
    {
        $dbPath = '/tmp/test_stateset_php.db';
        if (file_exists($dbPath)) {
            unlink($dbPath);
        }

        $commerce = new Commerce($dbPath);
        $this->assertInstanceOf(Commerce::class, $commerce);

        if (file_exists($dbPath)) {
            unlink($dbPath);
        }
    }

    // ========== Customers API ==========

    public function testCustomersApiInstance(): void
    {
        $this->assertInstanceOf(Customers::class, $this->commerce->customers());
    }

    public function testCreateCustomer(): void
    {
        $customer = $this->commerce->customers()->create(
            'alice@example.com',
            'Alice',
            'Smith',
            null,
            null
        );

        $this->assertInstanceOf(Customer::class, $customer);
        $this->assertEquals('alice@example.com', $customer->getEmail());
        $this->assertEquals('Alice', $customer->getFirstName());
        $this->assertEquals('Smith', $customer->getLastName());
        $this->assertEquals('Alice Smith', $customer->getFullName());
    }

    public function testCreateCustomerWithPhone(): void
    {
        $customer = $this->commerce->customers()->create(
            'bob@example.com',
            'Bob',
            'Jones',
            '+1-555-1234',
            true
        );

        $this->assertEquals('+1-555-1234', $customer->getPhone());
        $this->assertTrue($customer->getAcceptsMarketing());
    }

    public function testGetCustomerById(): void
    {
        $created = $this->commerce->customers()->create(
            'test@example.com',
            'Test',
            'User',
            null,
            null
        );
        $found = $this->commerce->customers()->get($created->getId());

        $this->assertNotNull($found);
        $this->assertEquals($created->getId(), $found->getId());
    }

    public function testGetNonExistentCustomer(): void
    {
        $found = $this->commerce->customers()->get('00000000-0000-0000-0000-000000000000');
        $this->assertNull($found);
    }

    public function testGetCustomerByEmail(): void
    {
        $this->commerce->customers()->create(
            'unique@example.com',
            'Unique',
            'User',
            null,
            null
        );
        $found = $this->commerce->customers()->getByEmail('unique@example.com');

        $this->assertNotNull($found);
        $this->assertEquals('unique@example.com', $found->getEmail());
    }

    public function testListCustomers(): void
    {
        $this->commerce->customers()->create('list1@example.com', 'List', 'One', null, null);
        $this->commerce->customers()->create('list2@example.com', 'List', 'Two', null, null);

        $customers = $this->commerce->customers()->list();
        $this->assertIsArray($customers);
        $this->assertGreaterThanOrEqual(2, count($customers));
    }

    public function testCountCustomers(): void
    {
        $initialCount = $this->commerce->customers()->count();
        $this->commerce->customers()->create('count@example.com', 'Count', 'Test', null, null);

        $this->assertEquals($initialCount + 1, $this->commerce->customers()->count());
    }

    // ========== Orders API ==========

    public function testOrdersApiInstance(): void
    {
        $this->assertInstanceOf(Orders::class, $this->commerce->orders());
    }

    public function testCreateOrder(): void
    {
        $customer = $this->commerce->customers()->create(
            'order@example.com',
            'Order',
            'Test',
            null,
            null
        );

        $items = [
            ['sku' => 'WIDGET-001', 'name' => 'Widget', 'quantity' => 2, 'unit_price' => 29.99]
        ];

        $order = $this->commerce->orders()->create($customer->getId(), $items, 'USD', null);

        $this->assertInstanceOf(Order::class, $order);
        $this->assertEquals($customer->getId(), $order->getCustomerId());
        $this->assertEquals('USD', $order->getCurrency());
        $this->assertEquals(1, $order->getItemCount());
    }

    public function testListOrders(): void
    {
        $orders = $this->commerce->orders()->list();
        $this->assertIsArray($orders);
    }

    // ========== Products API ==========

    public function testProductsApiInstance(): void
    {
        $this->assertInstanceOf(Products::class, $this->commerce->products());
    }

    public function testCreateProduct(): void
    {
        $product = $this->commerce->products()->create(
            'Test Product',
            'A test product',
            'TestVendor',
            'Widget'
        );

        $this->assertInstanceOf(Product::class, $product);
        $this->assertEquals('Test Product', $product->getName());
        $this->assertEquals('A test product', $product->getDescription());
        $this->assertEquals('TestVendor', $product->getVendor());
    }

    public function testListProducts(): void
    {
        $products = $this->commerce->products()->list();
        $this->assertIsArray($products);
    }

    // ========== Inventory API ==========

    public function testInventoryApiInstance(): void
    {
        $this->assertInstanceOf(Inventory::class, $this->commerce->inventory());
    }

    public function testCreateInventoryItem(): void
    {
        $item = $this->commerce->inventory()->create('SKU-001', 100, 10, 50);

        $this->assertInstanceOf(InventoryItem::class, $item);
        $this->assertEquals('SKU-001', $item->getSku());
        $this->assertEquals(100, $item->getQuantityOnHand());
        $this->assertEquals(100, $item->getQuantityAvailable());
    }

    public function testAdjustInventory(): void
    {
        $item = $this->commerce->inventory()->create('SKU-ADJ', 50, null, null);
        $adjusted = $this->commerce->inventory()->adjust($item->getId(), -10, 'Sold 10 units');

        $this->assertEquals(40, $adjusted->getQuantityOnHand());
    }

    public function testReserveAndReleaseInventory(): void
    {
        $item = $this->commerce->inventory()->create('SKU-RES', 100, null, null);

        $reserved = $this->commerce->inventory()->reserve($item->getId(), 20, null);
        $this->assertEquals(20, $reserved->getQuantityReserved());
        $this->assertEquals(80, $reserved->getQuantityAvailable());

        $released = $this->commerce->inventory()->release($item->getId(), 10);
        $this->assertEquals(10, $released->getQuantityReserved());
        $this->assertEquals(90, $released->getQuantityAvailable());
    }

    // ========== Carts API ==========

    public function testCartsApiInstance(): void
    {
        $this->assertInstanceOf(Carts::class, $this->commerce->carts());
    }

    public function testCreateCart(): void
    {
        $cart = $this->commerce->carts()->create(null, 'USD');

        $this->assertInstanceOf(Cart::class, $cart);
        $this->assertEquals('USD', $cart->getCurrency());
        $this->assertEmpty($cart->getItems());
    }

    public function testAddItemToCart(): void
    {
        $cart = $this->commerce->carts()->create(null, 'USD');
        $updated = $this->commerce->carts()->addItem(
            $cart->getId(),
            'SKU-001',
            'Test Item',
            2,
            19.99
        );

        $this->assertCount(1, $updated->getItems());
        $this->assertEquals('SKU-001', $updated->getItems()[0]->getSku());
        $this->assertEquals(2, $updated->getItems()[0]->getQuantity());
    }

    // ========== Analytics API ==========

    public function testAnalyticsApiInstance(): void
    {
        $this->assertInstanceOf(Analytics::class, $this->commerce->analytics());
    }

    public function testSalesSummary(): void
    {
        $summary = $this->commerce->analytics()->salesSummary(30);

        $this->assertInstanceOf(SalesSummary::class, $summary);
        $this->assertGreaterThanOrEqual(0, $summary->getTotalOrders());
        $this->assertGreaterThanOrEqual(0, $summary->getTotalRevenue());
    }

    // ========== All API Accessors ==========

    public function testAllApiAccessors(): void
    {
        $this->assertInstanceOf(Customers::class, $this->commerce->customers());
        $this->assertInstanceOf(Orders::class, $this->commerce->orders());
        $this->assertInstanceOf(Products::class, $this->commerce->products());
        $this->assertInstanceOf(Inventory::class, $this->commerce->inventory());
        // Returns uses ReturnRequest class in PHP to avoid reserved word conflict
        $this->assertNotNull($this->commerce->returns());
        $this->assertNotNull($this->commerce->payments());
        $this->assertNotNull($this->commerce->shipments());
        $this->assertNotNull($this->commerce->warranties());
        $this->assertNotNull($this->commerce->purchase_orders());
        $this->assertNotNull($this->commerce->invoices());
        $this->assertNotNull($this->commerce->bom());
        $this->assertNotNull($this->commerce->work_orders());
        $this->assertInstanceOf(Carts::class, $this->commerce->carts());
        $this->assertInstanceOf(Analytics::class, $this->commerce->analytics());
        $this->assertNotNull($this->commerce->currency());
        $this->assertNotNull($this->commerce->subscriptions());
        $this->assertNotNull($this->commerce->promotions());
        $this->assertNotNull($this->commerce->tax());
    }
}
