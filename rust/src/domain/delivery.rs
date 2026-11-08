use crate::domain::physical_asset::{Address,Geolocation,identifier};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize,Serialize};

#[derive(Clone,Debug,Deserialize,Serialize,JsonSchema)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum WorkStatus {New,Assigned,Scheduled,InProgress,Completed,Cancelled,Ko}

#[derive(Clone,Debug,Deserialize,Serialize,JsonSchema)]
pub struct Work {
    pub activation_id:String,
    pub work_order_id:String,
    pub operator_name:String,
    pub technician_id:Option<String>,
    pub ont_serial:Option<String>,
    pub customer_id:Option<String>,
    pub customer_phone:Option<String>,
    pub address:Address,
    pub geolocation:Option<Geolocation>,
    pub appointment_at:Option<String>,
    pub team_id:Option<String>,
    pub status:WorkStatus,
    pub ko_reason:Option<String>,
    pub notes:Option<String>,
}

pub fn validate(input:&Work)->Result<(), &'static str> {
    for value in [&input.activation_id,&input.work_order_id] {identifier(value)?;}
    if let Some(technician)=&input.technician_id {identifier(technician)?;}
    for value in [&input.customer_id,&input.team_id].into_iter().flatten() {identifier(value)?;}
    if let Some(serial)=&input.ont_serial {crate::domain::field_ops::validate_serial(serial)?;}
    if matches!(input.status,WorkStatus::Assigned|WorkStatus::Scheduled|WorkStatus::InProgress|WorkStatus::Completed)&&input.technician_id.is_none() {return Err("work phase requires assigned technician");}
    if matches!(input.status,WorkStatus::Completed)&&(input.ont_serial.is_none()||input.customer_id.is_none()) {return Err("closure requires registered ONT serial and customer");}
    let mut values=vec![(&input.operator_name,256),(&input.address.street_name,256),(&input.address.street_number,64),(&input.address.city,256),(&input.address.postal_code,64),(&input.address.province,64)];
    if let Some(phone)=&input.customer_phone {values.push((phone,64));}
    if let Some(notes)=&input.notes {values.push((notes,2048));}
    if let Some(reason)=&input.ko_reason {values.push((reason,2048));}
    for (value,max) in values {if value.trim().is_empty()||value.len()>max||value.chars().any(char::is_control) {return Err("invalid work text");}}
    if matches!(input.status,WorkStatus::Ko)&&input.ko_reason.as_ref().is_none_or(|reason|reason.trim().is_empty()) {return Err("KO reason required");}
    if let Some(appointment)=&input.appointment_at {chrono::DateTime::parse_from_rfc3339(appointment).map_err(|_|"appointment must be RFC3339")?;}
    if matches!(input.status,WorkStatus::Scheduled)&&input.appointment_at.is_none(){return Err("scheduled work requires appointment");}
    if let Some(gps)=&input.geolocation {
        if !gps.latitude.is_finite()||!(-90.0..=90.0).contains(&gps.latitude)||!gps.longitude.is_finite()||!(-180.0..=180.0).contains(&gps.longitude)||!gps.accuracy_m.is_finite()||gps.accuracy_m<=0.0 {return Err("invalid work GPS");}
        chrono::DateTime::parse_from_rfc3339(&gps.geolocated_at).map_err(|_|"GPS time must be RFC3339")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduled_and_ko_work_require_actual_evidence() {
        let mut work=Work{activation_id:"ACT-1".into(),work_order_id:"WR-1".into(),operator_name:"TEST".into(),technician_id:None,ont_serial:None,customer_id:None,customer_phone:None,
            address:Address{street_name:"Via Roma".into(),street_number:"27".into(),postal_code:"33100".into(),city:"Udine".into(),province:"UD".into(),address_note:None},geolocation:None,appointment_at:None,team_id:None,status:WorkStatus::New,ko_reason:None,notes:None};
        assert!(validate(&work).is_ok());
        work.status=WorkStatus::Scheduled;assert!(validate(&work).is_err());
        work.status=WorkStatus::Ko;assert!(validate(&work).is_err());
        work.status=WorkStatus::Completed;assert!(validate(&work).is_err());
    }
}