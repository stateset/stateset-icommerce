//! Backend-agnostic dynamic segment checks: membership is the customers that
//! match the rules when it is read, and every customer and order this test
//! makes carries a uuid tag so a shared database's other rows cannot match.

use rust_decimal::Decimal;
use stateset_core::*;
use uuid::Uuid;

fn rule(field: &str, operator: SegmentOperator, value: &str) -> SegmentRule {
    SegmentRule { field: field.into(), operator, value: value.into() }
}

fn order(
    repo: &impl OrderRepository,
    customer: CustomerId,
    total: i64,
    currency: CurrencyCode,
) -> Order {
    repo.create(CreateOrder {
        customer_id: customer,
        currency: Some(currency),
        items: vec![CreateOrderItem {
            product_id: ProductId::new(),
            variant_id: None,
            sku: format!("SEG-{}", Uuid::new_v4().simple()),
            name: "Segment test item".into(),
            quantity: 1,
            unit_price: Decimal::from(total),
            discount: None,
            tax_amount: None,
        }],
        ..Default::default()
    })
    .unwrap()
}

pub(crate) fn dynamic_segments(
    segments: &impl SegmentRepository,
    customers: &impl CustomerRepository,
    orders: &impl OrderRepository,
) {
    let tag = Uuid::new_v4().simple().to_string();
    let customer = |name: &str, tags: Vec<String>| {
        customers
            .create(CreateCustomer {
                email: format!("{name}-{tag}@segments.example"),
                first_name: name.into(),
                last_name: "Segment".into(),
                tags: Some(tags),
                ..Default::default()
            })
            .unwrap()
    };
    let big = customer("big", vec!["vip".into()]);
    let small = customer("small", vec![]);
    let euro = customer("euro", vec!["vip".into()]);
    let none = customer("none", vec!["vip".into()]);

    order(orders, big.id, 400, CurrencyCode::USD);
    order(orders, big.id, 200, CurrencyCode::USD);
    order(orders, small.id, 100, CurrencyCode::USD);
    order(orders, euro.id, 900, CurrencyCode::EUR);
    // A cancelled order does not count towards spend.
    let cancelled = order(orders, small.id, 5_000, CurrencyCode::USD);
    orders
        .update(
            cancelled.id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )
        .unwrap();

    let pinned = rule("email", SegmentOperator::EndsWith, &format!("-{tag}@segments.example"));
    let segment = segments
        .create(CreateSegment {
            name: format!("High USD spenders {tag}"),
            description: None,
            segment_type: SegmentType::Dynamic,
            rules: vec![pinned.clone(), rule("total_spent:USD", SegmentOperator::Gte, "500")],
        })
        .unwrap();

    let members = |id| -> Vec<CustomerId> {
        segments.list_members(id, None, None).unwrap().into_iter().map(|m| m.customer_id).collect()
    };
    assert_eq!(members(segment.id), vec![big.id], "only `big` spent >= 500 USD");
    assert_eq!(segments.count_members(segment.id).unwrap(), 1);
    assert_eq!(
        segments.get(segment.id).unwrap().unwrap().member_count,
        1,
        "get evaluates the count"
    );
    assert!(segments.is_member(segment.id, big.id).unwrap());
    assert!(!segments.is_member(segment.id, small.id).unwrap(), "cancelled spend excluded");
    assert!(!segments.is_member(segment.id, euro.id).unwrap(), "EUR spend is not USD");

    // Membership is evaluated when read: a new order moves `small` in.
    order(orders, small.id, 450, CurrencyCode::USD);
    assert!(segments.is_member(segment.id, small.id).unwrap());
    assert_eq!(segments.count_members(segment.id).unwrap(), 2);
    let listed = segments
        .list(SegmentFilter {
            name: Some(format!("High USD spenders {tag}")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(listed.iter().map(|s| s.member_count).collect::<Vec<_>>(), vec![2], "list too");
    assert_eq!(members(segment.id), vec![big.id, small.id], "ordered by customer creation");
    let page: Vec<_> = segments
        .list_members(segment.id, Some(1), Some(1))
        .unwrap()
        .into_iter()
        .map(|m| m.customer_id)
        .collect();
    assert_eq!(page, vec![small.id], "limit/offset page the evaluated members");

    // Tags and order counts; customers with no orders are evaluated too.
    let tagged = segments
        .create(CreateSegment {
            name: format!("VIPs without orders {tag}"),
            description: None,
            segment_type: SegmentType::Dynamic,
            rules: vec![
                pinned,
                rule("tags", SegmentOperator::Contains, "VIP"),
                rule("order_count", SegmentOperator::Eq, "0"),
            ],
        })
        .unwrap();
    assert_eq!(members(tagged.id), vec![none.id]);

    // Replacing the rules re-validates them and changes membership.
    segments
        .update(
            tagged.id,
            UpdateSegment {
                rules: Some(vec![rule(
                    "email",
                    SegmentOperator::In,
                    &format!("[\"euro-{tag}@segments.example\"]"),
                )]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(members(tagged.id), vec![euro.id]);

    let refused = |result: Result<_>| matches!(result, Err(CommerceError::ValidationError(_)));
    assert!(refused(segments.add_member(segment.id, none.id).map(drop)), "dynamic: no manual adds");
    assert!(refused(segments.remove_member(segment.id, big.id)), "dynamic: no manual removes");
    assert!(refused(
        segments
            .create(CreateSegment {
                name: format!("bad {tag}"),
                description: None,
                segment_type: SegmentType::Dynamic,
                rules: vec![rule("lifetime_value", SegmentOperator::Gt, "1")],
            })
            .map(drop)
    ));
    assert!(refused(
        segments
            .update(
                segment.id,
                UpdateSegment {
                    rules: Some(vec![rule("total_spent", SegmentOperator::Gt, "1")]),
                    ..Default::default()
                },
            )
            .map(drop)
    ));
    assert!(refused(
        segments
            .create(CreateSegment {
                name: format!("static with rules {tag}"),
                description: None,
                segment_type: SegmentType::Static,
                rules: vec![rule("order_count", SegmentOperator::Gt, "1")],
            })
            .map(drop)
    ));

    // Static segments are unchanged: explicit members only.
    let curated = segments
        .create(CreateSegment {
            name: format!("Curated {tag}"),
            description: None,
            segment_type: SegmentType::Static,
            rules: vec![],
        })
        .unwrap();
    segments.add_member(curated.id, euro.id).unwrap();
    assert_eq!(members(curated.id), vec![euro.id]);
    assert!(matches!(segments.is_member(SegmentId::new(), big.id), Err(CommerceError::NotFound)));
}
