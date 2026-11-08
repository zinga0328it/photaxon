use crate::{domain::delivery::Work,lia_mcp::LiaMcp,services::delivery_service};
use rmcp::{handler::server::wrapper::Parameters,schemars::JsonSchema,tool,tool_router};
use serde::Deserialize;

#[derive(Deserialize,JsonSchema)]
pub struct CreateWorkArgs{pub operation_id:String,pub work:Work}
#[derive(Deserialize,JsonSchema)]
pub struct UpdateWorkArgs{pub operation_id:String,pub work:Work,pub expected_revision:i64}
#[derive(Deserialize,JsonSchema)]
pub struct WorkArgs{pub work_order_id:String}
#[derive(Deserialize,JsonSchema)]
pub struct CalendarArgs{pub from:String,pub to:String}

#[tool_router(router=delivery_tools,vis="pub(crate)")]
impl LiaMcp{
    #[tool(description="ADMIN WRITE: create or enrich a real WR activation record with operator, customer, full address/GPS, technician and appointment. Requires existing technician/customer, idempotency key and AAA audit.")]
    async fn create_work(&self,Parameters(args):Parameters<CreateWorkArgs>)->Result<String,rmcp::ErrorData>{
        let result=delivery_service::save_work(&self.pool,&self.admin,&args.operation_id,&args.work,None).await.map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?;
        Ok(result.to_string())
    }
    #[tool(description="READ: retrieve a WR and its activation/delivery details from AAA. Ambiguous WR references are refused rather than selecting an arbitrary practice.")]
    async fn get_work(&self,Parameters(args):Parameters<WorkArgs>)->Result<String,rmcp::ErrorData>{
        let result=delivery_service::get_work(&self.pool,&args.work_order_id).await.map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?;
        Ok(serde_json::json!(result).to_string())
    }
    #[tool(description="ADMIN WRITE: update draft delivery details at an expected revision, including appointment and technician. Does not overwrite an active activation lifecycle; audit and idempotency are atomic on AAA.")]
    async fn update_work(&self,Parameters(args):Parameters<UpdateWorkArgs>)->Result<String,rmcp::ErrorData>{
        let result=delivery_service::save_work(&self.pool,&self.admin,&args.operation_id,&args.work,Some(args.expected_revision)).await.map_err(|reason|rmcp::ErrorData::invalid_params(reason,None))?;
        Ok(result.to_string())
    }
    #[tool(description="VALIDATE: check structured WR fields, address/GPS, scheduled appointment and KO reason without reading or writing AAA.")]
    async fn validate_work(&self,Parameters(args):Parameters<CreateWorkArgs>)->Result<String,rmcp::ErrorData>{
        let result=crate::domain::delivery::validate(&args.work);
        Ok(match result{Ok(())=>serde_json::json!({"status":"PASS","persisted":false}),Err(reason)=>serde_json::json!({"status":"DENY","reason":reason,"persisted":false})}.to_string())
    }
    #[tool(description="READ: list up to 200 scheduled WR appointments in an explicit RFC3339 time interval, with assigned technician and team. No notifications are sent.")]
    async fn get_work_calendar(&self,Parameters(args):Parameters<CalendarArgs>)->Result<String,rmcp::ErrorData>{
        let from=chrono::DateTime::parse_from_rfc3339(&args.from).map_err(|_|rmcp::ErrorData::invalid_params("from must be RFC3339",None))?;
        let to=chrono::DateTime::parse_from_rfc3339(&args.to).map_err(|_|rmcp::ErrorData::invalid_params("to must be RFC3339",None))?;
        if to<=from||(to-from).num_days()>93{return Err(rmcp::ErrorData::invalid_params("calendar interval must be positive and at most 93 days",None));}
        let rows:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('work_order_id',w.work_order_id,'activation_id',w.activation_id,'appointment_at',w.appointment_at,'team_id',w.team_id,'technician_id',a.technician_id,'status',w.work_status) FROM delivery_work_details w JOIN activation_requests a ON a.activation_id=w.activation_id WHERE appointment_at>=$1::text::timestamptz AND appointment_at<$2::text::timestamptz ORDER BY appointment_at,w.work_order_id LIMIT 200")
            .bind(from.to_rfc3339()).bind(to.to_rfc3339()).fetch_all(&self.pool).await.map_err(|_|rmcp::ErrorData::internal_error("AAA work calendar unavailable",None))?;
        Ok(serde_json::json!(rows).to_string())
    }
    #[tool(description="READ: count delivery WR records by workflow status from AAA. Does not count synthetic or inferred practices.")]
    async fn get_work_statistics(&self)->Result<String,rmcp::ErrorData>{
        let rows:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('status',work_status,'count',count(*)) FROM delivery_work_details GROUP BY work_status ORDER BY work_status")
            .fetch_all(&self.pool).await.map_err(|_|rmcp::ErrorData::internal_error("AAA delivery statistics unavailable",None))?;
        Ok(serde_json::json!(rows).to_string())
    }
}