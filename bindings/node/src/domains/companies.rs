//! Companies  (B2B accounts and contacts).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Companies  (B2B accounts and contacts)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCompanyInput {
    pub name: String,
    pub reference: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// ISO 4217 currency code
    pub currency: Option<String>,
    pub payment_terms_days: Option<i32>,
    pub tags: Option<Vec<String>>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateCompanyInput {
    pub name: Option<String>,
    pub reference: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// ISO 4217 currency code
    pub currency: Option<String>,
    pub payment_terms_days: Option<i32>,
    /// active, inactive
    #[napi(ts_type = "CompanyStatus")]
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CompanyFilterInput {
    /// active, inactive
    #[napi(ts_type = "CompanyStatus")]
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateContactInput {
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub title: Option<String>,
    pub company_ids: Option<Vec<String>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CompanyOutput {
    pub id: String,
    pub name: String,
    pub reference: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub currency: String,
    pub payment_terms_days: Option<i32>,
    /// active, inactive
    #[napi(ts_type = "CompanyStatus")]
    pub status: String,
    pub tags: Vec<String>,
    /// Metadata as JSON
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Company> for CompanyOutput {
    fn from(c: stateset_core::Company) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            reference: c.reference,
            email: c.email,
            phone: c.phone,
            currency: c.currency.to_string(),
            payment_terms_days: c.payment_terms_days,
            status: c.status.to_string(),
            tags: c.tags,
            metadata: c.metadata.to_string(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CompanyShippingAddressOutput {
    pub id: String,
    pub company_id: String,
    pub label: Option<String>,
    pub name: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::CompanyShippingAddress> for CompanyShippingAddressOutput {
    fn from(a: stateset_core::CompanyShippingAddress) -> Self {
        Self {
            id: a.id.to_string(),
            company_id: a.company_id.to_string(),
            label: a.label,
            name: a.name,
            line1: a.line1,
            line2: a.line2,
            city: a.city,
            region: a.region,
            postal_code: a.postal_code,
            country: a.country,
            is_default: a.is_default,
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CompanyPriceOverrideOutput {
    pub company_id: String,
    pub product_id: String,
    /// Exact decimal string
    pub price: String,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::CompanyPriceOverride> for CompanyPriceOverrideOutput {
    fn from(o: stateset_core::CompanyPriceOverride) -> Self {
        Self {
            company_id: o.company_id.to_string(),
            product_id: o.product_id.to_string(),
            price: o.price.to_string(),
            currency: o.currency.to_string(),
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ContactOutput {
    pub id: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub title: Option<String>,
    pub company_ids: Vec<String>,
    pub portal_enabled: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Contact> for ContactOutput {
    fn from(c: stateset_core::Contact) -> Self {
        Self {
            id: c.id.to_string(),
            first_name: c.first_name,
            last_name: c.last_name,
            email: c.email,
            phone: c.phone,
            title: c.title,
            company_ids: c.company_ids.iter().map(ToString::to_string).collect(),
            portal_enabled: c.portal_enabled,
            is_active: c.is_active,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

pub(crate) fn parse_company_status(s: &str) -> Result<stateset_core::CompanyStatus> {
    s.parse::<stateset_core::CompanyStatus>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid company status: {s}")))
}

#[napi]
pub struct Companies {
    pub(crate) commerce: Handle,
}

#[napi]
impl Companies {
    /// Whether the companies backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.companies().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateCompanyInput) -> Result<CompanyOutput> {
        let commerce = self.commerce.get()?;
        let company = commerce
            .companies()
            .create(stateset_core::CreateCompany {
                name: input.name,
                reference: input.reference,
                email: input.email,
                phone: input.phone,
                currency: parse_currency_opt(input.currency)?,
                payment_terms_days: input.payment_terms_days,
                tags: input.tags.unwrap_or_default(),
                metadata: parse_metadata_json(input.metadata)?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create company", e))?;
        Ok(company.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<CompanyOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "company")?;
        let company = commerce
            .companies()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get company", e))?;
        Ok(company.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateCompanyInput) -> Result<CompanyOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "company")?;
        let company = commerce
            .companies()
            .update(
                uuid.into(),
                stateset_core::UpdateCompany {
                    name: input.name,
                    reference: input.reference,
                    email: input.email,
                    phone: input.phone,
                    currency: parse_currency_opt(input.currency)?,
                    payment_terms_days: input.payment_terms_days,
                    status: input.status.as_deref().map(parse_company_status).transpose()?,
                    tags: input.tags,
                    metadata: input.metadata.map(Some).map(parse_metadata_json).transpose()?,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update company", e))?;
        Ok(company.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<CompanyFilterInput>) -> Result<Vec<CompanyOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::CompanyFilter::default()),
            |f| -> Result<stateset_core::CompanyFilter> {
                Ok(stateset_core::CompanyFilter {
                    status: f.status.as_deref().map(parse_company_status).transpose()?,
                    search: f.search,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let companies = commerce
            .companies()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list companies", e))?;
        Ok(companies.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "company")?;
        commerce
            .companies()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete company", e))
    }

    /// List a company's shipping addresses.
    #[napi]
    pub async fn list_addresses(&self, id: String) -> Result<Vec<CompanyShippingAddressOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "company")?;
        let addresses = commerce
            .companies()
            .list_addresses(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list company addresses", e))?;
        Ok(addresses.into_iter().map(Into::into).collect())
    }

    /// List a company's product price overrides.
    #[napi]
    pub async fn list_price_overrides(
        &self,
        id: String,
    ) -> Result<Vec<CompanyPriceOverrideOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "company")?;
        let overrides = commerce
            .companies()
            .list_price_overrides(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list company price overrides", e))?;
        Ok(overrides.into_iter().map(Into::into).collect())
    }

    /// Create a contact linked to one or more companies.
    #[napi]
    pub async fn create_contact(&self, input: CreateContactInput) -> Result<ContactOutput> {
        let commerce = self.commerce.get()?;
        let company_ids = input
            .company_ids
            .unwrap_or_default()
            .iter()
            .map(|id| Ok(parse_uuid_str(id, "company_id")?.into()))
            .collect::<Result<Vec<_>>>()?;
        let contact = commerce
            .companies()
            .create_contact(stateset_core::CreateContact {
                first_name: input.first_name,
                last_name: input.last_name,
                email: input.email,
                phone: input.phone,
                title: input.title,
                company_ids,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create contact", e))?;
        Ok(contact.into())
    }

    #[napi]
    pub async fn get_contact(&self, id: String) -> Result<Option<ContactOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "contact")?;
        let contact = commerce
            .companies()
            .get_contact(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get contact", e))?;
        Ok(contact.map(Into::into))
    }

    /// List contacts for a company.
    #[napi]
    pub async fn list_contacts(&self, company_id: String) -> Result<Vec<ContactOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&company_id, "company")?;
        let contacts = commerce
            .companies()
            .list_contacts(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list contacts", e))?;
        Ok(contacts.into_iter().map(Into::into).collect())
    }
}
