use rmcp::schemars::JsonSchema;
use serde::{Deserialize,Serialize};

#[derive(Clone,Debug,Deserialize,Serialize,JsonSchema)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum AssignmentAction {Reserve,Assign,Release}

#[derive(Clone,Debug,Deserialize,Serialize,JsonSchema)]
pub struct ResourceAssignment {
    pub resource_id:String,
    pub work_order_id:String,
    pub customer_id:Option<String>,
    pub action:AssignmentAction,
    pub reason:String,
}

pub fn validate(input:&ResourceAssignment)->Result<(), &'static str>{
    crate::domain::physical_asset::identifier(&input.resource_id)?;
    crate::domain::physical_asset::identifier(&input.work_order_id)?;
    if let Some(customer)=&input.customer_id {crate::domain::physical_asset::identifier(customer)?;}
    if matches!(input.action,AssignmentAction::Assign)&&input.customer_id.is_none(){return Err("assignment requires customer");}
    if input.reason.trim().is_empty()||input.reason.len()>1024||input.reason.chars().any(char::is_control){return Err("assignment reason required");}
    Ok(())
}