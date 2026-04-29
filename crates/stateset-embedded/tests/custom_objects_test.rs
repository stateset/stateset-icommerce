#![cfg(feature = "sqlite")]

use serde_json::json;
use stateset_embedded::{
    Commerce, CreateCustomObject, CreateCustomObjectType, CustomFieldDefinition, CustomFieldType,
    CustomObjectFilter, UpdateCustomObject,
};

#[test]
fn test_custom_objects_type_and_record_crud_and_validation() {
    let commerce = Commerce::new(":memory:").expect("create commerce");

    let ty = commerce
        .custom_objects()
        .create_type(CreateCustomObjectType {
            handle: "warranty_registration__c".into(),
            display_name: "Warranty Registration".into(),
            description: Some("Customer warranty registration records".into()),
            fields: vec![
                CustomFieldDefinition {
                    key: "serial".into(),
                    field_type: CustomFieldType::String,
                    required: true,
                    list: false,
                    description: None,
                },
                CustomFieldDefinition {
                    key: "purchased_at".into(),
                    field_type: CustomFieldType::DateTime,
                    required: false,
                    list: false,
                    description: None,
                },
                CustomFieldDefinition {
                    key: "price_paid".into(),
                    field_type: CustomFieldType::Decimal,
                    required: false,
                    list: false,
                    description: None,
                },
            ],
        })
        .expect("create custom object type");

    assert_eq!(ty.handle, "warranty_registration__c");
    assert_eq!(ty.fields.len(), 3);

    let obj = commerce
        .custom_objects()
        .create_object(CreateCustomObject {
            type_handle: ty.handle.clone(),
            handle: Some("reg_001".into()),
            owner_type: Some("customer".into()),
            owner_id: Some("cust_123".into()),
            values: json!({
                "serial": "SN-0001",
                "purchased_at": "2026-02-09T00:00:00Z",
                "price_paid": "199.99"
            }),
        })
        .expect("create custom object record");

    assert_eq!(obj.type_handle, ty.handle);
    assert_eq!(obj.handle.as_deref(), Some("reg_001"));
    assert_eq!(obj.values["serial"].as_str(), Some("SN-0001"));

    let fetched = commerce
        .custom_objects()
        .get_object(obj.id)
        .expect("get custom object by id")
        .expect("custom object should exist");
    assert_eq!(fetched.values["serial"].as_str(), Some("SN-0001"));

    // Missing required field should fail.
    let err = commerce
        .custom_objects()
        .create_object(CreateCustomObject {
            type_handle: ty.handle.clone(),
            handle: Some("bad_001".into()),
            owner_type: None,
            owner_id: None,
            values: json!({}),
        })
        .unwrap_err();
    assert!(err.is_validation(), "expected validation error, got: {err}");

    // Listing by type handle should return our record.
    let list = commerce
        .custom_objects()
        .list_objects(CustomObjectFilter {
            type_handle: Some(ty.handle.clone()),
            ..Default::default()
        })
        .expect("list custom objects");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, obj.id);

    // Wrong value type should fail.
    let err = commerce
        .custom_objects()
        .update_object(
            obj.id,
            UpdateCustomObject { values: Some(json!({"serial": 123})), ..Default::default() },
        )
        .unwrap_err();
    assert!(err.is_validation(), "expected validation error, got: {err}");

    let updated = commerce
        .custom_objects()
        .update_object(
            obj.id,
            UpdateCustomObject { values: Some(json!({"serial": "SN-0002"})), ..Default::default() },
        )
        .expect("update custom object values");
    assert_eq!(updated.values["serial"].as_str(), Some("SN-0002"));

    // Deleting the type should cascade-delete records.
    commerce.custom_objects().delete_type(ty.id).expect("delete custom object type");
    let gone = commerce.custom_objects().get_object(obj.id).expect("get after delete");
    assert!(gone.is_none());
}
