use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Address {
    pub street_name: String,
    pub street_number: String,
    pub postal_code: String,
    pub city: String,
    pub province: String,
    pub address_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Geolocation {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_m: f64,
    pub geolocated_at: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum AssetState { Planned, Active, Inactive, Fault, Retired }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum SplitterLocation { Cro, Cno, Roe, Pte, #[serde(rename="MUFFOLA")] Closure, Other }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum CableType { Feeder, Secondary, Distribution, Drop, Riser }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag="kind",content="details",rename_all="SCREAMING_SNAKE_CASE")]
pub enum AssetDetails {
    Cabinet { cabinet_type: String, central_area: String },
    Splitter { level: u8, ratio: u16, location_type: SplitterLocation, container_id: String, physical_position: String, attenuation_db: Option<f64> },
    Closure { model: String, placement: String, tray_count: u16 },
    Cable { cable_type: CableType, fiber_count: u16, origin_id: String, destination_id: String, length_m: f64, route: String },
    Tube { cable_id: String, number: u16, color: String, fiber_count: u16 },
    Fiber { cable_id: String, tube_id: String, number: u16, color: String },
    Roe { cabinet_id: String, building: String, staircase: Option<String>, floor_position: Option<String>, capacity: u16 },
    Pte { cabinet_id: String, building: String, capacity: u16 },
    TerminalBlock { roe_id: String, physical_position: String, capacity: u16 },
    Port { owner_id: String, number: u16, direction: String, fiber_id: Option<String> },
    Riser { building: String, staircase: String, cable_id: String, floors: u16 },
    FloorDistribution { riser_id: String, floor: String, input_fiber_id: String, output_capacity: u16 },
    Termination { roe_port_id: String, destination_unit: String, cable_id: String, fiber_id: String, length_m: f64, route: String, conduit: Option<String>, obstacles: Option<String> },
    Oto { work_order_id: String, customer_id: String, room: String, connector: String, fiber_id: String },
    Ont { serial: String, vendor: String, model: String, roe_port_id: String, oto_id: String, work_order_id: String, customer_id: String, vlan: Option<u16> },
    Modem { serial: String, vendor: String, model: String, ont_id: String, work_order_id: String, customer_id: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct PhysicalAsset {
    pub asset_id: String,
    pub operational_code: String,
    pub state: AssetState,
    pub address: Address,
    pub geolocation: Option<Geolocation>,
    pub asset: AssetDetails,
    pub notes: Option<String>,
}

impl AssetDetails {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Cabinet{..}=>"CABINET", Self::Splitter{..}=>"SPLITTER", Self::Closure{..}=>"CLOSURE",
            Self::Cable{..}=>"CABLE", Self::Tube{..}=>"TUBE", Self::Fiber{..}=>"FIBER", Self::Roe{..}=>"ROE",
            Self::Pte{..}=>"PTE", Self::TerminalBlock{..}=>"TERMINAL_BLOCK", Self::Port{..}=>"PORT",
            Self::Riser{..}=>"RISER", Self::FloorDistribution{..}=>"FLOOR_DISTRIBUTION",
            Self::Termination{..}=>"TERMINATION", Self::Oto{..}=>"OTO", Self::Ont{..}=>"ONT", Self::Modem{..}=>"MODEM",
        }
    }
    pub fn references(&self) -> Vec<(&str,&'static [&'static str])> {
        match self {
            Self::Cabinet{..}|Self::Closure{..}=>vec![],
            Self::Splitter{container_id,..}=>vec![(container_id,&["CABINET","ROE","PTE","CLOSURE"])],
            Self::Cable{origin_id,destination_id,..}=>vec![(origin_id,&["CABINET","CLOSURE","ROE","PTE","OTO","FLOOR_DISTRIBUTION"]),(destination_id,&["CABINET","CLOSURE","ROE","PTE","OTO","FLOOR_DISTRIBUTION"])],
            Self::Tube{cable_id,..}=>vec![(cable_id,&["CABLE"])],
            Self::Fiber{cable_id,tube_id,..}=>vec![(cable_id,&["CABLE"]),(tube_id,&["TUBE"])],
            Self::Roe{cabinet_id,..}|Self::Pte{cabinet_id,..}=>vec![(cabinet_id,&["CABINET"])],
            Self::TerminalBlock{roe_id,..}=>vec![(roe_id,&["ROE","PTE"])],
            Self::Port{owner_id,fiber_id,..}=>{
                let mut references=vec![(owner_id.as_str(), &["SPLITTER","TERMINAL_BLOCK"] as &'static [&'static str])];
                if let Some(fiber_id)=fiber_id { references.push((fiber_id,&["FIBER"])); }
                references
            },
            Self::Riser{cable_id,..}=>vec![(cable_id,&["CABLE"])],
            Self::FloorDistribution{riser_id,input_fiber_id,..}=>vec![(riser_id,&["RISER"]),(input_fiber_id,&["FIBER"])],
            Self::Termination{roe_port_id,cable_id,fiber_id,..}=>vec![(roe_port_id,&["PORT"]),(cable_id,&["CABLE"]),(fiber_id,&["FIBER"])],
            Self::Oto{fiber_id,..}=>vec![(fiber_id,&["FIBER"])],
            Self::Ont{roe_port_id,oto_id,..}=>vec![(roe_port_id,&["PORT"]),(oto_id,&["OTO"])],
            Self::Modem{ont_id,..}=>vec![(ont_id,&["ONT"])],
        }
    }
}

pub fn validate(asset: &PhysicalAsset) -> Result<(), &'static str> {
    identifier(&asset.asset_id)?;
    text(&asset.operational_code,128)?;
    for value in [&asset.address.street_name,&asset.address.street_number,&asset.address.postal_code,&asset.address.city,&asset.address.province] { text(value,256)?; }
    for value in [&asset.notes,&asset.address.address_note].into_iter().flatten() { text(value,2048)?; }
    if let Some(location)=&asset.geolocation {
        if !location.latitude.is_finite() || !(-90.0..=90.0).contains(&location.latitude)
            || !location.longitude.is_finite() || !(-180.0..=180.0).contains(&location.longitude)
            || !location.accuracy_m.is_finite() || location.accuracy_m<=0.0
        { return Err("invalid GPS coordinates or accuracy"); }
        chrono::DateTime::parse_from_rfc3339(&location.geolocated_at).map_err(|_| "GPS time must be RFC3339")?;
        text(&location.source,128)?;
    }
    for (id,_) in asset.asset.references() { identifier(id)?; if id==asset.asset_id { return Err("asset cannot reference itself"); } }
    match &asset.asset {
        AssetDetails::Cabinet{cabinet_type,central_area}=> { text(cabinet_type,128)?; text(central_area,256)?; },
        AssetDetails::Splitter{level,ratio,attenuation_db,physical_position,..}=>{
            if !matches!(level,1|2)||*ratio==0||*ratio>128 {return Err("invalid splitter level/ratio");}
            if attenuation_db.is_some_and(|db| !db.is_finite()||db<0.0) {return Err("invalid splitter attenuation");}
            text(physical_position,256)?;
        },
        AssetDetails::Closure{model,placement,tray_count}=>{text(model,128)?;text(placement,128)?;positive(*tray_count)?;},
        AssetDetails::Cable{fiber_count,length_m,route,..}=>{positive(*fiber_count)?;length(*length_m)?;text(route,4096)?;},
        AssetDetails::Tube{number,color,fiber_count,..}=>{positive(*number)?;positive(*fiber_count)?;text(color,64)?;},
        AssetDetails::Fiber{number,color,..}=>{positive(*number)?;text(color,64)?;},
        AssetDetails::Roe{capacity,building,..}|AssetDetails::Pte{capacity,building,..}=>{positive(*capacity)?;text(building,256)?;},
        AssetDetails::TerminalBlock{capacity,physical_position,..}=>{positive(*capacity)?;text(physical_position,256)?;},
        AssetDetails::Port{number,direction,..}=>{positive(*number)?;if !matches!(direction.as_str(),"IN"|"OUT") {return Err("port direction must be IN or OUT");}},
        AssetDetails::Riser{building,staircase,floors,..}=>{text(building,256)?;text(staircase,128)?;positive(*floors)?;},
        AssetDetails::FloorDistribution{floor,output_capacity,..}=>{text(floor,64)?;positive(*output_capacity)?;},
        AssetDetails::Termination{destination_unit,length_m,route,..}=>{text(destination_unit,256)?;length(*length_m)?;text(route,4096)?;},
        AssetDetails::Oto{work_order_id,customer_id,room,connector,..}=>{identifier(work_order_id)?;identifier(customer_id)?;text(room,128)?;text(connector,64)?;},
        AssetDetails::Ont{serial,vendor,model,work_order_id,customer_id,vlan,..}=>{
            text(serial,128)?;text(vendor,128)?;text(model,128)?;identifier(work_order_id)?;identifier(customer_id)?;
            if vlan.is_some_and(|vlan| vlan==0||vlan>=4095) {return Err("VLAN must be 1..4094");}
        },
        AssetDetails::Modem{serial,vendor,model,work_order_id,customer_id,..}=>{text(serial,128)?;text(vendor,128)?;text(model,128)?;identifier(work_order_id)?;identifier(customer_id)?;},
    }
    Ok(())
}

