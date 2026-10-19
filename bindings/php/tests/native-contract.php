<?php

declare(strict_types=1);

// Run without Composer's stubs. Every class and operation must come from the
// extension loaded with `php -d extension=... tests/native-contract.php`.
function check(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$classes = [
    'Commerce', 'Customer', 'Customers', 'OrderItem', 'Order', 'Orders',
    'ProductVariant', 'Product', 'Products', 'InventoryItem', 'Inventory',
    'ReturnRequest', 'Returns', 'Payments', 'CartItem', 'Cart', 'Carts',
    'SalesSummary', 'Analytics', 'Shipment', 'Shipments', 'Warranty',
    'WarrantyClaim', 'Warranties', 'Supplier', 'PurchaseOrder', 'PurchaseOrders',
    'Invoice', 'Invoices', 'BomComponent', 'BillOfMaterials', 'BomApi',
    'WorkOrder', 'WorkOrders', 'ExchangeRate', 'CurrencyOps', 'SubscriptionPlan',
    'Subscription', 'Subscriptions', 'Promotion', 'Promotions', 'TaxJurisdiction',
    'TaxRate', 'Tax', 'Quality', 'Lots', 'Serials', 'Warehouse', 'Receiving',
    'Fulfillment', 'AccountsPayable', 'AccountsReceivable', 'CostAccounting',
    'Credit', 'Backorders', 'GeneralLedger', 'Crypto',
];
foreach ($classes as $name) {
    $name = 'StateSet\\' . $name;
    check(class_exists($name, false), "Native class is missing: $name");
    check((new ReflectionClass($name))->isInternal(), "Class came from a stub: $name");
}

$database = tempnam(sys_get_temp_dir(), 'stateset-php-contract-');
check($database !== false, 'Could not allocate a private database');
try {
    $commerce = new StateSet\Commerce($database);
    // Trailing optional arguments must still be omittable after the macro
    // migration. Properties retain the established snake_case names.
    $customer = $commerce->customers()->create('native@example.test', 'Ada', 'Kernel');
    check($customer->email === 'native@example.test', 'Customer email did not round-trip');
    check($customer->first_name === 'Ada', 'Legacy first_name property changed');
    check($customer->last_name === 'Kernel', 'Legacy last_name property changed');
    check($customer->getFullName() === 'Ada Kernel', 'Public method naming changed');
    $id = $customer->id;
    check($commerce->customers()->get($id)->email === $customer->email, 'Customer lookup failed');
    check($commerce->customers()->getByEmail($customer->email)->id === $id, 'Email lookup failed');

    $copy = clone $customer;
    check($copy->id === $id && $copy->first_name === 'Ada', 'Native class cloning changed');
    $item = $commerce->inventory()->create('NATIVE-CONTRACT', 12);
    check($item->quantity_on_hand === 12, 'Inventory quantity did not round-trip');
    check($commerce->inventory()->getBySku('NATIVE-CONTRACT')->quantity_available === 12,
        'Inventory lookup or legacy property naming changed');

    $refused = false;
    try {
        $commerce->customers()->get('invalid-uuid');
    } catch (Exception $error) {
        $refused = str_contains($error->getMessage(), 'Invalid customer UUID');
    }
    check($refused, 'Invalid customer identity did not become a PHP exception');

    // A second engine instance reads the committed file, rather than an object
    // cached by the wrapper. This also exercises native module destruction.
    unset($commerce);
    $reopened = new StateSet\Commerce($database);
    check($reopened->customers()->get($id)->first_name === 'Ada', 'Committed data was not durable');
    check($reopened->inventory()->getBySku('NATIVE-CONTRACT')->quantity_on_hand === 12,
        'Inventory was not durable');
    unset($reopened, $customer, $copy, $item);
} finally {
    foreach ([$database, $database . '-wal', $database . '-shm'] as $file) {
        if (is_file($file)) {
            unlink($file);
        }
    }
}

echo "Native class registration, customer persistence, inventory, optional arguments and exceptions passed\n";
