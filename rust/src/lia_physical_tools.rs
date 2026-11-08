use crate::{
    domain::physical_asset::PhysicalAsset, lia_mcp::LiaMcp, services::physical_asset_service,
};
use rmcp::{handler::server::wrapper::Parameters, schemars::JsonSchema, tool, tool_router};
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct CreateAssetArgs {
    pub operation_id: String,
    pub asset: PhysicalAsset,
}
#[derive(Deserialize, JsonSchema)]
pub struct UpdateAssetArgs {
    pub operation_id: String,
    pub asset: PhysicalAsset,
    pub expected_revision: i64,
}
#[derive(Deserialize, JsonSchema)]
pub struct GetAssetArgs {
    pub asset_id: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct DryRunAssetArgs {
    pub asset: PhysicalAsset,
    pub updating: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct LegacyImportArgs {
    pub operation_id: String,
    pub kind: crate::services::legacy_asset_service::LegacyAssetKind,
    pub source_id: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct LegacyPreviewArgs {
    pub kind: crate::services::legacy_asset_service::LegacyAssetKind,
    pub source_id: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct LegacySyncArgs {
    pub operation_id: String,
}

#[tool_router(router=legacy_tools,vis="pub(crate)")]
impl LiaMcp {
    #[tool(
        description = "DRY-RUN: read actual legacy AAA asset metadata before import. Reports missing measured metadata explicitly; does not invent address/GPS or persist data."
    )]
    async fn dry_run_legacy_import(
        &self,
        Parameters(args): Parameters<LegacyPreviewArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        crate::services::legacy_asset_service::preview(&self.pool, &args.kind, &args.source_id)
            .await
            .map(|result| result.to_string())
            .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))
    }
    #[tool(
        description = "ADMIN WRITE: register one existing AAA legacy asset in the physical registry with its actual ID, capacity and provenance. Missing measured metadata remains LEGACY_INCOMPLETE. Parents must be imported before children; measured records are never overwritten."
    )]
    async fn import_legacy_asset(
        &self,
        Parameters(args): Parameters<LegacyImportArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        crate::services::legacy_asset_service::import(
            &self.pool,
            &self.admin,
            &args.operation_id,
            &args.kind,
            &args.source_id,
        )
        .await
        .map(|result| result.to_string())
        .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))
    }
    #[tool(
        description = "ADMIN WRITE: import up to 1000 actual legacy cabinets/splitters/ports/feeders/ROE/PTE/ONT/modems into the physical registry in parent-first order, one AAA transaction with per-asset audit/idempotency. Does not modify legacy rows or fill unknown GPS/address/details."
    )]
    async fn import_legacy_inventory(
        &self,
        Parameters(args): Parameters<LegacySyncArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        crate::services::legacy_asset_service::sync_inventory(
            &self.pool,
            &self.admin,
            &args.operation_id,
        )
        .await
        .map(|result| result.to_string())
        .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct AssignResourceArgs {
    pub operation_id: String,
    pub resource_id: String,
    pub work_order_id: String,
    pub customer_id: Option<String>,
    pub reason: String,
}

fn kind(asset: &PhysicalAsset, expected: &str) -> Result<(), rmcp::ErrorData> {
    if asset.asset.kind() != expected {
        Err(rmcp::ErrorData::invalid_params(
            "asset kind does not match this tool",
            None,
        ))
    } else {
        Ok(())
    }
}

macro_rules! physical_tools {
    ($(($create:ident,$get:ident,$update:ident,$dry_run:ident,$kind:literal)),+ $(,)?)=>{
        #[tool_router(router=physical_tools,vis="pub(crate)")]
        impl LiaMcp {
            $(
                #[tool(description="ADMIN WRITE: create one typed physical asset with operational code, full address, optional GPS and validated dependencies. Uses AAA transaction, capacity checks, idempotency and audit.")]
                async fn $create(&self,Parameters(args):Parameters<CreateAssetArgs>)->Result<String,rmcp::ErrorData> {
                    kind(&args.asset,$kind)?;
                    let result=physical_asset_service::save(&self.pool,&self.admin,&args.operation_id,&args.asset,None).await
                        .map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?;
                    Ok(result.to_string())
                }
                #[tool(description="READ: get one typed physical asset from AAA including code, address, GPS, details, authors and revision. Does not invent missing metadata.")]
                async fn $get(&self,Parameters(args):Parameters<GetAssetArgs>)->Result<String,rmcp::ErrorData> {
                    let result=physical_asset_service::get(&self.pool,&args.asset_id).await
                        .map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?;
                    if result.as_ref().is_some_and(|value| value["asset"]["asset"]["kind"].as_str()!=Some($kind)) {
                        return Err(rmcp::ErrorData::invalid_params("asset kind does not match this tool",None));
                    }
                    Ok(serde_json::json!(result).to_string())
                }
                #[tool(description="ADMIN WRITE: update a physical asset at the expected revision. Cannot change type or mutate an assigned/referenced asset. Validates dependencies and capacity, records before/after audit and idempotency.")]
                async fn $update(&self,Parameters(args):Parameters<UpdateAssetArgs>)->Result<String,rmcp::ErrorData> {
                    kind(&args.asset,$kind)?;
                    let result=physical_asset_service::save(&self.pool,&self.admin,&args.operation_id,&args.asset,Some(args.expected_revision)).await
                        .map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?;
                    Ok(result.to_string())
                }
                #[tool(description="DRY-RUN: validate physical asset metadata, dependencies and capacity against AAA in a read-only transaction. Never persists rows.")]
                async fn $dry_run(&self,Parameters(args):Parameters<DryRunAssetArgs>)->Result<String,rmcp::ErrorData> {
                    kind(&args.asset,$kind)?;
                    let result=physical_asset_service::dry_run(&self.pool,&args.asset,args.updating).await
                        .map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?;
                    Ok(result.to_string())
                }
            )+
        }
    }
}

physical_tools! {
    (create_physical_cabinet,get_physical_cabinet,update_physical_cabinet,dry_run_physical_cabinet,"CABINET"),
    (create_physical_splitter,get_physical_splitter,update_physical_splitter,dry_run_physical_splitter,"SPLITTER"),
    (create_closure,get_closure,update_closure,dry_run_closure,"CLOSURE"),
    (create_fiber_cable,get_fiber_cable,update_fiber_cable,dry_run_fiber_cable,"CABLE"),
    (create_tube,get_tube,update_tube,dry_run_tube,"TUBE"),
    (create_fiber,get_fiber,update_fiber,dry_run_fiber,"FIBER"),
    (create_physical_roe,get_physical_roe,update_physical_roe,dry_run_physical_roe,"ROE"),
    (create_physical_pte,get_physical_pte,update_physical_pte,dry_run_physical_pte,"PTE"),
    (create_terminal_block,get_terminal_block,update_terminal_block,dry_run_terminal_block,"TERMINAL_BLOCK"),
    (create_physical_port,get_physical_port,update_physical_port,dry_run_physical_port,"PORT"),
    (create_riser,get_riser,update_riser,dry_run_riser,"RISER"),
    (create_floor_distribution,get_floor_distribution,update_floor_distribution,dry_run_floor_distribution,"FLOOR_DISTRIBUTION"),
    (create_termination_segment,get_termination_segment,update_termination_segment,dry_run_termination_segment,"TERMINATION"),
    (create_oto,get_oto,update_oto,dry_run_oto,"OTO"),
    (create_physical_ont,get_physical_ont,update_physical_ont,dry_run_physical_ont,"ONT"),
    (create_physical_modem,get_physical_modem,update_physical_modem,dry_run_physical_modem,"MODEM"),
}

macro_rules! assignment_tools {
    ($(($name:ident,$kind:literal,$action:ident)),+ $(,)?)=>{
        #[tool_router(router=resource_tools,vis="pub(crate)")]
        impl LiaMcp {
            $(
                #[tool(description="ADMIN WRITE: change a typed physical fiber/port reservation on AAA. Validates WR/customer and occupied/leased legacy ports. Assignment requires a reservation of the same WR; release cancels only reservations with no customer. Atomic audit and idempotency.")]
                async fn $name(&self,Parameters(args):Parameters<AssignResourceArgs>)->Result<String,rmcp::ErrorData>{
                    self.admin.require_write_authorization().map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?;
                    let asset=physical_asset_service::get(&self.pool,&args.resource_id).await.map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?
                        .ok_or_else(||rmcp::ErrorData::invalid_params("physical resource does not exist",None))?;
                    if asset["asset"]["asset"]["kind"].as_str()!=Some($kind){return Err(rmcp::ErrorData::invalid_params("resource type does not match this tool",None));}
                    let input=crate::domain::resource_assignment::ResourceAssignment{resource_id:args.resource_id,work_order_id:args.work_order_id,customer_id:args.customer_id,action:crate::domain::resource_assignment::AssignmentAction::$action,reason:args.reason};
                    let result=crate::services::resource_assignment_service::change(&self.pool,&self.admin,&args.operation_id,&input).await.map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?;
                    Ok(result.to_string())
                }
            )+
        }
    }
}
assignment_tools! {
    (reserve_port,"PORT",Reserve),
    (assign_port,"PORT",Assign),
    (release_port,"PORT",Release),
    (reserve_fiber,"FIBER",Reserve),
    (allocate_fiber,"FIBER",Assign),
    (release_fiber,"FIBER",Release),
}
