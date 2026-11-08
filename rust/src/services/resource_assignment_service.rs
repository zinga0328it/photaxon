use crate::{domain::resource_assignment::{self,AssignmentAction,ResourceAssignment},mcp::auth::ControlPlaneAdminContext,
    services::control_plane_admin_service as operations};
use serde_json::{Value,json};
use sqlx::{PgPool,Postgres,Row,Transaction};
use uuid::Uuid;

pub async fn change(pool:&PgPool,actor:&ControlPlaneAdminContext,operation_id:&str,input:&ResourceAssignment)->Result<Value, &'static str>{
    actor.require_write_authorization()?;
    let transaction=operations::begin_transaction(pool).await?;
    let (transaction,result)=change_transaction(transaction,actor,operation_id,input).await?;
    transaction.commit().await.map_err(|_|"assignment commit failed")?;
    Ok(result)
}

pub async fn change_transaction<'tx>(mut transaction:Transaction<'tx,Postgres>,actor:&ControlPlaneAdminContext,operation_id:&str,input:&ResourceAssignment)->Result<(Transaction<'tx,Postgres>,Value), &'static str>{
    actor.require_write_authorization()?;
    resource_assignment::validate(input)?;
    let payload=serde_json::to_value(input).map_err(|_|"assignment encoding failed")?;
    let digest=operations::input_digest(&payload)?;
    let tool=match input.action{AssignmentAction::Reserve=>"reserve_resource",AssignmentAction::Assign=>"assign_resource",AssignmentAction::Release=>"release_resource"};
    if let Some(result)=operations::begin_operation(&mut transaction,actor,operation_id,tool,&digest).await?{return Ok((transaction,result));}
    let resource=sqlx::query("SELECT asset_kind,state FROM physical_asset_records WHERE asset_id=$1 FOR UPDATE")
        .bind(&input.resource_id).fetch_optional(&mut *transaction).await.map_err(|_|"resource lock failed")?.ok_or("physical resource not registered")?;
    let kind:String=resource.try_get("asset_kind").map_err(|_|"resource kind unavailable")?;
    let state:String=resource.try_get("state").map_err(|_|"resource state unavailable")?;
    if !matches!(kind.as_str(),"FIBER"|"PORT")||state!="ACTIVE"{return Err("only ACTIVE fiber or port can be assigned");}
    let wr_exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM activation_requests WHERE wr=$1)")
        .bind(&input.work_order_id).fetch_one(&mut *transaction).await.map_err(|_|"WR lookup failed")?;
    if !wr_exists{return Err("WR does not exist");}
    if let Some(customer)=&input.customer_id{
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM customers WHERE customer_id=$1 AND status='ACTIVE')")
            .bind(customer).fetch_one(&mut *transaction).await.map_err(|_|"customer lookup failed")?;
        if !exists{return Err("ACTIVE customer does not exist");}
    }
    let current=sqlx::query("SELECT assignment_id,work_order_id,customer_id,status FROM physical_resource_assignments WHERE resource_id=$1 AND status<>'RELEASED' FOR UPDATE")
        .bind(&input.resource_id).fetch_optional(&mut *transaction).await.map_err(|_|"resource assignment lookup failed")?;
    let legacy_conflict:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ports WHERE port_id=$1 AND resource_state<>'FREE') OR EXISTS(SELECT 1 FROM onts WHERE current_port_id=$1 AND (status='ACTIVE' OR customer_id IS NOT NULL)) OR EXISTS(SELECT 1 FROM resource_leases WHERE resource_id=$1 AND released_at IS NULL AND expires_at>now()) OR EXISTS(SELECT 1 FROM resource_states WHERE resource_id=$1 AND (resource_state<>'FREE' OR owner_transaction_id IS NOT NULL))")
        .bind(&input.resource_id).fetch_one(&mut *transaction).await.map_err(|_|"legacy port protection check failed")?;
    if legacy_conflict{return Err("resource is occupied, owned or customer-associated in AAA");}
    let before=current.as_ref().map(|row|{
        Ok::<_, &'static str>(json!({"assignment_id":row.try_get::<String,_>("assignment_id").map_err(|_|"assignment invalid")?,"status":row.try_get::<String,_>("status").map_err(|_|"assignment state invalid")?}))
    }).transpose()?;
    let assignment_id=match (&input.action,current){
        (AssignmentAction::Reserve,None)=>{
            let id=Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO physical_resource_assignments(assignment_id,resource_id,work_order_id,status,assigned_by) VALUES ($1,$2,$3,'RESERVED',$4)")
                .bind(&id).bind(&input.resource_id).bind(&input.work_order_id).bind(actor.principal_id).execute(&mut *transaction).await.map_err(|_|"resource reservation failed")?;
            id
        },
        (AssignmentAction::Assign,Some(row))=>{
            if row.try_get::<String,_>("work_order_id").map_err(|_|"assignment WR unavailable")?!=input.work_order_id
                ||row.try_get::<String,_>("status").map_err(|_|"assignment state unavailable")?!="RESERVED"
                ||row.try_get::<Option<String>,_>("customer_id").map_err(|_|"assignment customer unavailable")?.is_some(){return Err("resource is occupied or reserved for another WR");}
            let id:String=row.try_get("assignment_id").map_err(|_|"assignment ID unavailable")?;
            sqlx::query("UPDATE physical_resource_assignments SET status='OCCUPIED',customer_id=$2 WHERE assignment_id=$1 AND status='RESERVED'")
                .bind(&id).bind(&input.customer_id).execute(&mut *transaction).await.map_err(|_|"resource assignment failed")?;
            id
        },
        (AssignmentAction::Release,Some(row))=>{
            if row.try_get::<String,_>("work_order_id").map_err(|_|"assignment WR unavailable")?!=input.work_order_id
                ||row.try_get::<String,_>("status").map_err(|_|"assignment state unavailable")?!="RESERVED"
                ||row.try_get::<Option<String>,_>("customer_id").map_err(|_|"assignment customer unavailable")?.is_some(){return Err("occupied/customer-associated resource cannot be released by reservation cancellation");}
            let id:String=row.try_get("assignment_id").map_err(|_|"assignment ID unavailable")?;
            sqlx::query("UPDATE physical_resource_assignments SET status='RELEASED',released_at=now() WHERE assignment_id=$1")
                .bind(&id).execute(&mut *transaction).await.map_err(|_|"reservation release failed")?;
            id
        },
        (AssignmentAction::Reserve,Some(_))=>return Err("resource already reserved or occupied"),
        (AssignmentAction::Assign,None)=>return Err("reserve resource for the WR before assignment"),
        (AssignmentAction::Release,None)=>return Err("resource has no reservation to release"),
    };
    let result=json!({"assignment_id":assignment_id,"resource_id":input.resource_id,"before":before,"action":input.action});
    operations::finish_operation(&mut transaction,actor,operation_id,tool,&kind,&input.resource_id,&digest,payload,json!({"status":"PASS"}),result.clone()).await?;
    Ok((transaction,result))
}