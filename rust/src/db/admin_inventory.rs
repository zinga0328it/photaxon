use serde_json::Value;
use sqlx::PgPool;

const CABINETS: &str = "SELECT COALESCE(jsonb_agg(to_jsonb(item) ORDER BY item.cabinet_id), '[]'::jsonb) FROM (SELECT cabinet_id,status,display_name,location_reference,parent_node,notes FROM cabinets ORDER BY cabinet_id LIMIT 1000) item";
const LINES: &str = "SELECT COALESCE(jsonb_agg(to_jsonb(item) ORDER BY item.line_id), '[]'::jsonb) FROM (SELECT line_id,cabinet_id,openstack_network_name,status FROM ftth_lines ORDER BY line_id LIMIT 1000) item";
const SPLITTERS: &str = "SELECT COALESCE(jsonb_agg(to_jsonb(item) ORDER BY item.splitter_id), '[]'::jsonb) FROM (SELECT splitter_id,cabinet_id,line_id,ratio,capacity_total,status FROM splitters ORDER BY splitter_id LIMIT 1000) item";
const PORTS: &str = "SELECT COALESCE(jsonb_agg(to_jsonb(item) ORDER BY item.splitter_id,item.port_id), '[]'::jsonb) FROM (SELECT port_id,splitter_id,resource_state FROM ports ORDER BY splitter_id,port_id LIMIT 5000) item";
const ROES: &str = "SELECT COALESCE(jsonb_agg(to_jsonb(item) ORDER BY item.roe_id), '[]'::jsonb) FROM (SELECT roe_id,cabinet_id,splitter_id,address_reference,upstream_resource,street_name,street_number,city,postal_code,latitude,longitude,location_accuracy_m,feeder_cable_id,status FROM roes ORDER BY roe_id LIMIT 2000) item";
const FEEDER_CABLES: &str = "SELECT COALESCE(jsonb_agg(to_jsonb(item) ORDER BY item.cable_id), '[]'::jsonb) FROM (SELECT cable_id,cabinet_id,fiber_count,fibers_used,fibers_reserved,status FROM feeder_cables ORDER BY cable_id LIMIT 2000) item";
const ONTS: &str = "SELECT COALESCE(jsonb_agg(to_jsonb(item) ORDER BY item.ont_id), '[]'::jsonb) FROM (SELECT ont_id,serial_number,roe_id,current_port_id,customer_id,vendor,model,status FROM onts ORDER BY ont_id LIMIT 5000) item";
const MODEMS: &str = "SELECT COALESCE(jsonb_agg(to_jsonb(item) ORDER BY item.modem_id), '[]'::jsonb) FROM (SELECT m.modem_id,m.serial,m.vendor,m.model,m.status,l.ont_id,l.status AS link_status,l.linked_at,l.ended_at FROM modems m LEFT JOIN ont_modem_links l ON l.modem_id=m.modem_id ORDER BY m.modem_id,l.linked_at LIMIT 5000) item";
const CUSTOMERS: &str = "SELECT COALESCE(jsonb_agg(to_jsonb(item) ORDER BY item.customer_id), '[]'::jsonb) FROM (SELECT customer_id,status FROM customers ORDER BY customer_id LIMIT 5000) item";

pub async fn get_network_inventory(pool: &PgPool) -> Result<Value, sqlx::Error> {
    let cabinets = fetch(pool, CABINETS).await?;
    let lines = fetch(pool, LINES).await?;
    let splitters = fetch(pool, SPLITTERS).await?;
    let ports = fetch(pool, PORTS).await?;
    let roes = fetch(pool, ROES).await?;
    let feeder_cables = fetch(pool, FEEDER_CABLES).await?;
    let onts = fetch(pool, ONTS).await?;
    let modems = fetch(pool, MODEMS).await?;
    let customers = fetch(pool, CUSTOMERS).await?;
    Ok(serde_json::json!({
        "source": "AAA",
        "cabinets": cabinets,
        "lines": lines,
        "splitters": splitters,
        "ports": ports,
        "roes": roes,
        "feeder_cables": feeder_cables,
        "onts": onts,
        "modems_and_links": modems,
        "customers": customers,
    }))
}

async fn fetch(pool: &PgPool, query: &'static str) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar::<_, Value>(query).fetch_one(pool).await
}
