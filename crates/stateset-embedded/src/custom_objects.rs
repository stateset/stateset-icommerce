//! Custom Objects (custom states / metaobjects) operations

use stateset_core::{
    CreateCustomObject, CreateCustomObjectType, CustomObject, CustomObjectFilter, CustomObjectType,
    CustomObjectTypeFilter, Result, UpdateCustomObject, UpdateCustomObjectType,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Custom object operations interface.
pub struct CustomObjects {
    db: Arc<dyn Database>,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl std::fmt::Debug for CustomObjects {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomObjects").finish_non_exhaustive()
    }
}

impl CustomObjects {
    #[cfg(feature = "events")]
    pub(crate) fn new(db: Arc<dyn Database>, event_system: Arc<EventSystem>) -> Self {
        Self { db, event_system }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    #[cfg(feature = "events")]
    fn type_fields_changed(input: &UpdateCustomObjectType) -> Vec<String> {
        let mut fields = Vec::new();
        if input.display_name.is_some() {
            fields.push("display_name".to_string());
        }
        if input.description.is_some() {
            fields.push("description".to_string());
        }
        if input.fields.is_some() {
            fields.push("fields".to_string());
        }
        fields
    }

    #[cfg(feature = "events")]
    fn object_fields_changed(input: &UpdateCustomObject) -> Vec<String> {
        let mut fields = Vec::new();
        if input.handle.is_some() {
            fields.push("handle".to_string());
        }
        if input.owner_type.is_some() || input.owner_id.is_some() {
            fields.push("owner".to_string());
        }
        if input.values.is_some() {
            fields.push("values".to_string());
        }
        fields
    }

    // ------------------------------------------------------------------------
    // Types (schemas)
    // ------------------------------------------------------------------------

    pub fn create_type(&self, input: CreateCustomObjectType) -> Result<CustomObjectType> {
        let ty = self.db.custom_objects().create_type(input)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::CustomObjectTypeCreated {
                type_id: ty.id,
                handle: ty.handle.clone(),
                timestamp: ty.created_at,
            });
        }
        Ok(ty)
    }

    pub fn get_type(&self, id: Uuid) -> Result<Option<CustomObjectType>> {
        self.db.custom_objects().get_type(id)
    }

    pub fn get_type_by_handle(&self, handle: &str) -> Result<Option<CustomObjectType>> {
        self.db.custom_objects().get_type_by_handle(handle)
    }

    pub fn update_type(&self, id: Uuid, input: UpdateCustomObjectType) -> Result<CustomObjectType> {
        #[cfg(feature = "events")]
        let fields_changed = Self::type_fields_changed(&input);

        let updated = self.db.custom_objects().update_type(id, input)?;

        #[cfg(feature = "events")]
        if !fields_changed.is_empty() {
            self.emit(CommerceEvent::CustomObjectTypeUpdated {
                type_id: updated.id,
                handle: updated.handle.clone(),
                fields_changed,
                timestamp: updated.updated_at,
            });
        }

        Ok(updated)
    }

    pub fn list_types(&self, filter: CustomObjectTypeFilter) -> Result<Vec<CustomObjectType>> {
        self.db.custom_objects().list_types(filter)
    }

    pub fn delete_type(&self, id: Uuid) -> Result<()> {
        #[cfg(feature = "events")]
        let ty = self.db.custom_objects().get_type(id)?;

        self.db.custom_objects().delete_type(id)?;

        #[cfg(feature = "events")]
        if let Some(ty) = ty {
            self.emit(CommerceEvent::CustomObjectTypeDeleted {
                type_id: ty.id,
                handle: ty.handle,
                timestamp: chrono::Utc::now(),
            });
        }

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Records
    // ------------------------------------------------------------------------

    pub fn create_object(&self, input: CreateCustomObject) -> Result<CustomObject> {
        let obj = self.db.custom_objects().create_object(input)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::CustomObjectCreated {
                object_id: obj.id,
                type_handle: obj.type_handle.clone(),
                owner_type: obj.owner_type.clone(),
                owner_id: obj.owner_id.clone(),
                timestamp: obj.created_at,
            });
        }
        Ok(obj)
    }

    pub fn get_object(&self, id: Uuid) -> Result<Option<CustomObject>> {
        self.db.custom_objects().get_object(id)
    }

    pub fn get_object_by_handle(
        &self,
        type_handle: &str,
        object_handle: &str,
    ) -> Result<Option<CustomObject>> {
        self.db.custom_objects().get_object_by_handle(type_handle, object_handle)
    }

    pub fn update_object(&self, id: Uuid, input: UpdateCustomObject) -> Result<CustomObject> {
        #[cfg(feature = "events")]
        let fields_changed = Self::object_fields_changed(&input);

        let updated = self.db.custom_objects().update_object(id, input)?;

        #[cfg(feature = "events")]
        if !fields_changed.is_empty() {
            self.emit(CommerceEvent::CustomObjectUpdated {
                object_id: updated.id,
                type_handle: updated.type_handle.clone(),
                fields_changed,
                timestamp: updated.updated_at,
            });
        }

        Ok(updated)
    }

    pub fn list_objects(&self, filter: CustomObjectFilter) -> Result<Vec<CustomObject>> {
        self.db.custom_objects().list_objects(filter)
    }

    pub fn delete_object(&self, id: Uuid) -> Result<()> {
        #[cfg(feature = "events")]
        let obj = self.db.custom_objects().get_object(id)?;

        self.db.custom_objects().delete_object(id)?;

        #[cfg(feature = "events")]
        if let Some(obj) = obj {
            self.emit(CommerceEvent::CustomObjectDeleted {
                object_id: obj.id,
                type_handle: obj.type_handle,
                timestamp: chrono::Utc::now(),
            });
        }

        Ok(())
    }
}
