use crate::{
    domain::ont_acceptance::{
        OneTestResult, OntTestRequest, OpticalMeasurement, OpticalPrecheckProfile,
    },
    services::ont_acceptance_service as service,
};
use rmcp::{handler::server::wrapper::Parameters, schemars::JsonSchema, tool, tool_router};
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct RequirementsArgs {
    pub work_order_id: String,
    pub ont_id: String,
    pub operator: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct StartArgs {
    pub operation_id: String,
    pub input: OntTestRequest,
}
#[derive(Deserialize, JsonSchema)]
pub struct ResultArgs {
    pub operation_id: String,
    pub input: OneTestResult,
}
#[derive(Deserialize, JsonSchema)]
pub struct DryRunArgs {
    pub input: OneTestResult,
}
#[derive(Deserialize, JsonSchema)]
pub struct TestIdArgs {
    pub test_id: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct WorkArgs {
    pub work_order_id: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct MeasurementArgs {
    pub operation_id: String,
    pub test_id: String,
    pub measurement: OpticalMeasurement,
}
#[derive(Deserialize, JsonSchema)]
pub struct ClosureArgs {
    pub operation_id: String,
    pub work_order_id: String,
    pub external_reference: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct ProfileArgs {
    pub operation_id: String,
    pub input: OpticalPrecheckProfile,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadAuditArgs {
    pub resource_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Deserialize, JsonSchema)]
pub struct OperationHistoryArgs {
    pub operation_id: String,
}

fn error(reason: &'static str) -> rmcp::ErrorData {
    rmcp::ErrorData::invalid_params(reason, None)
}

#[tool_router(router=one_tools,vis="pub")]
impl crate::lia_mcp::LiaMcp {
    #[tool(
        description = "READ: read up to 200 sanitized administrative audit entries, optionally restricted to one resource. Includes ONE test and WR closure audit; no SQL input."
    )]
    async fn read_audit(
        &self,
        Parameters(args): Parameters<ReadAuditArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        crate::services::control_plane_admin_service::read_admin_audit(
            &self.pool,
            args.resource_id.as_deref(),
            args.limit.unwrap_or(50),
        )
        .await
        .map(|entries| serde_json::json!(entries).to_string())
        .map_err(error)
    }

    #[tool(
        description = "READ: retrieve the recorded administrative operation, idempotency digest, result and audit by its operation ID."
    )]
    async fn get_operation_history(
        &self,
        Parameters(args): Parameters<OperationHistoryArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        crate::services::control_plane_admin_service::get_operation_history(
            &self.pool,
            &args.operation_id,
        )
        .await
        .map(|entry| serde_json::json!(entry).to_string())
        .map_err(error)
    }
    #[tool(
        description = "READ: obtain installed WR/ONT acceptance requirements. ONE_PASS and codice_collaudo are mandatory; RX bounds are only local precheck profiles. No ONE API is contacted."
    )]
    async fn get_ont_test_requirements(
        &self,
        Parameters(args): Parameters<RequirementsArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::requirements(
            &self.pool,
            &args.work_order_id,
            &args.ont_id,
            &args.operator,
        )
        .await
        .map(|value| value.to_string())
        .map_err(error)
    }
    #[tool(
        description = "READ VALIDATE: validate real WR/operator/customer/ONT association before manually obtaining the ONE test. Does not start or simulate ONE."
    )]
    async fn validate_ont_for_test(
        &self,
        Parameters(args): Parameters<RequirementsArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::requirements(
            &self.pool,
            &args.work_order_id,
            &args.ont_id,
            &args.operator,
        )
        .await
        .map(|value| {
            serde_json::json!({"status":"PASS","requirements":value,"persisted":false}).to_string()
        })
        .map_err(error)
    }
    #[tool(
        description = "ADMIN WRITE: record a local optical measurement on a pending acceptance test. A precheck PASS never means final ONE PASS."
    )]
    async fn record_optical_measurement(
        &self,
        Parameters(args): Parameters<MeasurementArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::record_measurement(
            &self.pool,
            &self.admin,
            &args.operation_id,
            &args.test_id,
            &args.measurement,
        )
        .await
        .map(|value| value.to_string())
        .map_err(error)
    }
    #[tool(
        description = "ADMIN WRITE: open the acceptance record as ONE_PENDING for an installed ONT/WR. Manual provider only: the technician must run the real test in ONE; this tool does not call ONE."
    )]
    async fn start_ont_acceptance_test(
        &self,
        Parameters(args): Parameters<StartArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::start_test(&self.pool, &self.admin, &args.operation_id, &args.input)
            .await
            .map(|value| value.to_string())
            .map_err(error)
    }
    #[tool(
        description = "ADMIN WRITE: record an outcome actually obtained from ONE. ONE_PASS requires a unique codice_collaudo; supplied WR, ONT, serial and operator must match the started test. No simulated outcome or ONE API call."
    )]
    async fn record_one_test_result(
        &self,
        Parameters(args): Parameters<ResultArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::record_result(&self.pool, &self.admin, &args.operation_id, &args.input)
            .await
            .map(|value| value.to_string())
            .map_err(error)
    }
    #[tool(
        description = "READ: obtain the structured, manually recorded ONE result, its code, local measurements and provenance by test ID."
    )]
    async fn get_one_test_result(
        &self,
        Parameters(args): Parameters<TestIdArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::get_test(&self.pool, &args.test_id)
            .await
            .map(|value| serde_json::json!(value).to_string())
            .map_err(error)
    }
    #[tool(
        description = "READ: list up to 100 actual ONE acceptance records for one WR, including PENDING/PASS/FAIL and codes."
    )]
    async fn list_one_tests_for_work(
        &self,
        Parameters(args): Parameters<WorkArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::list_tests(&self.pool, &args.work_order_id)
            .await
            .map(|value| serde_json::json!(value).to_string())
            .map_err(error)
    }
    #[tool(
        description = "DRY-RUN: validate a proposed real ONE result/code against AAA without persisting any row. This is not a ONE acceptance test."
    )]
    async fn dry_run_one_test_result(
        &self,
        Parameters(args): Parameters<DryRunArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::dry_run_result(&self.pool, &args.input)
            .await
            .map(|value| value.to_string())
            .map_err(error)
    }
    #[tool(
        description = "DRY-RUN: deny positive WR closure without the latest valid ONE_PASS and codice_collaudo for the installed ONT. No local RX value can authorize closure."
    )]
    async fn validate_work_closure(
        &self,
        Parameters(args): Parameters<WorkArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::validate_closure(&self.pool, &args.work_order_id)
            .await
            .map(|value| value.to_string())
            .map_err(error)
    }
    #[tool(
        description = "ADMIN WRITE: close the FiberSpider WR only with installed customer ONT/modem and valid latest ONE_PASS/code. Preserve other extra_fields and enqueue an external delivery patch by WR/reference. External delivery remains PENDING until a documented adapter delivers it."
    )]
    async fn close_work_with_test_result(
        &self,
        Parameters(args): Parameters<ClosureArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::close_work(
            &self.pool,
            &self.admin,
            &args.operation_id,
            &args.work_order_id,
            &args.external_reference,
        )
        .await
        .map(|value| value.to_string())
        .map_err(error)
    }
    #[tool(
        description = "ADMIN WRITE: configure operator/profile/vendor/model RX min/max for local precheck only. These bounds never replace the real ONE outcome and code."
    )]
    async fn set_optical_precheck_profile(
        &self,
        Parameters(args): Parameters<ProfileArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        service::save_profile(&self.pool, &self.admin, &args.operation_id, &args.input)
            .await
            .map(|value| value.to_string())
            .map_err(error)
    }
}