pub fn identifier(value:&str)->Result<(), &'static str> {
    if value.is_empty()||value.len()>128||!value.bytes().all(|byte| byte.is_ascii_alphanumeric()||matches!(byte,b'-'|b'_')) {return Err("invalid asset/reference ID");}
    Ok(())
}
fn text(value:&str,max:usize)->Result<(), &'static str> { if value.trim().is_empty()||value.len()>max||value.chars().any(char::is_control) {Err("invalid asset text field")} else {Ok(())} }
fn positive(value:u16)->Result<(), &'static str> {if value==0||value>512 {Err("capacity/position must be 1..512")} else {Ok(())}}
fn length(value:f64)->Result<(), &'static str> {if !value.is_finite()||value<0.0 {Err("invalid cable length")} else {Ok(())}}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operational_code_is_not_internal_identifier_and_gps_is_optional() {
        let mut asset=PhysicalAsset {asset_id:"internal-1".into(),operational_code:"ARL UD 0234".into(),state:AssetState::Active,
            address:Address{street_name:"Via Roma".into(),street_number:"27".into(),postal_code:"33100".into(),city:"Udine".into(),province:"UD".into(),address_note:None},
            geolocation:None,asset:AssetDetails::Cabinet{cabinet_type:"ARLO".into(),central_area:"UDINE".into()},notes:None};
        assert!(validate(&asset).is_ok());
        asset.geolocation=Some(Geolocation{latitude:91.0,longitude:13.0,accuracy_m:1.0,geolocated_at:"2026-11-08T12:00:00Z".into(),source:"TECHNICIAN".into()});
        assert!(validate(&asset).is_err());
    }
}