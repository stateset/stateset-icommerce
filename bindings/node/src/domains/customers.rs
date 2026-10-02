//! Customers API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Customers API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCustomerInput {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub accepts_marketing: Option<bool>,
    pub tags: Option<Vec<String>>,
    #[napi(ts_type = "CustomerMetadata")]
    pub metadata: Option<serde_json::Value>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomerOutput {
    pub id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    #[napi(ts_type = "CustomerStatus")]
    pub status: String,
    pub accepts_marketing: bool,
    pub email_verified: bool,
    pub tags: Vec<String>,
    #[napi(ts_type = "CustomerMetadata")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Customer> for CustomerOutput {
    fn from(c: stateset_core::Customer) -> Self {
        Self {
            id: c.id.to_string(),
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
            phone: c.phone,
            status: format!("{}", c.status),
            accepts_marketing: c.accepts_marketing,
            email_verified: c.email_verified,
            tags: c.tags,
            metadata: c.metadata,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateCustomerInput {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    #[napi(ts_type = "CustomerStatus")]
    pub status: Option<String>,
    pub accepts_marketing: Option<bool>,
    pub tags: Option<Vec<String>>,
    #[napi(ts_type = "CustomerMetadata")]
    pub metadata: Option<serde_json::Value>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCustomerAddressInput {
    pub customer_id: String,
    /// Defaults to `both`.
    #[napi(ts_type = "AddressType")]
    pub address_type: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub is_default: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomerAddressOutput {
    pub id: String,
    pub customer_id: String,
    #[napi(ts_type = "AddressType")]
    pub address_type: String,
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::CustomerAddress> for CustomerAddressOutput {
    fn from(a: stateset_core::CustomerAddress) -> Self {
        Self {
            id: a.id.to_string(),
            customer_id: a.customer_id.to_string(),
            address_type: format!("{}", a.address_type),
            first_name: a.first_name,
            last_name: a.last_name,
            company: a.company,
            line1: a.line1,
            line2: a.line2,
            city: a.city,
            state: a.state,
            postal_code: a.postal_code,
            country: a.country,
            phone: a.phone,
            is_default: a.is_default,
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        }
    }
}

pub(crate) fn create_customer_address_from_input(
    input: CreateCustomerAddressInput,
) -> Result<stateset_core::CreateCustomerAddress> {
    let customer_id: uuid::Uuid = input
        .customer_id
        .parse()
        .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
    let address_type = input
        .address_type
        .map(|s| s.parse::<stateset_core::AddressType>())
        .transpose()
        .map_err(|_| coded(ErrCode::Validation, "Invalid address type"))?;
    Ok(stateset_core::CreateCustomerAddress {
        customer_id: customer_id.into(),
        address_type,
        first_name: input.first_name,
        last_name: input.last_name,
        company: input.company,
        line1: input.line1,
        line2: input.line2,
        city: input.city,
        state: input.state,
        postal_code: input.postal_code,
        country: input.country,
        phone: input.phone,
        is_default: input.is_default,
    })
}

#[napi]
pub struct Customers {
    pub(crate) commerce: Handle,
}

#[napi]
impl Customers {
    #[napi]
    pub async fn create(&self, input: CreateCustomerInput) -> Result<CustomerOutput> {
        let commerce = self.commerce.get()?;
        let customer = commerce
            .customers()
            .create(stateset_core::CreateCustomer {
                email: input.email,
                first_name: input.first_name,
                last_name: input.last_name,
                phone: input.phone,
                accepts_marketing: input.accepts_marketing,
                tags: input.tags,
                metadata: input.metadata,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create customer", e))?;

        Ok(customer.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<CustomerOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let customer = commerce
            .customers()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get customer", e))?;

        Ok(customer.map(|c| c.into()))
    }

    #[napi]
    pub async fn get_by_email(&self, email: String) -> Result<Option<CustomerOutput>> {
        let commerce = self.commerce.get()?;
        let customer = commerce
            .customers()
            .get_by_email(&email)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get customer", e))?;

        Ok(customer.map(|c| c.into()))
    }

    /// List customers, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every customer).
    #[napi]
    pub async fn list(&self, filter: Option<CustomerFilterInput>) -> Result<Vec<CustomerOutput>> {
        let commerce = self.commerce.get()?;
        let filter = customer_filter_from_input(filter)?;
        let customers = commerce
            .customers()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list customers", e))?;

        Ok(customers.into_iter().map(|c| c.into()).collect())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .customers()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count customers", e))?;

        Ok(count as u32)
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateCustomerInput) -> Result<CustomerOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let status = input
            .status
            .map(|s| s.parse::<stateset_core::CustomerStatus>())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer status"))?;
        let customer = commerce
            .customers()
            .update(
                uuid.into(),
                stateset_core::UpdateCustomer {
                    email: input.email,
                    first_name: input.first_name,
                    last_name: input.last_name,
                    phone: input.phone,
                    status,
                    accepts_marketing: input.accepts_marketing,
                    tags: input.tags,
                    metadata: input.metadata,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update customer", e))?;
        Ok(customer.into())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .customers()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete customer", e))?;
        Ok(())
    }

    #[napi]
    pub async fn find_or_create(&self, input: CreateCustomerInput) -> Result<CustomerOutput> {
        let commerce = self.commerce.get()?;
        let customer = commerce
            .customers()
            .find_or_create(stateset_core::CreateCustomer {
                email: input.email,
                first_name: input.first_name,
                last_name: input.last_name,
                phone: input.phone,
                accepts_marketing: input.accepts_marketing,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to find or create customer", e))?;
        Ok(customer.into())
    }

    #[napi]
    pub async fn add_address(
        &self,
        input: CreateCustomerAddressInput,
    ) -> Result<CustomerAddressOutput> {
        let commerce = self.commerce.get()?;
        let address = commerce
            .customers()
            .add_address(create_customer_address_from_input(input)?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add address", e))?;
        Ok(address.into())
    }

    #[napi]
    pub async fn get_addresses(&self, customer_id: String) -> Result<Vec<CustomerAddressOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let addresses = commerce
            .customers()
            .get_addresses(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get addresses", e))?;
        Ok(addresses.into_iter().map(|a| a.into()).collect())
    }

    #[napi]
    pub async fn update_address(
        &self,
        address_id: String,
        input: CreateCustomerAddressInput,
    ) -> Result<CustomerAddressOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            address_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let address = commerce
            .customers()
            .update_address(uuid, create_customer_address_from_input(input)?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update address", e))?;
        Ok(address.into())
    }

    #[napi]
    pub async fn delete_address(&self, address_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            address_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .customers()
            .delete_address(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete address", e))?;
        Ok(())
    }

    #[napi(ts_args_type = "customerId: string, addressId: string, addressType: AddressType")]
    pub async fn set_default_address(
        &self,
        customer_id: String,
        address_id: String,
        address_type: String,
    ) -> Result<()> {
        let commerce = self.commerce.get()?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let addr: uuid::Uuid =
            address_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid address UUID"))?;
        let atype = address_type
            .parse::<stateset_core::AddressType>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid address type"))?;
        commerce
            .customers()
            .set_default_address(cust.into(), addr, atype)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set default address", e))?;
        Ok(())
    }
}
