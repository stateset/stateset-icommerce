//! Fulfillment API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Fulfillment API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWaveInput {
    pub warehouse_id: i32,
    pub order_ids: Vec<String>,
    pub priority: Option<i32>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WaveOutput {
    pub id: String,
    pub wave_number: String,
    pub warehouse_id: i32,
    pub order_count: i32,
    #[napi(ts_type = "WaveStatus")]
    pub status: String,
    pub created_at: String,
}

impl From<stateset_core::Wave> for WaveOutput {
    fn from(w: stateset_core::Wave) -> Self {
        Self {
            id: w.id.to_string(),
            wave_number: w.wave_number,
            warehouse_id: w.warehouse_id,
            order_count: w.order_count,
            status: format!("{:?}", w.status),
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PickTaskOutput {
    pub id: String,
    pub wave_id: Option<String>,
    pub order_id: String,
    pub sku: String,
    pub quantity_requested: f64,
    pub quantity_picked: f64,
    #[napi(ts_type = "PickTaskStatus")]
    pub status: String,
    pub source_location_id: i32,
}

impl TryFrom<stateset_core::PickTask> for PickTaskOutput {
    type Error = Error;

    fn try_from(p: stateset_core::PickTask) -> Result<Self> {
        Ok(Self {
            id: p.id.to_string(),
            wave_id: p.wave_id.map(|id| id.to_string()),
            order_id: p.order_id.to_string(),
            sku: p.sku,
            quantity_requested: to_f64_checked(p.quantity_requested, "pick quantity requested")?,
            quantity_picked: to_f64_checked(p.quantity_picked, "pick quantity picked")?,
            status: format!("{:?}", p.status),
            source_location_id: p.source_location_id,
        })
    }
}

/// Optional filters for `Fulfillment.listWaves`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct WaveFilterInput {
    pub warehouse_id: Option<i32>,
    /// The rendered form (`InProgress`) or the engine's snake_case (`in_progress`).
    #[napi(ts_type = "WaveStatusInput")]
    pub status: Option<String>,
    /// RFC 3339 timestamp.
    pub from_date: Option<String>,
    /// RFC 3339 timestamp.
    pub to_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<WaveFilterInput> for stateset_core::WaveFilter {
    type Error = Error;

    fn try_from(f: WaveFilterInput) -> Result<Self> {
        Ok(Self {
            warehouse_id: f.warehouse_id,
            status: parse_optional_enum(f.status, "wave status")?,
            from_date: parse_optional_datetime(f.from_date, "from date")?,
            to_date: parse_optional_datetime(f.to_date, "to date")?,
            limit: f.limit,
            offset: f.offset,
        })
    }
}

/// Optional filters for `Fulfillment.listPicks`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PickTaskFilterInput {
    pub warehouse_id: Option<i32>,
    pub wave_id: Option<String>,
    pub order_id: Option<String>,
    /// The rendered form (`InProgress`) or the engine's snake_case (`in_progress`).
    #[napi(ts_type = "PickTaskStatusInput")]
    pub status: Option<String>,
    pub assigned_to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<PickTaskFilterInput> for stateset_core::PickTaskFilter {
    type Error = Error;

    fn try_from(f: PickTaskFilterInput) -> Result<Self> {
        Ok(Self {
            warehouse_id: f.warehouse_id,
            wave_id: parse_optional_id::<uuid::Uuid>(f.wave_id, "wave")?.map(Into::into),
            order_id: parse_optional_id::<uuid::Uuid>(f.order_id, "order")?.map(OrderId::from),
            status: parse_optional_enum(f.status, "pick status")?,
            assigned_to: f.assigned_to,
            limit: f.limit,
            offset: f.offset,
        })
    }
}

#[napi]
pub struct Fulfillment {
    pub(crate) commerce: Handle,
}

#[napi]
impl Fulfillment {
    /// Create a wave
    #[napi]
    pub async fn create_wave(&self, input: CreateWaveInput) -> Result<WaveOutput> {
        let commerce = self.commerce.get()?;
        let order_ids: Vec<OrderId> =
            parse_id_list::<OrderId>(Some(input.order_ids), "order")?.unwrap_or_default();
        let wave = commerce
            .fulfillment()
            .create_wave(stateset_core::CreateWave {
                warehouse_id: input.warehouse_id,
                order_ids,
                priority: input.priority,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create wave", e))?;
        Ok(wave.into())
    }

    /// Get a wave by ID
    #[napi]
    pub async fn get_wave(&self, id: String) -> Result<Option<WaveOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .get_wave(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get wave", e))?;
        Ok(wave.map(|w| w.into()))
    }

    /// List waves, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_waves(&self, filter: Option<WaveFilterInput>) -> Result<Vec<WaveOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::WaveFilter = filter.unwrap_or_default().try_into()?;
        let waves = commerce
            .fulfillment()
            .list_waves(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list waves", e))?;
        Ok(waves.into_iter().map(|w| w.into()).collect())
    }

    /// Release a wave for picking
    #[napi]
    pub async fn release_wave(&self, id: String) -> Result<WaveOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .release_wave(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to release wave", e))?;
        Ok(wave.into())
    }

    /// Complete a wave
    #[napi]
    pub async fn complete_wave(&self, id: String) -> Result<WaveOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .complete_wave(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete wave", e))?;
        Ok(wave.into())
    }

    /// Cancel a wave
    #[napi]
    pub async fn cancel_wave(&self, id: String) -> Result<WaveOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .cancel_wave(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel wave", e))?;
        Ok(wave.into())
    }

    /// Get a pick task by ID
    #[napi]
    pub async fn get_pick(&self, id: String) -> Result<Option<PickTaskOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let pick = commerce
            .fulfillment()
            .get_pick(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get pick", e))?;
        convert_optional_output(pick)
    }

    /// List pick tasks, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_picks(
        &self,
        filter: Option<PickTaskFilterInput>,
    ) -> Result<Vec<PickTaskOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::PickTaskFilter = filter.unwrap_or_default().try_into()?;
        let picks = commerce
            .fulfillment()
            .list_picks(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list picks", e))?;
        convert_outputs(picks)
    }

    /// Assign a pick task
    #[napi]
    pub async fn assign_pick(&self, id: String, assigned_to: String) -> Result<PickTaskOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let pick = commerce
            .fulfillment()
            .assign_pick(uuid, &assigned_to)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to assign pick", e))?;
        convert_output(pick)
    }

    /// Start a pick task
    #[napi]
    pub async fn start_pick(&self, id: String) -> Result<PickTaskOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let pick = commerce
            .fulfillment()
            .start_pick(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to start pick", e))?;
        convert_output(pick)
    }

    /// Cancel a pick task
    #[napi]
    pub async fn cancel_pick(&self, id: String) -> Result<PickTaskOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let pick = commerce
            .fulfillment()
            .cancel_pick(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel pick", e))?;
        convert_output(pick)
    }

    /// Check if an order is ready to pack
    #[napi]
    pub async fn is_order_ready_to_pack(&self, order_id: String) -> Result<bool> {
        let commerce = self.commerce.get()?;
        let uuid = order_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let ready = commerce
            .fulfillment()
            .is_order_ready_to_pack(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check", e))?;
        Ok(ready)
    }

    /// Check if an order is ready to ship
    #[napi]
    pub async fn is_order_ready_to_ship(&self, order_id: String) -> Result<bool> {
        let commerce = self.commerce.get()?;
        let uuid = order_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let ready = commerce
            .fulfillment()
            .is_order_ready_to_ship(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check", e))?;
        Ok(ready)
    }

    /// Count waves
    #[napi]
    pub async fn count_waves(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .fulfillment()
            .count_waves(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count waves", e))?;
        Ok(count as u32)
    }
}
